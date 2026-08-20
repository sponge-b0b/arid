use std::fmt::Write as _;
use std::path::PathBuf;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::corpus::Corpus;
use crate::model::{DuplicateGroup, NormalizedLine, Occurrence, PreparedFile};

const FINDING_FINGERPRINT_DOMAIN: &[u8] = b"arid-finding-v1\0";
const FINDING_FINGERPRINT_PREFIX: &str = "arid-finding-v1:sha256:";

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum FingerprintError {
    #[error("duplicate group has no occurrences")]
    EmptyGroup,

    #[error("duplicate group references unknown file id {file}")]
    InvalidFile { file: u32 },

    #[error(
        "{path:?}: duplicate occurrence start {start} with length {length} \
         is outside normalized source lines"
    )]
    InvalidOccurrenceRange {
        path: PathBuf,
        start: u32,
        length: u32,
    },

    #[error(
        "{path:?}: normalized line range {start}..{end} is outside the \
         normalized source buffer"
    )]
    InvalidLineRange { path: PathBuf, start: u32, end: u32 },

    #[error("normalized line exceeds u32 byte-length capacity")]
    LineTooLong,
}

/// Returns the stable v1 identity of one exact normalized duplicate block.
///
/// Identity includes only the normalized block contract. It intentionally
/// excludes paths, physical line numbers, occurrence order/multiplicity,
/// structural metadata, output format, and worker behavior.
pub(crate) fn finding_fingerprint(
    corpus: &Corpus,
    group: &DuplicateGroup,
) -> Result<String, FingerprintError> {
    let occurrence = group
        .occurrences
        .first()
        .copied()
        .ok_or(FingerprintError::EmptyGroup)?;

    let (file, lines) = occurrence_lines(corpus, group, occurrence)?;
    let mut hasher = Sha256::new();

    hasher.update(FINDING_FINGERPRINT_DOMAIN);
    hasher.update(group.effective_lines.to_be_bytes());
    hasher.update(group.normalized_len.to_be_bytes());

    for line in lines {
        let text = file
            .normalized
            .get(line.text_range.start as usize..line.text_range.end as usize)
            .ok_or_else(|| FingerprintError::InvalidLineRange {
                path: file.path.clone(),
                start: line.text_range.start,
                end: line.text_range.end,
            })?;

        let length = u32::try_from(text.len()).map_err(|_| FingerprintError::LineTooLong)?;

        hasher.update(length.to_be_bytes());
        hasher.update(text.as_bytes());
    }

    let digest = hasher.finalize();
    let mut fingerprint = String::with_capacity(FINDING_FINGERPRINT_PREFIX.len() + digest.len() * 2);

    fingerprint.push_str(FINDING_FINGERPRINT_PREFIX);

    for byte in digest {
        write!(&mut fingerprint, "{byte:02x}").expect("writing to String cannot fail");
    }

    Ok(fingerprint)
}

fn occurrence_lines<'a>(
    corpus: &'a Corpus,
    group: &DuplicateGroup,
    occurrence: Occurrence,
) -> Result<(&'a PreparedFile, &'a [NormalizedLine]), FingerprintError> {
    let file = corpus
        .files
        .get(occurrence.file as usize)
        .ok_or(FingerprintError::InvalidFile {
            file: occurrence.file,
        })?;

    if occurrence.normalized_len != group.normalized_len {
        return Err(FingerprintError::InvalidOccurrenceRange {
            path: file.path.clone(),
            start: occurrence.normalized_start,
            length: occurrence.normalized_len,
        });
    }

    let start = occurrence.normalized_start as usize;
    let end = start
        .checked_add(occurrence.normalized_len as usize)
        .ok_or_else(|| FingerprintError::InvalidOccurrenceRange {
            path: file.path.clone(),
            start: occurrence.normalized_start,
            length: occurrence.normalized_len,
        })?;

    let lines =
        file.lines
            .get(start..end)
            .ok_or_else(|| FingerprintError::InvalidOccurrenceRange {
                path: file.path.clone(),
                start: occurrence.normalized_start,
                length: occurrence.normalized_len,
            })?;

    if lines.is_empty() {
        return Err(FingerprintError::InvalidOccurrenceRange {
            path: file.path.clone(),
            start: occurrence.normalized_start,
            length: occurrence.normalized_len,
        });
    }

    Ok((file, lines))
}

#[cfg(test)]
mod tests {
    use std::ops::Range;
    use std::path::PathBuf;

    use crate::corpus::build_corpus;
    use crate::model::{
        NormalizedSegment, StructuralContext, StructuralScope,
    };

    use super::*;

