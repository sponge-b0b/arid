use std::ops::Range;
use std::path::PathBuf;

use thiserror::Error;

use crate::model::{
    NormalizationOptions, NormalizedLine, NormalizedSegment, PreparedFile, StructuralContext,
    StructuralScope,
};
use crate::python::{self, StructuralRegion, SuppressionEvent, SuppressionKind};
use crate::suppression::{SuppressionRegion, derive_suppression_regions};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SuppressionMode {
    Apply,
    Audit,
}

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
    let (file, _) = prepare_file_with_mode(path.into(), source, options, SuppressionMode::Apply)?;

    Ok(file)
}

pub(crate) fn prepare_file_with_mode(
    path: PathBuf,
    source: String,
    options: NormalizationOptions,
    suppression_mode: SuppressionMode,
) -> Result<(PreparedFile, Vec<SuppressionRegion>), PrepareError> {
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

    let suppressions = if suppression_mode == SuppressionMode::Audit {
        derive_suppression_regions(&source, &analysis.suppressions)
    } else {
        Vec::new()
    };

    let (normalized, lines, segments) = normalize_source(
        &source,
        &analysis.masks,
        &analysis.suppressions,
        &analysis.structural_regions,
        suppression_mode,
    );

    Ok((
        PreparedFile {
            path,
            source,
            normalized,
            lines,
            segments,
        },
        suppressions,
    ))
}

fn normalize_source(
    source: &str,
    masks: &[Range<usize>],
    suppressions: &[SuppressionEvent],
    structural_regions: &[StructuralRegion],
    suppression_mode: SuppressionMode,
) -> (String, Vec<NormalizedLine>, Vec<NormalizedSegment>) {
    let mut normalized = String::new();
    let mut lines = Vec::new();
    let mut segments = Vec::new();

    let mut line_start = 0_usize;
    let mut mask_index = 0_usize;
    let mut suppression_index = 0_usize;
    let mut structural_region_index = 0_usize;
    let mut active_structural_regions = Vec::new();

    let apply_suppressions = suppression_mode == SuppressionMode::Apply;
    let mut disabled = false;
    let mut segment_start: Option<u32> = None;

    for (source_line, raw_line) in source.split_inclusive('\n').enumerate() {
        let line_total_end = line_start + raw_line.len();

        let content = raw_line.strip_suffix('\n').unwrap_or(raw_line);
        let content = content.strip_suffix('\r').unwrap_or(content);
        let content_end = line_start + content.len();

        update_structural_regions(
            line_start,
            content_end,
            structural_regions,
            &mut structural_region_index,
            &mut active_structural_regions,
        );

        if !apply_suppressions || !disabled {
            let (line, retained_ranges) =
                normalized_line(source, line_start, content_end, masks, &mut mask_index);

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

                let (context, scope) = classify_structural_line(
                    source,
                    &retained_ranges,
                    structural_regions,
                    &active_structural_regions,
                );

                lines.push(NormalizedLine {
                    text_range: start..end,
                    source_line: u32::try_from(source_line)
                        .expect("source cannot contain more than u32::MAX lines"),
                    effective: is_effective(&line),
                    context,
                    scope,
                });
            }
        } else {
            advance_masks(content_end, masks, &mut mask_index);
        }

        if apply_suppressions {
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
) -> (String, Vec<Range<usize>>) {
    while let Some(mask) = masks.get(*mask_index) {
        if mask.end <= line_start {
            *mask_index += 1;
        } else {
            break;
        }
    }

    let mut output = String::new();
    let mut retained_ranges = Vec::new();
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
            retained_ranges.push(cursor..masked_start);
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
        retained_ranges.push(cursor..line_end);
    }

    *mask_index = index;

    (output.trim().to_owned(), retained_ranges)
}

