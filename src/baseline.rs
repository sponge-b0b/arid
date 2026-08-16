use std::fmt::Write as _;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::corpus::Corpus;
use crate::model::{DuplicateGroup, NormalizationOptions};

pub const BASELINE_SCHEMA_VERSION: u8 = 1;

const GROUP_FINGERPRINT_DOMAIN: &[u8] = b"arid-baseline-group-v1\0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Baseline {
    pub version: u8,
    pub normalization: BaselineNormalization,
    pub groups: Vec<BaselineGroup>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaselineNormalization {
    pub ignore_comments: bool,
    pub ignore_docstrings: bool,
    pub ignore_imports: bool,
    pub ignore_signatures: bool,
}

impl From<NormalizationOptions> for BaselineNormalization {
    fn from(options: NormalizationOptions) -> Self {
        Self {
            ignore_comments: options.ignore_comments,
            ignore_docstrings: options.ignore_docstrings,
            ignore_imports: options.ignore_imports,
            ignore_signatures: options.ignore_signatures,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BaselineGroup {
    pub fingerprint: String,
    pub lines: u32,
    pub occurrences: Vec<BaselinePathCount>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BaselinePathCount {
    pub path: String,
    pub count: u32,
}

#[derive(Debug, Error)]
pub enum BaselineError {
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
    InvalidLineRange {
        path: PathBuf,
        start: u32,
        end: u32,
    },

    #[error("normalized line exceeds u32 byte-length capacity")]
    LineTooLong,
}

/// Returns the stable identity of one exact normalized duplicate block.
///
/// Identity intentionally excludes paths, physical source lines, structural
/// metadata, and occurrence distribution. The canonical first occurrence is
/// sufficient because every occurrence in a valid duplicate group is exactly
/// equal after normalization.
pub fn group_fingerprint(
    corpus: &Corpus,
    group: &DuplicateGroup,
) -> Result<String, BaselineError> {
    let occurrence = group
        .occurrences
        .first()
        .copied()
        .ok_or(BaselineError::EmptyGroup)?;

    let file = corpus
        .files
        .get(occurrence.file as usize)
        .ok_or(BaselineError::InvalidFile {
            file: occurrence.file,
        })?;

    if occurrence.normalized_len != group.normalized_len {
        return Err(BaselineError::InvalidOccurrenceRange {
            path: file.path.clone(),
            start: occurrence.normalized_start,
            length: occurrence.normalized_len,
        });
    }

    let start = occurrence.normalized_start as usize;
    let end = start
        .checked_add(occurrence.normalized_len as usize)
        .ok_or_else(|| BaselineError::InvalidOccurrenceRange {
            path: file.path.clone(),
            start: occurrence.normalized_start,
            length: occurrence.normalized_len,
        })?;

    let lines =
        file.lines
            .get(start..end)
            .ok_or_else(|| BaselineError::InvalidOccurrenceRange {
                path: file.path.clone(),
                start: occurrence.normalized_start,
                length: occurrence.normalized_len,
            })?;

    let mut hasher = Sha256::new();

    hasher.update(GROUP_FINGERPRINT_DOMAIN);
    hasher.update(group.effective_lines.to_be_bytes());

    for line in lines {
        let text = file
            .normalized
            .get(line.text_range.start as usize..line.text_range.end as usize)
            .ok_or_else(|| BaselineError::InvalidLineRange {
                path: file.path.clone(),
                start: line.text_range.start,
                end: line.text_range.end,
            })?;

        let length = u32::try_from(text.len()).map_err(|_| BaselineError::LineTooLong)?;

        hasher.update(length.to_be_bytes());
        hasher.update(text.as_bytes());
    }

    let digest = hasher.finalize();
    let mut fingerprint = String::with_capacity("sha256:".len() + digest.len() * 2);

    fingerprint.push_str("sha256:");

    for byte in digest {
        write!(&mut fingerprint, "{byte:02x}").expect("writing to String cannot fail");
    }

    Ok(fingerprint)
}

#[cfg(test)]
mod tests {
    use std::ops::Range;
    use std::path::PathBuf;

    use crate::corpus::build_corpus;
    use crate::model::{
        NormalizedLine, NormalizedSegment, Occurrence, PreparedFile, StructuralContext,
        StructuralScope,
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

    fn group(normalized_len: u32, effective_lines: u32) -> DuplicateGroup {
        DuplicateGroup {
            effective_lines,
            normalized_len,
            occurrences: vec![Occurrence {
                file: 0,
                normalized_start: 0,
                normalized_len,
            }],
        }
    }

    #[test]
    fn normalization_snapshot_contains_only_identity_settings() {
        let snapshot = BaselineNormalization::from(NormalizationOptions {
            ignore_comments: false,
            ignore_docstrings: true,
            ignore_imports: false,
            ignore_signatures: true,
        });

        assert_eq!(
            snapshot,
            BaselineNormalization {
                ignore_comments: false,
                ignore_docstrings: true,
                ignore_imports: false,
                ignore_signatures: true,
            }
        );
    }

    #[test]
    fn schema_serializes_to_expected_shape() {
        let baseline = Baseline {
            version: BASELINE_SCHEMA_VERSION,
            normalization: BaselineNormalization {
                ignore_comments: true,
                ignore_docstrings: true,
                ignore_imports: true,
                ignore_signatures: true,
            },
            groups: vec![BaselineGroup {
                fingerprint: "sha256:example".to_owned(),
                lines: 9,
                occurrences: vec![
                    BaselinePathCount {
                        path: "src/a.py".to_owned(),
                        count: 1,
                    },
                    BaselinePathCount {
                        path: "src/b.py".to_owned(),
                        count: 2,
                    },
                ],
            }],
        };

        assert_eq!(
            serde_json::to_string_pretty(&baseline).unwrap(),
            concat!(
                "{\n",
                "  \"version\": 1,\n",
                "  \"normalization\": {\n",
                "    \"ignore_comments\": true,\n",
                "    \"ignore_docstrings\": true,\n",
                "    \"ignore_imports\": true,\n",
                "    \"ignore_signatures\": true\n",
                "  },\n",
                "  \"groups\": [\n",
                "    {\n",
                "      \"fingerprint\": \"sha256:example\",\n",
                "      \"lines\": 9,\n",
                "      \"occurrences\": [\n",
                "        {\n",
                "          \"path\": \"src/a.py\",\n",
                "          \"count\": 1\n",
                "        },\n",
                "        {\n",
                "          \"path\": \"src/b.py\",\n",
                "          \"count\": 2\n",
                "        }\n",
                "      ]\n",
                "    }\n",
                "  ]\n",
                "}"
            )
        );
    }

    #[test]
    fn group_fingerprint_matches_stable_known_vector() {
        let corpus =
            build_corpus(vec![prepared("src/example.py", &["alpha = 1", "beta = 2"])]).unwrap();

        assert_eq!(
            group_fingerprint(&corpus, &group(2, 2)).unwrap(),
            "sha256:80d31b6f8888c1118e22dcd6bfee46046848b0c9f944aa4a236a1cfd5c8fac2d"
        );
    }

    #[test]
    fn group_fingerprint_ignores_path_and_physical_line_numbers() {
        let first =
            build_corpus(vec![prepared("src/a.py", &["alpha = 1", "beta = 2"])]).unwrap();

        let mut moved = prepared("renamed/b.py", &["alpha = 1", "beta = 2"]);
        moved.lines[0].source_line = 100;
        moved.lines[1].source_line = 200;

        let second = build_corpus(vec![moved]).unwrap();
        let group = group(2, 2);

        assert_eq!(
            group_fingerprint(&first, &group).unwrap(),
            group_fingerprint(&second, &group).unwrap()
        );
    }

    #[test]
    fn group_fingerprint_preserves_normalized_line_boundaries() {
        let first = build_corpus(vec![prepared("a.py", &["ab", "c"])]).unwrap();
        let second = build_corpus(vec![prepared("a.py", &["a", "bc"])]).unwrap();
        let group = group(2, 2);

        assert_ne!(
            group_fingerprint(&first, &group).unwrap(),
            group_fingerprint(&second, &group).unwrap()
        );
    }

    #[test]
    fn group_fingerprint_includes_effective_line_count() {
        let corpus = build_corpus(vec![prepared("a.py", &["alpha = 1", "beta = 2"])]).unwrap();

        assert_ne!(
            group_fingerprint(&corpus, &group(2, 1)).unwrap(),
            group_fingerprint(&corpus, &group(2, 2)).unwrap()
        );
    }

    #[test]
    fn group_fingerprint_rejects_mismatched_occurrence_length() {
        let corpus = build_corpus(vec![prepared("a.py", &["alpha = 1", "beta = 2"])]).unwrap();

        let malformed = DuplicateGroup {
            effective_lines: 2,
            normalized_len: 2,
            occurrences: vec![Occurrence {
                file: 0,
                normalized_start: 0,
                normalized_len: 1,
            }],
        };

        assert!(matches!(
            group_fingerprint(&corpus, &malformed),
            Err(BaselineError::InvalidOccurrenceRange { .. })
        ));
    }
}