    fn prepared(path: &str, text_lines: &[&str]) -> PreparedFile {
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
                context: StructuralContext::Executable,
                scope: StructuralScope::Module,
            });
        }

        let segments = if lines.is_empty() {
            Vec::new()
        } else {
            vec![NormalizedSegment {
                start: 0,
                end: lines.len() as u32,
            }]
        };

        PreparedFile {
            path: PathBuf::from(path),
            source: String::new(),
            normalized,
            lines,
            segments,
        }
    }

    fn occurrence(file: u32, start: u32, len: u32) -> Occurrence {
        Occurrence {
            file,
            normalized_start: start,
            normalized_len: len,
        }
    }

    fn group(effective_lines: u32, normalized_len: u32, occurrences: Vec<Occurrence>) -> DuplicateGroup {
        DuplicateGroup {
            effective_lines,
            normalized_len,
            occurrences,
        }
    }

    #[test]
    fn matches_stable_known_vector() {
        let corpus =
            build_corpus(vec![prepared("src/example.py", &["alpha = 1", "beta = 2"])]).unwrap();
        let group = group(2, 2, vec![occurrence(0, 0, 2)]);

        assert_eq!(
            finding_fingerprint(&corpus, &group).unwrap(),
            "arid-finding-v1:sha256:572a68e97a82622bee89bc469c0d261ad28357d1753dd70128e424144f9d6443"
        );
    }

    #[test]
    fn ignores_path_and_physical_line_numbers() {
        let first = build_corpus(vec![prepared("src/a.py", &["alpha = 1", "beta = 2"])]).unwrap();

        let mut moved = prepared("renamed/b.py", &["alpha = 1", "beta = 2"]);
        moved.lines[0].source_line = 100;
        moved.lines[1].source_line = 200;
        let second = build_corpus(vec![moved]).unwrap();
        let group = group(2, 2, vec![occurrence(0, 0, 2)]);

        assert_eq!(
            finding_fingerprint(&first, &group).unwrap(),
            finding_fingerprint(&second, &group).unwrap()
        );
    }

    #[test]
    fn ignores_occurrence_order_and_multiplicity() {
        let corpus = build_corpus(vec![
            prepared("a.py", &["alpha = 1", "beta = 2"]),
            prepared("b.py", &["alpha = 1", "beta = 2"]),
        ])
        .unwrap();

        let single = group(2, 2, vec![occurrence(0, 0, 2)]);
        let forward = group(2, 2, vec![occurrence(0, 0, 2), occurrence(1, 0, 2)]);
        let reversed = group(2, 2, vec![occurrence(1, 0, 2), occurrence(0, 0, 2)]);

        let expected = finding_fingerprint(&corpus, &single).unwrap();

        assert_eq!(finding_fingerprint(&corpus, &forward).unwrap(), expected);
        assert_eq!(finding_fingerprint(&corpus, &reversed).unwrap(), expected);
    }

    #[test]
    fn changes_when_normalized_content_changes() {
        let first = build_corpus(vec![prepared("a.py", &["alpha = 1", "beta = 2"])]).unwrap();
        let second = build_corpus(vec![prepared("a.py", &["alpha = 1", "beta = 3"])]).unwrap();
        let group = group(2, 2, vec![occurrence(0, 0, 2)]);

        assert_ne!(
            finding_fingerprint(&first, &group).unwrap(),
            finding_fingerprint(&second, &group).unwrap()
        );
    }

    #[test]
    fn preserves_normalized_line_boundaries() {
        let first = build_corpus(vec![prepared("a.py", &["ab", "c"])]).unwrap();
        let second = build_corpus(vec![prepared("a.py", &["a", "bc"])]).unwrap();
        let group = group(2, 2, vec![occurrence(0, 0, 2)]);

        assert_ne!(
            finding_fingerprint(&first, &group).unwrap(),
            finding_fingerprint(&second, &group).unwrap()
        );
    }

    #[test]
    fn includes_effective_line_count() {
        let corpus = build_corpus(vec![prepared("a.py", &["alpha = 1", "beta = 2"])]).unwrap();

        assert_ne!(
            finding_fingerprint(&corpus, &group(1, 2, vec![occurrence(0, 0, 2)])).unwrap(),
            finding_fingerprint(&corpus, &group(2, 2, vec![occurrence(0, 0, 2)])).unwrap()
        );
    }

    #[test]
    fn rejects_mismatched_occurrence_length() {
        let corpus = build_corpus(vec![prepared("a.py", &["alpha = 1", "beta = 2"])]).unwrap();
        let malformed = group(2, 2, vec![occurrence(0, 0, 1)]);

        assert!(matches!(
            finding_fingerprint(&corpus, &malformed),
            Err(FingerprintError::InvalidOccurrenceRange { .. })
        ));
    }
}