fn update_structural_regions(
    line_start: usize,
    line_end: usize,
    regions: &[StructuralRegion],
    region_index: &mut usize,
    active: &mut Vec<usize>,
) {
    active.retain(|index| regions[*index].range.end > line_start);

    while let Some(region) = regions.get(*region_index) {
        if region.range.start >= line_end {
            break;
        }

        if region.range.end > line_start {
            active.push(*region_index);
        }

        *region_index += 1;
    }
}

fn classify_structural_line(
    source: &str,
    retained_ranges: &[Range<usize>],
    regions: &[StructuralRegion],
    active: &[usize],
) -> (StructuralContext, StructuralScope) {
    let mut context = None;
    let mut scope = None;

    for retained in retained_ranges {
        let mut boundaries = vec![retained.start, retained.end];

        for index in active {
            let region = &regions[*index];

            if region.range.end <= retained.start || region.range.start >= retained.end {
                continue;
            }

            boundaries.push(region.range.start.max(retained.start));
            boundaries.push(region.range.end.min(retained.end));
        }

        boundaries.sort_unstable();
        boundaries.dedup();

        for interval in boundaries.windows(2) {
            let start = interval[0];
            let end = interval[1];

            if start == end || source[start..end].trim().is_empty() {
                continue;
            }

            let most_specific = active
                .iter()
                .map(|index| &regions[*index])
                .filter(|region| region.range.start <= start && region.range.end >= end)
                .min_by_key(|region| region.range.end - region.range.start);

            let (next_context, next_scope) = most_specific.map_or(
                (StructuralContext::Executable, StructuralScope::Module),
                |region| (region.context, region.scope),
            );

            context = Some(match context {
                None => next_context,
                Some(current) if current == next_context => current,
                Some(_) => StructuralContext::Mixed,
            });

            scope = Some(match scope {
                None => next_scope,
                Some(current) if current == next_scope => current,
                Some(current) => more_specific_scope(current, next_scope),
            });
        }
    }

    (
        context.unwrap_or(StructuralContext::Executable),
        scope.unwrap_or(StructuralScope::Module),
    )
}

fn more_specific_scope(left: StructuralScope, right: StructuralScope) -> StructuralScope {
    if scope_rank(right) > scope_rank(left) {
        right
    } else {
        left
    }
}

