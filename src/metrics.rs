use thiserror::Error;

use crate::corpus::Corpus;
use crate::model::{DuplicateGroup, Occurrence};

#[derive(Debug, Clone, PartialEq)]
pub struct Metrics {
    pub files: u64,
    pub source_lines: u64,
    pub analyzed_lines: u64,
    pub duplicate_groups: u64,
    pub duplicate_lines: u64,
    pub duplication_percent: f64,
}

#[derive(Debug, Error)]
pub enum MetricsError {
    #[error("duplicate group {group} has no occurrences")]
    EmptyGroup { group: usize },

    #[error("duplicate group {group} occurrence references unknown file {file}")]
    UnknownFile { group: usize, file: u32 },

    #[error(
        "duplicate group {group} occurrence in file {file} has invalid \
         normalized range {start}..{end}"
    )]
    InvalidOccurrence {
        group: usize,
        file: u32,
        start: u32,
        end: u64,
    },

    #[error(
        "duplicate group {group} declares length {group_length}, but an \
         occurrence declares length {occurrence_length}"
    )]
    LengthMismatch {
        group: usize,
        group_length: u32,
        occurrence_length: u32,
    },

    #[error(
        "duplicate group {group} contains duplicate occurrence in file \
         {file} at normalized line {start}"
    )]
    DuplicateOccurrence { group: usize, file: u32, start: u32 },
}

/// Calculates repository-level duplication metrics.
///
/// For each duplicate group, exactly one occurrence is treated as the
/// canonical copy. Every other occurrence is redundant.
///
/// Canonical selection is deterministic because `FileId` values follow the
/// corpus's sorted path order and `Occurrence` ordering then uses normalized
/// source position.
///
/// Redundant lines are unioned across all groups, so a normalized source line
/// that participates in multiple findings contributes only once to
/// `duplicate_lines`.
pub fn calculate_metrics(
    corpus: &Corpus,
    groups: &[DuplicateGroup],
) -> Result<Metrics, MetricsError> {
    let files = corpus.files.len() as u64;

    let source_lines = corpus
        .files
        .iter()
        .map(|file| physical_line_count(&file.source))
        .sum();

    let analyzed_lines = corpus
        .files
        .iter()
        .map(|file| file.lines.iter().filter(|line| line.effective).count() as u64)
        .sum();

    let mut redundant = corpus
        .files
        .iter()
        .map(|file| vec![false; file.lines.len()])
        .collect::<Vec<_>>();

    for (group_index, group) in groups.iter().enumerate() {
        validate_group(corpus, group, group_index)?;

        let canonical_index = group
            .occurrences
            .iter()
            .enumerate()
            .min_by_key(|(_, occurrence)| *occurrence)
            .map(|(index, _)| index)
            .expect("validated duplicate group must contain an occurrence");

        for (occurrence_index, occurrence) in group.occurrences.iter().enumerate() {
            if occurrence_index == canonical_index {
                continue;
            }

            mark_redundant_lines(corpus, &mut redundant, *occurrence);
        }
    }

    let duplicate_lines = redundant
        .iter()
        .map(|lines| lines.iter().filter(|&&is_redundant| is_redundant).count() as u64)
        .sum();

    let duplication_percent = if analyzed_lines == 0 {
        0.0
    } else {
        duplicate_lines as f64 / analyzed_lines as f64 * 100.0
    };

    Ok(Metrics {
        files,
        source_lines,
        analyzed_lines,
        duplicate_groups: groups.len() as u64,
        duplicate_lines,
        duplication_percent,
    })
}

fn physical_line_count(source: &str) -> u64 {
    source.lines().count() as u64
}

fn validate_group(
    corpus: &Corpus,
    group: &DuplicateGroup,
    group_index: usize,
) -> Result<(), MetricsError> {
    if group.occurrences.is_empty() {
        return Err(MetricsError::EmptyGroup { group: group_index });
    }

    let mut occurrences = group.occurrences.clone();
    occurrences.sort_unstable();

    for pair in occurrences.windows(2) {
        if pair[0] == pair[1] {
            return Err(MetricsError::DuplicateOccurrence {
                group: group_index,
                file: pair[0].file,
                start: pair[0].normalized_start,
            });
        }
    }

    for occurrence in &group.occurrences {
        if occurrence.normalized_len != group.normalized_len {
            return Err(MetricsError::LengthMismatch {
                group: group_index,
                group_length: group.normalized_len,
                occurrence_length: occurrence.normalized_len,
            });
        }

        validate_occurrence(corpus, *occurrence, group_index)?;
    }

    Ok(())
}

fn validate_occurrence(
    corpus: &Corpus,
    occurrence: Occurrence,
    group_index: usize,
) -> Result<(), MetricsError> {
    let Some(file) = corpus.files.get(occurrence.file as usize) else {
        return Err(MetricsError::UnknownFile {
            group: group_index,
            file: occurrence.file,
        });
    };

    let start = occurrence.normalized_start;
    let end = u64::from(start) + u64::from(occurrence.normalized_len);

    if occurrence.normalized_len == 0 || end > file.lines.len() as u64 {
        return Err(MetricsError::InvalidOccurrence {
            group: group_index,
            file: occurrence.file,
            start,
            end,
        });
    }

    Ok(())
}

