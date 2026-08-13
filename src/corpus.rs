use std::collections::HashMap;
use std::path::PathBuf;

use thiserror::Error;

use crate::model::{CorpusPos, FileId, LineId, NormalizedLine, PreparedFile};

pub type CorpusToken = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorpusLocation {
    pub file: FileId,
    pub normalized_line: u32,
}

#[derive(Debug)]
pub struct Corpus {
    /// Prepared files in deterministic path order.
    pub files: Vec<PreparedFile>,

    /// Interned normalized-line IDs plus unique segment sentinels.
    pub tokens: Vec<CorpusToken>,

    /// Source location for each corpus position.
    ///
    /// Sentinel positions contain `None`.
    pub positions: Vec<Option<CorpusLocation>>,

    /// Number of distinct normalized source lines.
    ///
    /// Actual line tokens occupy `0..line_id_count`.
    pub line_id_count: u32,

    /// Number of unique segment sentinels in the corpus.
    pub segment_count: u32,
}

impl Corpus {
    #[must_use]
    pub fn source_at(&self, position: CorpusPos) -> Option<CorpusLocation> {
        self.positions.get(position as usize).copied().flatten()
    }

    #[must_use]
    pub fn is_sentinel_token(&self, token: CorpusToken) -> bool {
        token >= self.line_id_count
    }
}

#[derive(Debug, Error)]
pub enum CorpusError {
    #[error("duplicate prepared file path: {0:?}")]
    DuplicatePath(PathBuf),

    #[error(
        "{path:?}: normalized segments do not form an exact partition of \
         {line_count} normalized lines"
    )]
    InvalidSegments { path: PathBuf, line_count: usize },

    #[error(
        "{path:?}: normalized line range {start}..{end} is outside the \
         normalized source buffer"
    )]
    InvalidLineRange { path: PathBuf, start: u32, end: u32 },

    #[error("corpus exceeds u32 indexing capacity")]
    TooLarge,
}

pub fn build_corpus(mut files: Vec<PreparedFile>) -> Result<Corpus, CorpusError> {
    files.sort_by(|left, right| left.path.cmp(&right.path));

    reject_duplicate_paths(&files)?;

    for file in &files {
        validate_segments(file)?;
    }

    let total_segments = files.iter().try_fold(0_usize, |total, file| {
        total
            .checked_add(file.segments.len())
            .ok_or(CorpusError::TooLarge)
    })?;

    let total_lines = files.iter().try_fold(0_usize, |total, file| {
        total
            .checked_add(file.lines.len())
            .ok_or(CorpusError::TooLarge)
    })?;

    let total_tokens = total_lines
        .checked_add(total_segments)
        .ok_or(CorpusError::TooLarge)?;

    if total_tokens > u32::MAX as usize {
        return Err(CorpusError::TooLarge);
    }

    let mut interned: HashMap<&str, LineId> = HashMap::new();
    let mut file_line_ids = Vec::with_capacity(files.len());

    // First pass:
    //
    // Assign deterministic LineId values in sorted path order and then
    // normalized-line order. HashMap iteration order never participates in
    // the assignment.
    for file in &files {
        let mut line_ids = Vec::with_capacity(file.lines.len());

        for line in &file.lines {
            let text = normalized_line_text(file, line)?;

            let id = if let Some(&existing) = interned.get(text) {
                existing
            } else {
                let id = u32::try_from(interned.len()).map_err(|_| CorpusError::TooLarge)?;

                interned.insert(text, id);
                id
            };

            line_ids.push(id);
        }

        file_line_ids.push(line_ids);
    }

    let line_id_count = u32::try_from(interned.len()).map_err(|_| CorpusError::TooLarge)?;

    let segment_count = u32::try_from(total_segments).map_err(|_| CorpusError::TooLarge)?;

    // `interned` borrows normalized strings owned by `files`. We no longer
    // need the map after the deterministic LineId assignments are complete.
    drop(interned);

    validate_token_space(line_id_count, segment_count)?;

    let mut tokens = Vec::with_capacity(total_tokens);
    let mut positions = Vec::with_capacity(total_tokens);

    let mut sentinel_ordinal = 0_u32;

    // Second pass:
    //
    // Build the global corpus. Each normalized segment receives its own
    // unique sentinel token so no suffix match can cross a file or
    // suppression boundary.
    for (file_index, file) in files.iter().enumerate() {
        let file_id = u32::try_from(file_index).map_err(|_| CorpusError::TooLarge)?;

        let line_ids = &file_line_ids[file_index];

        for segment in &file.segments {
            for normalized_line in segment.start..segment.end {
                let line_index = normalized_line as usize;

                tokens.push(line_ids[line_index]);

                positions.push(Some(CorpusLocation {
                    file: file_id,
                    normalized_line,
                }));
            }

            let sentinel = line_id_count
                .checked_add(sentinel_ordinal)
                .ok_or(CorpusError::TooLarge)?;

            tokens.push(sentinel);
            positions.push(None);

            sentinel_ordinal += 1;
        }
    }

    debug_assert_eq!(tokens.len(), positions.len());
    debug_assert_eq!(sentinel_ordinal, segment_count);

    Ok(Corpus {
        files,
        tokens,
        positions,
        line_id_count,
        segment_count,
    })
}