fn scope_rank(scope: StructuralScope) -> u8 {
    match scope {
        StructuralScope::Module => 0,
        StructuralScope::Class => 1,
        StructuralScope::Function => 2,
    }
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
    use crate::suppression::SuppressionEnd;

    fn prepare(source: &str) -> PreparedFile {
        prepare_file(
            "example.py",
            source.to_owned(),
            NormalizationOptions::default(),
        )
        .unwrap()
    }

    fn prepare_audit(source: &str) -> (PreparedFile, Vec<SuppressionRegion>) {
        prepare_file_with_mode(
            PathBuf::from("example.py"),
            source.to_owned(),
            NormalizationOptions::default(),
            SuppressionMode::Audit,
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
    fn preserves_decorators_while_ignoring_async_and_nested_signatures() {
        let file = prepare(
            r#"
@outer_decorator
async def outer(
    value: dict[str, tuple[int, int]],
) -> list[int]:
    @inner_decorator
    def inner(item: int) -> int:
        return item

    return [inner(value["item"][0])]
"#,
        );

        assert_eq!(
            texts(&file),
            vec![
                "@outer_decorator",
                "@inner_decorator",
                "return item",
                "return [inner(value[\"item\"][0])]",
            ]
        );
    }

    #[test]
    fn removes_import_without_leaving_a_semicolon() {
        let file = prepare("import os; run()\nrun(); import os\n");

        assert_eq!(texts(&file), vec!["run()", "run()",]);
    }

    #[test]
    fn classifies_structural_context_and_scope() {
        let file = prepare(
            r#"
SETTING = 1

class Example:
    value = 1

    def method(self):
        result = 2
        return result

if enabled:
    nested = 3
"#,
        );

        assert_eq!(
            texts(&file),
            vec![
                "SETTING = 1",
                "class Example:",
                "value = 1",
                "result = 2",
                "return result",
                "if enabled:",
                "nested = 3",
            ]
        );

        let metadata: Vec<_> = file
            .lines
            .iter()
            .map(|line| (line.context, line.scope))
            .collect();

        assert_eq!(
            metadata,
            vec![
                (StructuralContext::Declarative, StructuralScope::Module),
                (StructuralContext::Declarative, StructuralScope::Class),
                (StructuralContext::Declarative, StructuralScope::Class),
                (StructuralContext::Executable, StructuralScope::Function),
                (StructuralContext::Executable, StructuralScope::Function),
                (StructuralContext::Executable, StructuralScope::Module),
                (StructuralContext::Executable, StructuralScope::Module),
            ]
        );
    }

    #[test]
    fn classifies_type_aliases_as_declarative_module_scope() {
        let file = prepare(concat!(
            "type JsonScalar = str | int | float | bool | None\n",
            "\n",
            "type CuratedRagSource = (\n",
            "    ReportRecord\n",
            "    | AgentSignalRecord\n",
            "    | RecommendationRecord\n",
            ")\n",
        ));

        assert_eq!(
            texts(&file),
            vec![
                "type JsonScalar = str | int | float | bool | None",
                "type CuratedRagSource = (",
                "ReportRecord",
                "| AgentSignalRecord",
                "| RecommendationRecord",
                ")",
            ]
        );

        assert!(
            file.lines
                .iter()
                .all(|line| line.context == StructuralContext::Declarative)
        );
        assert!(
            file.lines
                .iter()
                .all(|line| line.scope == StructuralScope::Module)
        );

        assert!(!file.lines.last().unwrap().effective);
    }

    #[test]
    fn classifies_mixed_statements_on_one_line() {
        let file = prepare("value = 1; run()\n");

        assert_eq!(texts(&file), vec!["value = 1; run()"]);
        assert_eq!(file.lines[0].context, StructuralContext::Mixed);
        assert_eq!(file.lines[0].scope, StructuralScope::Module);
    }

    #[test]
    fn classifies_only_retained_source_after_masking() {
        let file = prepare("import os; run()\n");

        assert_eq!(texts(&file), vec!["run()"]);
        assert_eq!(file.lines[0].context, StructuralContext::Executable);
        assert_eq!(file.lines[0].scope, StructuralScope::Module);
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
    fn inline_disable_takes_effect_after_its_physical_line() {
        let file =
            prepare("before()\nkept()  # arid: disable\nhidden()\n# arid: enable\nafter()\n");

        assert_eq!(texts(&file), vec!["before()", "kept()", "after()"]);
        assert_eq!(
            file.segments,
            vec![
                NormalizedSegment { start: 0, end: 2 },
                NormalizedSegment { start: 2, end: 3 },
            ]
        );
    }

    #[test]
    fn audit_preparation_retains_suppressed_source_without_barriers() {
        let (file, suppressions) = prepare_audit(concat!(
            "first()\n",
            "# arid: disable\n",
            "hidden()\n",
            "# arid: disable\n",
            "also_hidden()\n",
            "# arid: enable\n",
            "# arid: enable\n",
            "last()\n",
        ));

        assert_eq!(
            texts(&file),
            vec!["first()", "hidden()", "also_hidden()", "last()"]
        );
        assert_eq!(file.segments, vec![NormalizedSegment { start: 0, end: 4 }]);
        assert_eq!(
            suppressions,
            vec![SuppressionRegion {
                disable_line: 2,
                end: SuppressionEnd::Enable { line: 6 },
            }]
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
            "value = 1\n)\n".to_owned(),
            NormalizationOptions::default(),
        )
        .unwrap_err();

        let message = error.to_string();

        assert!(message.contains("broken.py"));
        assert!(message.contains("invalid Python syntax"));
        assert!(message.contains("line 2, column 1"));
    }
}
