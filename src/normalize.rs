use std::ops::Range;
use std::path::PathBuf;

use thiserror::Error;

use crate::model::{NormalizationOptions, NormalizedLine, NormalizedSegment, PreparedFile};
use crate::python::{self, SuppressionEvent, SuppressionKind};

#[derive(Debug, Error)]
pub enum PrepareError {
    #[error("{path}: {message}")]
    Python { path: PathBuf, message: String },
}

pub fn prepare_file(
    path: impl Into<PathBuf>,
    source: String,
    options: NormalizationOptions,
) -> Result<PreparedFile, PrepareError> {
    let path = path.into();

    // A UTF-8 BOM is not Python source content and must not participate in
    // duplicate matching. Strip it before parsing so parser and mask offsets
    // remain relative to the same source buffer.
    let source = source
        .strip_prefix('\u{feff}')
        .unwrap_or(&source)
        .to_owned();

    let analysis = python::analyze(&source, options).map_err(|error| PrepareError::Python {
        path: path.clone(),
        message: error.to_string(),
    })?;

    let (normalized, lines, segments) =
        normalize_source(&source, &analysis.masks, &analysis.suppressions);

    Ok(PreparedFile {
        path,
        source,
        normalized,
        lines,
        segments,
    })
}

fn normalize_source(
    source: &str,
    masks: &[Range<usize>],
    suppressions: &[SuppressionEvent],
) -> (String, Vec<NormalizedLine>, Vec<NormalizedSegment>) {
    let mut normalized = String::new();
    let mut lines = Vec::new();
    let mut segments = Vec::new();

    let mut line_start = 0_usize;
    let mut mask_index = 0_usize;
    let mut suppression_index = 0_usize;

    let mut disabled = false;
    let mut segment_start: Option<u32> = None;

    for (source_line, raw_line) in source.split_inclusive('\n').enumerate() {
        let line_total_end = line_start + raw_line.len();

        let content = raw_line.strip_suffix('\n').unwrap_or(raw_line);
        let content = content.strip_suffix('\r').unwrap_or(content);
        let content_end = line_start + content.len();

        if !disabled {
            let line = normalized_line(source, line_start, content_end, masks, &mut mask_index);

            if !line.is_empty() {
                let normalized_index = u32::try_from(lines.len()).expect(
                    "a single Python file cannot contain more than u32::MAX normalized lines",
                );

                segment_start.get_or_insert(normalized_index);

                let start = u32::try_from(normalized.len())
                    .expect("normalized source cannot exceed u32::MAX bytes");

                normalized.push_str(&line);

                let end = u32::try_from(normalized.len())
                    .expect("normalized source cannot exceed u32::MAX bytes");

                normalized.push('\n');

                lines.push(NormalizedLine {
                    text_range: start..end,
                    source_line: u32::try_from(source_line)
                        .expect("source cannot contain more than u32::MAX lines"),
                    effective: is_effective(&line),
                });
            }
        } else {
            advance_masks(content_end, masks, &mut mask_index);
        }

        // Suppression directives take effect after their physical source line.
        // This gives intuitive behavior for an inline directive:
        //
        //     do_work()  # arid: disable
        //
        // do_work() itself remains part of the preceding segment.
        while let Some(event) = suppressions.get(suppression_index) {
            if event.offset >= line_total_end {
                break;
            }

            match (disabled, event.kind) {
                (false, SuppressionKind::Disable) => {
                    close_segment(&mut segments, &mut segment_start, lines.len());
                    disabled = true;
                }

                (true, SuppressionKind::Enable) => {
                    disabled = false;
                }

                _ => {}
            }

            suppression_index += 1;
        }

        line_start = line_total_end;
    }

    close_segment(&mut segments, &mut segment_start, lines.len());

    (normalized, lines, segments)
}

fn normalized_line(
    source: &str,
    line_start: usize,
    line_end: usize,
    masks: &[Range<usize>],
    mask_index: &mut usize,
) -> String {
    while let Some(mask) = masks.get(*mask_index) {
        if mask.end <= line_start {
            *mask_index += 1;
        } else {
            break;
        }
    }

    let mut output = String::new();
    let mut cursor = line_start;
    let mut index = *mask_index;

    while let Some(mask) = masks.get(index) {
        if mask.start >= line_end {
            break;
        }

        let masked_start = mask.start.max(line_start);
        let masked_end = mask.end.min(line_end);

        if cursor < masked_start {
            output.push_str(&source[cursor..masked_start]);
        }

        cursor = cursor.max(masked_end);

        if mask.end <= line_end {
            index += 1;
        } else {
            break;
        }
    }

    if cursor < line_end {
        output.push_str(&source[cursor..line_end]);
    }

    *mask_index = index;

    output.trim().to_owned()
}