fn reject_duplicate_paths(files: &[PreparedFile]) -> Result<(), CorpusError> {
    for pair in files.windows(2) {
        if pair[0].path == pair[1].path {
            return Err(CorpusError::DuplicatePath(pair[0].path.clone()));
        }
    }

    Ok(())
}

fn validate_segments(file: &PreparedFile) -> Result<(), CorpusError> {
    let line_count = u32::try_from(file.lines.len()).map_err(|_| CorpusError::TooLarge)?;

    if line_count == 0 {
        if file.segments.is_empty() {
            return Ok(());
        }

        return Err(CorpusError::InvalidSegments {
            path: file.path.clone(),
            line_count: file.lines.len(),
        });
    }

    if file.segments.is_empty() {
        return Err(CorpusError::InvalidSegments {
            path: file.path.clone(),
            line_count: file.lines.len(),
        });
    }

    let mut expected_start = 0_u32;

    for segment in &file.segments {
        if segment.start != expected_start
            || segment.start >= segment.end
            || segment.end > line_count
        {
            return Err(CorpusError::InvalidSegments {
                path: file.path.clone(),
                line_count: file.lines.len(),
            });
        }

        expected_start = segment.end;
    }

    if expected_start != line_count {
        return Err(CorpusError::InvalidSegments {
            path: file.path.clone(),
            line_count: file.lines.len(),
        });
    }

    Ok(())
}

fn normalized_line_text<'a>(
    file: &'a PreparedFile,
    line: &NormalizedLine,
) -> Result<&'a str, CorpusError> {
    file.normalized
        .get(line.text_range.start as usize..line.text_range.end as usize)
        .ok_or_else(|| CorpusError::InvalidLineRange {
            path: file.path.clone(),
            start: line.text_range.start,
            end: line.text_range.end,
        })
}