fn mark_redundant_lines(corpus: &Corpus, redundant: &mut [Vec<bool>], occurrence: Occurrence) {
    let file_index = occurrence.file as usize;
    let file = &corpus.files[file_index];

    let start = occurrence.normalized_start as usize;
    let end = start + occurrence.normalized_len as usize;

    for (line, is_redundant) in file.lines[start..end]
        .iter()
        .zip(&mut redundant[file_index][start..end])
    {
        if line.effective {
            *is_redundant = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Range;
    use std::path::PathBuf;

    use crate::corpus::build_corpus;
    use crate::model::{NormalizedLine, NormalizedSegment, PreparedFile};

    use super::*;

    fn prepared(path: &str, lines: &[(&str, bool)]) -> PreparedFile {
        let mut source = String::new();
        let mut normalized = String::new();
        let mut normalized_lines = Vec::new();

        for (source_line, &(text, effective)) in lines.iter().enumerate() {
            source.push_str(text);
            source.push('\n');

            let start = normalized.len() as u32;
            normalized.push_str(text);
            let end = normalized.len() as u32;
            normalized.push('\n');

            normalized_lines.push(NormalizedLine {
                text_range: Range { start, end },
                source_line: source_line as u32,
                effective,
            });
        }

        let line_count = normalized_lines.len() as u32;

        PreparedFile {
            path: PathBuf::from(path),
            source,
            normalized,
            lines: normalized_lines,
            segments: if line_count == 0 {
                Vec::new()
            } else {
                vec![NormalizedSegment {
                    start: 0,
                    end: line_count,
                }]
            },
        }
    }

    fn occurrence(file: u32, start: u32, length: u32) -> Occurrence {
        Occurrence {
            file,
            normalized_start: start,
            normalized_len: length,
        }
    }

    fn group(length: u32, occurrences: Vec<Occurrence>) -> DuplicateGroup {
        DuplicateGroup {
            effective_lines: length,
            normalized_len: length,
            occurrences,
        }
    }

    #[test]
    fn empty_corpus_has_zero_metrics() {
        let corpus = build_corpus(Vec::new()).unwrap();

        let metrics = calculate_metrics(&corpus, &[]).unwrap();

        assert_eq!(
            metrics,
            Metrics {
                files: 0,
                source_lines: 0,
                analyzed_lines: 0,
                duplicate_groups: 0,
                duplicate_lines: 0,
                duplication_percent: 0.0,
            }
        );
    }

    #[test]
    fn counts_files_source_lines_and_analyzed_lines() {
        let corpus = build_corpus(vec![
            prepared("a.py", &[("alpha()", true), (")", false), ("beta()", true)]),
            prepared("b.py", &[("gamma()", true), ("delta()", true)]),
        ])
        .unwrap();

        let metrics = calculate_metrics(&corpus, &[]).unwrap();

        assert_eq!(metrics.files, 2);
        assert_eq!(metrics.source_lines, 5);
        assert_eq!(metrics.analyzed_lines, 4);
        assert_eq!(metrics.duplicate_groups, 0);
        assert_eq!(metrics.duplicate_lines, 0);
        assert_eq!(metrics.duplication_percent, 0.0);
    }

    #[test]
    fn counts_only_occurrences_beyond_canonical_copy() {
        let common = [("alpha()", true), ("beta()", true), ("gamma()", true)];

        let corpus =
            build_corpus(vec![prepared("a.py", &common), prepared("b.py", &common)]).unwrap();

        let groups = vec![group(3, vec![occurrence(0, 0, 3), occurrence(1, 0, 3)])];

        let metrics = calculate_metrics(&corpus, &groups).unwrap();

        assert_eq!(metrics.analyzed_lines, 6);
        assert_eq!(metrics.duplicate_groups, 1);
        assert_eq!(metrics.duplicate_lines, 3);
        assert_eq!(metrics.duplication_percent, 50.0);
    }

    #[test]
    fn three_occurrences_count_two_as_redundant() {
        let common = [
            ("alpha()", true),
            ("beta()", true),
            ("gamma()", true),
            ("delta()", true),
        ];

        let corpus = build_corpus(vec![
            prepared("a.py", &common),
            prepared("b.py", &common),
            prepared("c.py", &common),
        ])
        .unwrap();

        let groups = vec![group(
            4,
            vec![
                occurrence(0, 0, 4),
                occurrence(1, 0, 4),
                occurrence(2, 0, 4),
            ],
        )];

        let metrics = calculate_metrics(&corpus, &groups).unwrap();

        assert_eq!(metrics.analyzed_lines, 12);
        assert_eq!(metrics.duplicate_lines, 8);

        assert!((metrics.duplication_percent - 66.666_666_666_666_66).abs() < f64::EPSILON);
    }

    #[test]
    fn counts_only_effective_redundant_lines() {
        let lines = [
            ("value = (", true),
            ("calculate()", true),
            (")", false),
            ("]", false),
        ];

        let corpus =
            build_corpus(vec![prepared("a.py", &lines), prepared("b.py", &lines)]).unwrap();

        let groups = vec![DuplicateGroup {
            effective_lines: 2,
            normalized_len: 4,
            occurrences: vec![occurrence(0, 0, 4), occurrence(1, 0, 4)],
        }];

        let metrics = calculate_metrics(&corpus, &groups).unwrap();

        assert_eq!(metrics.analyzed_lines, 4);
        assert_eq!(metrics.duplicate_lines, 2);
        assert_eq!(metrics.duplication_percent, 50.0);
    }

    #[test]
    fn overlapping_groups_do_not_double_count_lines() {
        let lines = [
            ("a()", true),
            ("b()", true),
            ("c()", true),
            ("d()", true),
            ("e()", true),
            ("f()", true),
        ];

        let corpus =
            build_corpus(vec![prepared("a.py", &lines), prepared("b.py", &lines)]).unwrap();

        let groups = vec![
            group(4, vec![occurrence(0, 0, 4), occurrence(1, 0, 4)]),
            group(4, vec![occurrence(0, 2, 4), occurrence(1, 2, 4)]),
        ];

        let metrics = calculate_metrics(&corpus, &groups).unwrap();

        // b.py redundant ranges are:
        //
        // 0..4
        // 2..6
        //
        // Their union is 0..6, so all six lines are counted once.
        assert_eq!(metrics.analyzed_lines, 12);
        assert_eq!(metrics.duplicate_lines, 6);
        assert_eq!(metrics.duplication_percent, 50.0);
    }

    #[test]
    fn canonical_occurrence_is_smallest_source_location() {
        let common = [("alpha()", true), ("beta()", true)];

        let corpus = build_corpus(vec![
            prepared("a.py", &common),
            prepared("b.py", &common),
            prepared("c.py", &common),
        ])
        .unwrap();

        // Deliberately provide occurrences out of order.
        let groups = vec![group(
            2,
            vec![
                occurrence(2, 0, 2),
                occurrence(1, 0, 2),
                occurrence(0, 0, 2),
            ],
        )];

        let metrics = calculate_metrics(&corpus, &groups).unwrap();

        // a.py is canonical because FileId 0 corresponds to the first
        // sorted path. b.py and c.py are redundant.
        assert_eq!(metrics.duplicate_lines, 4);
    }

    #[test]
    fn rejects_group_without_occurrences() {
        let corpus = build_corpus(Vec::new()).unwrap();

        let groups = vec![DuplicateGroup {
            effective_lines: 4,
            normalized_len: 4,
            occurrences: Vec::new(),
        }];

        let error = calculate_metrics(&corpus, &groups).unwrap_err();

        assert!(matches!(error, MetricsError::EmptyGroup { group: 0 }));
    }

    #[test]
    fn rejects_unknown_file() {
        let corpus = build_corpus(vec![prepared("a.py", &[("alpha()", true)])]).unwrap();

        let groups = vec![group(1, vec![occurrence(0, 0, 1), occurrence(1, 0, 1)])];

        let error = calculate_metrics(&corpus, &groups).unwrap_err();

        assert!(matches!(
            error,
            MetricsError::UnknownFile { group: 0, file: 1 }
        ));
    }

    #[test]
    fn rejects_out_of_range_occurrence() {
        let corpus = build_corpus(vec![prepared(
            "a.py",
            &[("alpha()", true), ("beta()", true)],
        )])
        .unwrap();

        let groups = vec![group(2, vec![occurrence(0, 0, 2), occurrence(0, 1, 2)])];

        let error = calculate_metrics(&corpus, &groups).unwrap_err();

        assert!(matches!(
            error,
            MetricsError::InvalidOccurrence {
                group: 0,
                file: 0,
                start: 1,
                ..
            }
        ));
    }

    #[test]
    fn rejects_occurrence_length_mismatch() {
        let corpus = build_corpus(vec![prepared(
            "a.py",
            &[("alpha()", true), ("beta()", true)],
        )])
        .unwrap();

        let groups = vec![DuplicateGroup {
            effective_lines: 2,
            normalized_len: 2,
            occurrences: vec![occurrence(0, 0, 2), occurrence(0, 1, 1)],
        }];

        let error = calculate_metrics(&corpus, &groups).unwrap_err();

        assert!(matches!(
            error,
            MetricsError::LengthMismatch {
                group: 0,
                group_length: 2,
                occurrence_length: 1,
            }
        ));
    }

    #[test]
    fn rejects_duplicate_occurrence() {
        let corpus = build_corpus(vec![prepared(
            "a.py",
            &[("alpha()", true), ("beta()", true)],
        )])
        .unwrap();

        let duplicate = occurrence(0, 0, 2);

        let groups = vec![group(2, vec![duplicate, duplicate])];

        let error = calculate_metrics(&corpus, &groups).unwrap_err();

        assert!(matches!(
            error,
            MetricsError::DuplicateOccurrence {
                group: 0,
                file: 0,
                start: 0,
            }
        ));
    }
}
