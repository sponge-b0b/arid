use thiserror::Error;

use crate::corpus::Corpus;
use crate::model::{CorpusPos, DuplicateGroup, Occurrence};
use crate::suffix::{
    build_lcp_array, build_suffix_array, SuffixError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetectionOptions {
    pub min_lines: u32,
    pub same_file: bool,
}

impl Default for DetectionOptions {
    fn default() -> Self {
        Self {
            min_lines: 4,
            same_file: true,
        }
    }
}

#[derive(Debug, Error)]
pub enum DetectError {
    #[error(transparent)]
    Suffix(#[from] SuffixError),

    #[error("min-lines must be at least 1")]
    InvalidMinLines,

    #[error(
        "corpus position {position} with length {length} does not map to a \
         contiguous normalized source range"
    )]
    InvalidOccurrence {
        position: CorpusPos,
        length: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LcpInterval {
    left: usize,
    right: usize,
    length: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CandidateOccurrence {
    corpus_start: CorpusPos,
    occurrence: Occurrence,
}

/// Detects duplicate normalized source blocks in `corpus`.
///
/// Detection is exact over the normalized line-token corpus. The function:
///
/// 1. builds the suffix array,
/// 2. builds the LCP array,
/// 3. extracts repeated LCP intervals,
/// 4. maps them back to normalized source locations,
/// 5. applies duplicate qualification and grouping rules.
pub fn detect_duplicates(
    corpus: &Corpus,
    options: DetectionOptions,
) -> Result<Vec<DuplicateGroup>, DetectError> {
    if options.min_lines == 0 {
        return Err(DetectError::InvalidMinLines);
    }

    let suffixes = build_suffix_array(&corpus.tokens)?;
    let lcp = build_lcp_array(&corpus.tokens, &suffixes)?;

    extract_duplicates(corpus, &suffixes, &lcp, options)
}

fn extract_duplicates(
    corpus: &Corpus,
    suffixes: &[CorpusPos],
    lcp: &[u32],
    options: DetectionOptions,
) -> Result<Vec<DuplicateGroup>, DetectError> {
    let intervals = lcp_intervals(lcp, options.min_lines);
    let mut groups = Vec::new();

    for interval in intervals {
        let mut occurrences =
            Vec::with_capacity(interval.right - interval.left + 1);

        for &suffix in &suffixes[interval.left..=interval.right] {
            occurrences.push(candidate_occurrence(
                corpus,
                suffix,
                interval.length,
            )?);
        }

        let occurrences = remove_same_file_overlaps(occurrences);

        if occurrences.len() < 2 {
            continue;
        }

        if !options.same_file && !spans_multiple_files(&occurrences) {
            continue;
        }

        let effective_lines =
            effective_line_count(corpus, occurrences[0].occurrence)?;

        if effective_lines < options.min_lines {
            continue;
        }

        // A maximal repeated block cannot be extended identically to the
        // left or right across every valid occurrence.
        if all_extend_left(corpus, &occurrences)
            || all_extend_right(corpus, &occurrences, interval.length)
        {
            continue;
        }

        let occurrences = occurrences
            .into_iter()
            .map(|candidate| candidate.occurrence)
            .collect();

        groups.push(DuplicateGroup {
            effective_lines,
            normalized_len: interval.length,
            occurrences,
        });
    }

    Ok(canonicalize_groups(groups))
}

/// Returns all non-zero LCP intervals whose physical normalized length can
/// possibly satisfy `min_lines`.
///
/// Each interval identifies a suffix-array range where every suffix shares
/// at least `length` leading normalized-line tokens.
fn lcp_intervals(
    lcp: &[u32],
    min_lines: u32,
) -> Vec<LcpInterval> {
    if lcp.len() < 2 {
        return Vec::new();
    }

    let mut intervals = Vec::new();
    let mut stack: Vec<(usize, u32)> = Vec::new();

    // `lcp[i]` describes suffixes i-1 and i, so traversal begins at 1.
    // One synthetic zero at the end flushes every remaining interval.
    for index in 1..=lcp.len() {
        let current = if index < lcp.len() {
            lcp[index]
        } else {
            0
        };

        let mut left = index - 1;

        while let Some(&(interval_left, height)) = stack.last() {
            if height <= current {
                break;
            }

            stack.pop();

            if height >= min_lines {
                intervals.push(LcpInterval {
                    left: interval_left,
                    right: index - 1,
                    length: height,
                });
            }

            left = interval_left;
        }

        let previous_height =
            stack.last().map_or(0, |&(_, height)| height);

        if current > previous_height {
            stack.push((left, current));
        }
    }

    intervals
}

fn candidate_occurrence(
    corpus: &Corpus,
    position: CorpusPos,
    length: u32,
) -> Result<CandidateOccurrence, DetectError> {
    let Some(start) = corpus.source_at(position) else {
        return Err(DetectError::InvalidOccurrence {
            position,
            length,
        });
    };

    let Some(last_offset) = length.checked_sub(1) else {
        return Err(DetectError::InvalidOccurrence {
            position,
            length,
        });
    };

    let Some(end_position) = position.checked_add(last_offset) else {
        return Err(DetectError::InvalidOccurrence {
            position,
            length,
        });
    };

    let Some(end) = corpus.source_at(end_position) else {
        return Err(DetectError::InvalidOccurrence {
            position,
            length,
        });
    };

    let Some(expected_end_line) =
        start.normalized_line.checked_add(last_offset)
    else {
        return Err(DetectError::InvalidOccurrence {
            position,
            length,
        });
    };

    if start.file != end.file
        || end.normalized_line != expected_end_line
    {
        return Err(DetectError::InvalidOccurrence {
            position,
            length,
        });
    }

    Ok(CandidateOccurrence {
        corpus_start: position,
        occurrence: Occurrence {
            file: start.file,
            normalized_start: start.normalized_line,
            normalized_len: length,
        },
    })
}

/// Removes overlapping occurrences within the same file.
///
/// For equal-length intervals, keeping the earliest start is optimal: it ends
/// no later than any overlapping alternative and therefore leaves the most
/// room for subsequent non-overlapping occurrences.
fn remove_same_file_overlaps(
    mut occurrences: Vec<CandidateOccurrence>,
) -> Vec<CandidateOccurrence> {
    occurrences.sort_by_key(|candidate| {
        (
            candidate.occurrence.file,
            candidate.occurrence.normalized_start,
            candidate.corpus_start,
        )
    });

    let mut kept = Vec::with_capacity(occurrences.len());
    let mut current_file = None;
    let mut last_end = 0_u64;

    for candidate in occurrences {
        let occurrence = candidate.occurrence;
        let start = u64::from(occurrence.normalized_start);
        let end = start + u64::from(occurrence.normalized_len);

        if current_file == Some(occurrence.file) {
            if start < last_end {
                continue;
            }
        } else {
            current_file = Some(occurrence.file);
        }

        last_end = end;
        kept.push(candidate);
    }

    kept
}

fn spans_multiple_files(
    occurrences: &[CandidateOccurrence],
) -> bool {
    let Some(first) = occurrences.first() else {
        return false;
    };

    occurrences
        .iter()
        .skip(1)
        .any(|candidate| {
            candidate.occurrence.file != first.occurrence.file
        })
}

fn effective_line_count(
    corpus: &Corpus,
    occurrence: Occurrence,
) -> Result<u32, DetectError> {
    let Some(file) = corpus.files.get(occurrence.file as usize) else {
        return Err(DetectError::InvalidOccurrence {
            position: 0,
            length: occurrence.normalized_len,
        });
    };

    let start = occurrence.normalized_start as usize;
    let Some(end) = start.checked_add(occurrence.normalized_len as usize)
    else {
        return Err(DetectError::InvalidOccurrence {
            position: 0,
            length: occurrence.normalized_len,
        });
    };

    let Some(lines) = file.lines.get(start..end) else {
        return Err(DetectError::InvalidOccurrence {
            position: 0,
            length: occurrence.normalized_len,
        });
    };

    u32::try_from(
        lines.iter().filter(|line| line.effective).count(),
    )
    .map_err(|_| DetectError::InvalidOccurrence {
        position: 0,
        length: occurrence.normalized_len,
    })
}

/// Returns true when every occurrence has the same real normalized-line token
/// immediately before it.
///
/// A file/segment boundary prevents extension because its preceding token is
/// either absent or a unique sentinel.
fn all_extend_left(
    corpus: &Corpus,
    occurrences: &[CandidateOccurrence],
) -> bool {
    let mut expected = None;

    for candidate in occurrences {
        if candidate.corpus_start == 0 {
            return false;
        }

        let previous_position =
            candidate.corpus_start as usize - 1;

        let token = corpus.tokens[previous_position];

        if corpus.is_sentinel_token(token) {
            return false;
        }

        match expected {
            Some(expected_token) if expected_token != token => {
                return false;
            }
            None => {
                expected = Some(token);
            }
            _ => {}
        }
    }

    expected.is_some()
}

/// Returns true when every occurrence has the same real normalized-line token
/// immediately after it.
///
/// A file/segment boundary prevents extension because the following token is
/// either absent or a unique sentinel.
fn all_extend_right(
    corpus: &Corpus,
    occurrences: &[CandidateOccurrence],
    length: u32,
) -> bool {
    let mut expected = None;

    for candidate in occurrences {
        let next_position =
            candidate.corpus_start as usize + length as usize;

        let Some(&token) = corpus.tokens.get(next_position) else {
            return false;
        };

        if corpus.is_sentinel_token(token) {
            return false;
        }

        match expected {
            Some(expected_token) if expected_token != token => {
                return false;
            }
            None => {
                expected = Some(token);
            }
            _ => {}
        }
    }

    expected.is_some()
}

/// Deduplicates equivalent groups and suppresses a shorter group only when
/// every occurrence is already represented inside one larger group.
///
/// This intentionally preserves useful nested groups when the shorter block
/// introduces an additional occurrence.
///
/// Example:
///
/// A and B share 10 lines.
/// A, B, and C share the first 6 lines.
///
/// Both groups remain because the 6-line group adds C.
fn canonicalize_groups(
    mut groups: Vec<DuplicateGroup>,
) -> Vec<DuplicateGroup> {
    // Process larger groups first so contained-match suppression can compare
    // each candidate only against groups already accepted.
    groups.sort_by(|left, right| {
        right
            .normalized_len
            .cmp(&left.normalized_len)
            .then_with(|| {
                right.effective_lines.cmp(&left.effective_lines)
            })
            .then_with(|| {
                left.occurrences.cmp(&right.occurrences)
            })
    });

    groups.dedup();

    let mut kept: Vec<DuplicateGroup> = Vec::new();

    for group in groups {
        if kept
            .iter()
            .any(|larger| group_is_contained(larger, &group))
        {
            continue;
        }

        kept.push(group);
    }

    // Detection output itself is deterministic and source-oriented even
    // before the reporting layer applies its final presentation ordering.
    kept.sort_by(|left, right| {
        left.occurrences[0]
            .cmp(&right.occurrences[0])
            .then_with(|| {
                right.normalized_len.cmp(&left.normalized_len)
            })
            .then_with(|| {
                left.occurrences.cmp(&right.occurrences)
            })
    });

    kept
}

fn group_is_contained(
    larger: &DuplicateGroup,
    smaller: &DuplicateGroup,
) -> bool {
    if larger.normalized_len <= smaller.normalized_len {
        return false;
    }

    smaller.occurrences.iter().all(|candidate| {
        larger
            .occurrences
            .iter()
            .any(|container| {
                occurrence_contains(*container, *candidate)
            })
    })
}

fn occurrence_contains(
    container: Occurrence,
    candidate: Occurrence,
) -> bool {
    if container.file != candidate.file {
        return false;
    }

    let container_start =
        u64::from(container.normalized_start);
    let container_end =
        container_start + u64::from(container.normalized_len);

    let candidate_start =
        u64::from(candidate.normalized_start);
    let candidate_end =
        candidate_start + u64::from(candidate.normalized_len);

    container_start <= candidate_start
        && container_end >= candidate_end
}

#[cfg(test)]
mod tests {
    use std::ops::Range;
    use std::path::PathBuf;

    use crate::corpus::build_corpus;
    use crate::model::{
        NormalizedLine, NormalizedSegment, PreparedFile,
    };

    use super::*;

    fn prepared(
        path: &str,
        text_lines: &[(&str, bool)],
        segments: &[(u32, u32)],
    ) -> PreparedFile {
        let mut normalized = String::new();
        let mut lines = Vec::new();

        for (source_line, &(text, effective)) in
            text_lines.iter().enumerate()
        {
            let start = normalized.len() as u32;
            normalized.push_str(text);
            let end = normalized.len() as u32;
            normalized.push('\n');

            lines.push(NormalizedLine {
                text_range: Range { start, end },
                source_line: source_line as u32,
                effective,
            });
        }

        PreparedFile {
            path: PathBuf::from(path),
            source: String::new(),
            normalized,
            lines,
            segments: segments
                .iter()
                .map(|&(start, end)| NormalizedSegment {
                    start,
                    end,
                })
                .collect(),
        }
    }

    fn effective_lines<'a>(lines: &[&'a str]) -> Vec<(&'a str, bool)> {
        lines.iter().map(|&line| (line, true)).collect()
    }

    fn detect(
        files: Vec<PreparedFile>,
        min_lines: u32,
        same_file: bool,
    ) -> Vec<DuplicateGroup> {
        let corpus = build_corpus(files).unwrap();

        detect_duplicates(
            &corpus,
            DetectionOptions {
                min_lines,
                same_file,
            },
        )
        .unwrap()
    }

    #[test]
    fn detects_cross_file_duplicate_block() {
        let first_lines = effective_lines(&[
            "alpha()",
            "beta()",
            "gamma()",
            "delta()",
            "only_a()",
        ]);

        let second_lines = effective_lines(&[
            "alpha()",
            "beta()",
            "gamma()",
            "delta()",
            "only_b()",
        ]);

        let groups = detect(
            vec![
                prepared(
                    "a.py",
                    &first_lines,
                    &[(0, 5)],
                ),
                prepared(
                    "b.py",
                    &second_lines,
                    &[(0, 5)],
                ),
            ],
            4,
            true,
        );

        assert_eq!(groups.len(), 1);

        assert_eq!(
            groups[0],
            DuplicateGroup {
                effective_lines: 4,
                normalized_len: 4,
                occurrences: vec![
                    Occurrence {
                        file: 0,
                        normalized_start: 0,
                        normalized_len: 4,
                    },
                    Occurrence {
                        file: 1,
                        normalized_start: 0,
                        normalized_len: 4,
                    },
                ],
            }
        );
    }

    #[test]
    fn groups_three_occurrences_together() {
        let common = [
            "alpha()",
            "beta()",
            "gamma()",
            "delta()",
        ];

        let mut first = effective_lines(&common);
        first.push(("only_a()", true));

        let mut second = effective_lines(&common);
        second.push(("only_b()", true));

        let mut third = effective_lines(&common);
        third.push(("only_c()", true));

        let groups = detect(
            vec![
                prepared("a.py", &first, &[(0, 5)]),
                prepared("b.py", &second, &[(0, 5)]),
                prepared("c.py", &third, &[(0, 5)]),
            ],
            4,
            true,
        );

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].normalized_len, 4);
        assert_eq!(groups[0].effective_lines, 4);
        assert_eq!(groups[0].occurrences.len(), 3);
    }

    #[test]
    fn detects_non_overlapping_same_file_duplicates() {
        let lines = effective_lines(&[
            "alpha()",
            "beta()",
            "gamma()",
            "delta()",
            "gap()",
            "alpha()",
            "beta()",
            "gamma()",
            "delta()",
        ]);

        let groups = detect(
            vec![prepared(
                "a.py",
                &lines,
                &[(0, 9)],
            )],
            4,
            true,
        );

        assert_eq!(groups.len(), 1);

        assert_eq!(
            groups[0].occurrences,
            vec![
                Occurrence {
                    file: 0,
                    normalized_start: 0,
                    normalized_len: 4,
                },
                Occurrence {
                    file: 0,
                    normalized_start: 5,
                    normalized_len: 4,
                },
            ]
        );
    }

    #[test]
    fn can_disable_same_file_only_detection() {
        let lines = effective_lines(&[
            "alpha()",
            "beta()",
            "gamma()",
            "delta()",
            "gap()",
            "alpha()",
            "beta()",
            "gamma()",
            "delta()",
        ]);

        let groups = detect(
            vec![prepared(
                "a.py",
                &lines,
                &[(0, 9)],
            )],
            4,
            false,
        );

        assert!(groups.is_empty());
    }

    #[test]
    fn qualifies_by_effective_lines_not_physical_length() {
        let lines = [
            ("(", false),
            ("calculate()", true),
            (")", false),
            ("return value", true),
        ];

        let files = vec![
            prepared("a.py", &lines, &[(0, 4)]),
            prepared("b.py", &lines, &[(0, 4)]),
        ];

        let too_strict = detect(
            files.clone(),
            3,
            true,
        );

        assert!(too_strict.is_empty());

        let accepted = detect(
            files,
            2,
            true,
        );

        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].normalized_len, 4);
        assert_eq!(accepted[0].effective_lines, 2);
    }

    #[test]
    fn does_not_match_across_segment_boundaries() {
        let first = effective_lines(&[
            "alpha()",
            "beta()",
            "gamma()",
            "delta()",
        ]);

        let second = first.clone();

        let groups = detect(
            vec![
                prepared(
                    "a.py",
                    &first,
                    &[(0, 2), (2, 4)],
                ),
                prepared(
                    "b.py",
                    &second,
                    &[(0, 4)],
                ),
            ],
            4,
            true,
        );

        assert!(groups.is_empty());
    }

    #[test]
    fn preserves_shorter_group_when_it_adds_an_occurrence() {
        let first = effective_lines(&[
            "alpha()",
            "beta()",
            "gamma()",
            "delta()",
            "only_a()",
        ]);

        let second = effective_lines(&[
            "alpha()",
            "beta()",
            "gamma()",
            "delta()",
            "only_b()",
        ]);

        let third = effective_lines(&[
            "alpha()",
            "beta()",
            "gamma()",
            "different()",
            "only_c()",
        ]);

        let groups = detect(
            vec![
                prepared("a.py", &first, &[(0, 5)]),
                prepared("b.py", &second, &[(0, 5)]),
                prepared("c.py", &third, &[(0, 5)]),
            ],
            3,
            true,
        );

        assert_eq!(groups.len(), 2);

        let lengths: Vec<u32> = groups
            .iter()
            .map(|group| group.normalized_len)
            .collect();

        assert!(lengths.contains(&4));
        assert!(lengths.contains(&3));

        let shorter = groups
            .iter()
            .find(|group| group.normalized_len == 3)
            .unwrap();

        assert_eq!(shorter.occurrences.len(), 3);
    }

    #[test]
    fn suppresses_fully_contained_repetitive_group() {
        let lines = effective_lines(&[
            "same()",
            "same()",
            "same()",
            "same()",
        ]);

        let groups = detect(
            vec![prepared(
                "a.py",
                &lines,
                &[(0, 4)],
            )],
            1,
            true,
        );

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].normalized_len, 2);

        assert_eq!(
            groups[0].occurrences,
            vec![
                Occurrence {
                    file: 0,
                    normalized_start: 0,
                    normalized_len: 2,
                },
                Occurrence {
                    file: 0,
                    normalized_start: 2,
                    normalized_len: 2,
                },
            ]
        );
    }

    #[test]
    fn rejects_zero_min_lines() {
        let corpus = build_corpus(Vec::new()).unwrap();

        let error = detect_duplicates(
            &corpus,
            DetectionOptions {
                min_lines: 0,
                same_file: true,
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            DetectError::InvalidMinLines
        ));
    }

    #[test]
    fn detection_is_independent_of_input_file_order() {
        let first = prepared(
            "b.py",
            &effective_lines(&[
                "alpha()",
                "beta()",
                "gamma()",
                "delta()",
            ]),
            &[(0, 4)],
        );

        let second = prepared(
            "a.py",
            &effective_lines(&[
                "alpha()",
                "beta()",
                "gamma()",
                "delta()",
            ]),
            &[(0, 4)],
        );

        let forward = detect(
            vec![first.clone(), second.clone()],
            4,
            true,
        );

        let reversed = detect(
            vec![second, first],
            4,
            true,
        );

        assert_eq!(forward, reversed);
    }
}