fn validate_token_space(line_id_count: u32, segment_count: u32) -> Result<(), CorpusError> {
    if segment_count == 0 {
        return Ok(());
    }

    let last_sentinel = u64::from(line_id_count) + u64::from(segment_count) - 1;

    if last_sentinel > u64::from(u32::MAX) {
        return Err(CorpusError::TooLarge);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::ops::Range;
    use std::path::PathBuf;

    use crate::model::{NormalizedLine, NormalizedSegment};

    use super::*;

    fn prepared(path: &str, text_lines: &[&str], segments: &[(u32, u32)]) -> PreparedFile {
        let mut normalized = String::new();
        let mut lines = Vec::new();

        for (source_line, text) in text_lines.iter().enumerate() {
            let start = normalized.len() as u32;
            normalized.push_str(text);
            let end = normalized.len() as u32;
            normalized.push('\n');

            lines.push(NormalizedLine {
                text_range: Range { start, end },
                source_line: source_line as u32,
                effective: true,
            });
        }

        PreparedFile {
            path: PathBuf::from(path),
            source: String::new(),
            normalized,
            lines,
            segments: segments
                .iter()
                .map(|&(start, end)| NormalizedSegment { start, end })
                .collect(),
        }
    }

    #[test]
    fn interns_identical_lines_once() {
        let corpus = build_corpus(vec![
            prepared("a.py", &["same()", "first()"], &[(0, 2)]),
            prepared("b.py", &["same()", "second()"], &[(0, 2)]),
        ])
        .unwrap();

        assert_eq!(corpus.line_id_count, 3);

        // a.py:
        //   same()  -> 0
        //   first() -> 1
        //
        // b.py:
        //   same()   -> 0
        //   second() -> 2
        //
        // sentinels -> 3, 4
        assert_eq!(corpus.tokens, vec![0, 1, 3, 0, 2, 4,]);
    }

    #[test]
    fn assigns_unique_sentinels_to_each_segment() {
        let corpus = build_corpus(vec![prepared(
            "a.py",
            &["one()", "two()", "three()", "four()"],
            &[(0, 2), (2, 4)],
        )])
        .unwrap();

        assert_eq!(corpus.line_id_count, 4);
        assert_eq!(corpus.segment_count, 2);

        assert_eq!(corpus.tokens, vec![0, 1, 4, 2, 3, 5,]);

        assert!(corpus.is_sentinel_token(4));
        assert!(corpus.is_sentinel_token(5));
        assert!(!corpus.is_sentinel_token(3));
    }

    #[test]
    fn sentinel_positions_have_no_source_location() {
        let corpus = build_corpus(vec![prepared("a.py", &["one()", "two()"], &[(0, 2)])]).unwrap();

        assert_eq!(
            corpus.source_at(0),
            Some(CorpusLocation {
                file: 0,
                normalized_line: 0,
            })
        );

        assert_eq!(
            corpus.source_at(1),
            Some(CorpusLocation {
                file: 0,
                normalized_line: 1,
            })
        );

        assert_eq!(corpus.source_at(2), None);
    }

    #[test]
    fn sorts_files_before_assigning_ids() {
        let first = prepared("b.py", &["same()", "from_b()"], &[(0, 2)]);

        let second = prepared("a.py", &["same()", "from_a()"], &[(0, 2)]);

        let forward = build_corpus(vec![first.clone(), second.clone()]).unwrap();

        let reversed = build_corpus(vec![second, first]).unwrap();

        assert_eq!(forward.tokens, reversed.tokens);
        assert_eq!(forward.positions, reversed.positions);

        assert_eq!(forward.files[0].path, PathBuf::from("a.py"));

        assert_eq!(forward.files[1].path, PathBuf::from("b.py"));

        assert_eq!(forward.tokens, vec![0, 1, 3, 0, 2, 4,]);
    }

    #[test]
    fn preserves_normalized_line_mapping_across_segments() {
        let corpus = build_corpus(vec![prepared(
            "a.py",
            &["one()", "two()", "three()", "four()"],
            &[(0, 2), (2, 4)],
        )])
        .unwrap();

        assert_eq!(
            corpus.positions,
            vec![
                Some(CorpusLocation {
                    file: 0,
                    normalized_line: 0,
                }),
                Some(CorpusLocation {
                    file: 0,
                    normalized_line: 1,
                }),
                None,
                Some(CorpusLocation {
                    file: 0,
                    normalized_line: 2,
                }),
                Some(CorpusLocation {
                    file: 0,
                    normalized_line: 3,
                }),
                None,
            ]
        );
    }

    #[test]
    fn rejects_duplicate_paths() {
        let result = build_corpus(vec![
            prepared("same.py", &["one()"], &[(0, 1)]),
            prepared("same.py", &["two()"], &[(0, 1)]),
        ]);

        assert!(matches!(result, Err(CorpusError::DuplicatePath(_))));
    }

    #[test]
    fn rejects_segments_that_do_not_partition_lines() {
        let result = build_corpus(vec![prepared(
            "a.py",
            &["one()", "two()", "three()"],
            &[(0, 1), (2, 3)],
        )]);

        assert!(matches!(result, Err(CorpusError::InvalidSegments { .. })));
    }

    #[test]
    fn accepts_empty_prepared_files() {
        let corpus = build_corpus(vec![prepared("empty.py", &[], &[])]).unwrap();

        assert!(corpus.tokens.is_empty());
        assert!(corpus.positions.is_empty());
        assert_eq!(corpus.line_id_count, 0);
        assert_eq!(corpus.segment_count, 0);
    }
}