fn advance_masks(line_end: usize, masks: &[Range<usize>], mask_index: &mut usize) {
    while let Some(mask) = masks.get(*mask_index) {
        if mask.end <= line_end {
            *mask_index += 1;
        } else {
            break;
        }
    }
}

fn close_segment(
    segments: &mut Vec<NormalizedSegment>,
    segment_start: &mut Option<u32>,
    line_count: usize,
) {
    let Some(start) = segment_start.take() else {
        return;
    };

    let end = u32::try_from(line_count)
        .expect("a single Python file cannot contain more than u32::MAX normalized lines");

    if start < end {
        segments.push(NormalizedSegment { start, end });
    }
}

fn is_effective(line: &str) -> bool {
    line.chars()
        .any(|character| character == '_' || character.is_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prepare(source: &str) -> PreparedFile {
        prepare_file(
            "example.py",
            source.to_owned(),
            NormalizationOptions::default(),
        )
        .unwrap()
    }

    fn texts(file: &PreparedFile) -> Vec<&str> {
        file.lines
            .iter()
            .map(|line| {
                &file.normalized[line.text_range.start as usize..line.text_range.end as usize]
            })
            .collect()
    }

    #[test]
    fn ignores_comments_but_not_hashes_inside_strings() {
        let file = prepare(
            r##"
value = "# not a comment"
other = 42  # comment
"##,
        );

        assert_eq!(
            texts(&file),
            vec![r##"value = "# not a comment""##, "other = 42",]
        );
    }

    #[test]
    fn ignores_structural_docstrings_only() {
        let file = prepare(
            r#"
"""module docstring"""

class Example:
    """class docstring"""

    def method(self):
        """method docstring"""
        value = 1
        """ordinary string expression"""
        return value
"#,
        );

        assert_eq!(
            texts(&file),
            vec![
                "class Example:",
                "value = 1",
                "\"\"\"ordinary string expression\"\"\"",
                "return value",
            ]
        );
    }

    #[test]
    fn ignores_imports_at_nested_scopes() {
        let file = prepare(
            r#"
import os

from package import (
    first,
    second,
)

if enabled:
    import nested
    run()
"#,
        );

        assert_eq!(texts(&file), vec!["if enabled:", "run()",]);
    }

    #[test]
    fn ignores_multiline_and_one_line_function_signatures() {
        let file = prepare(
            r#"
def calculate(
    first: int,
    second: int,
) -> int:
    return first + second

def compact(): return calculate(1, 2)
"#,
        );

        assert_eq!(
            texts(&file),
            vec!["return first + second", "return calculate(1, 2)",]
        );
    }

    #[test]
    fn removes_import_without_leaving_a_semicolon() {
        let file = prepare("import os; run()\nrun(); import os\n");

        assert_eq!(texts(&file), vec!["run()", "run()",]);
    }

    #[test]
    fn suppression_creates_segment_barrier() {
        let file = prepare(
            r#"
first()
second()

# arid: disable
ignored()
also_ignored()
# arid: enable

third()
fourth()
"#,
        );

        assert_eq!(
            texts(&file),
            vec!["first()", "second()", "third()", "fourth()",]
        );

        assert_eq!(
            file.segments,
            vec![
                NormalizedSegment { start: 0, end: 2 },
                NormalizedSegment { start: 2, end: 4 },
            ]
        );
    }

    #[test]
    fn punctuation_lines_are_retained_but_not_effective() {
        let file = prepare("value = (\n    call()\n)\n");

        assert_eq!(texts(&file), vec!["value = (", "call()", ")",]);

        assert!(file.lines[0].effective);
        assert!(file.lines[1].effective);
        assert!(!file.lines[2].effective);
    }

    #[test]
    fn preserves_original_source_line_mapping() {
        let file = prepare("\n# ignored\nfirst()\n\nsecond()\n");

        assert_eq!(texts(&file), vec!["first()", "second()",]);

        // Internal source lines are zero-based.
        assert_eq!(file.lines[0].source_line, 2);
        assert_eq!(file.lines[1].source_line, 4);
    }

    #[test]
    fn normalizes_crlf_line_endings() {
        let file = prepare("first()\r\nsecond()\r\n");

        assert_eq!(texts(&file), vec!["first()", "second()",]);
    }

    #[test]
    fn strips_utf8_bom() {
        let file = prepare("\u{feff}first()\n");

        assert_eq!(texts(&file), vec!["first()"]);
    }

    #[test]
    fn rejects_invalid_python() {
        let error = prepare_file(
            "broken.py",
            "def broken(:\n".to_owned(),
            NormalizationOptions::default(),
        )
        .unwrap_err();

        assert!(error.to_string().contains("broken.py"));
        assert!(error.to_string().contains("invalid Python syntax"));
    }
}
