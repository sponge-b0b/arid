use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::corpus::Corpus;
use crate::model::{DuplicateGroup, NormalizationOptions, NormalizedLine, Occurrence, PreparedFile};

pub const BASELINE_SCHEMA_VERSION: u8 = 1;

const GROUP_FINGERPRINT_DOMAIN: &[u8] = b"arid-baseline-group-v1\0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Baseline {
    pub version: u8,
    pub normalization: BaselineNormalization,
    pub groups: Vec<BaselineGroup>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct BaselineGroup {
    pub fingerprint: String,
    pub lines: u32,
    pub occurrences: Vec<BaselinePathCount>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

    #[error("source path {path:?} is outside project root {root:?}")]
    PathOutsideProjectRoot { path: PathBuf, root: PathBuf },

    #[error("source path {path:?} cannot be represented as a clean project-relative path")]
    InvalidProjectRelativePath { path: PathBuf },

    #[error("source path {path:?} is not valid UTF-8")]
    NonUtf8ProjectRelativePath { path: PathBuf },

    #[error("duplicate occurrence count exceeds u32 capacity for {path:?}")]
    OccurrenceCountOverflow { path: String },

    #[error("multiple detected groups have the same baseline fingerprint {fingerprint}")]
    DuplicateFingerprint { fingerprint: String },

    #[error("failed to read baseline {path:?}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to write baseline {path:?}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to parse baseline {path:?}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("failed to serialize baseline: {0}")]
    Serialize(#[source] serde_json::Error),

    #[error(
        "unsupported baseline schema version {found}; supported version is {supported}"
    )]
    UnsupportedSchemaVersion { found: u8, supported: u8 },

    #[error(
        "baseline normalization settings do not match current settings: \
         baseline {baseline:?}, current {current:?}"
    )]
    NormalizationMismatch {
        baseline: BaselineNormalization,
        current: BaselineNormalization,
    },

    #[error("invalid baseline fingerprint {fingerprint:?}")]
    InvalidFingerprint { fingerprint: String },

    #[error("baseline group {fingerprint} must contain at least one duplicate line")]
    InvalidBaselineLines { fingerprint: String },

    #[error("baseline group {fingerprint} has no occurrence paths")]
    EmptyBaselineOccurrences { fingerprint: String },

    #[error("invalid baseline path {path:?}")]
    InvalidBaselinePath { path: String },

    #[error("baseline occurrence count for {path:?} in group {fingerprint} must be at least 1")]
    ZeroBaselineOccurrenceCount {
        fingerprint: String,
        path: String,
    },

    #[error("baseline contains duplicate group fingerprint {fingerprint}")]
    DuplicateBaselineGroup { fingerprint: String },

    #[error("baseline group {fingerprint} contains duplicate path entry {path:?}")]
    DuplicateBaselinePath {
        fingerprint: String,
        path: String,
    },
}

/// Constructs a canonical baseline from already-detected duplicate groups.
///
/// Baseline construction is post-detection. It does not alter duplicate
/// identity, rerun detection, or depend on physical source line numbers.
pub fn build_baseline(
    corpus: &Corpus,
    groups: &[DuplicateGroup],
    normalization: NormalizationOptions,
    project_root: &Path,
) -> Result<Baseline, BaselineError> {
    let mut baseline_groups = groups
        .iter()
        .map(|group| {
            Ok(BaselineGroup {
                fingerprint: group_fingerprint(corpus, group)?,
                lines: group.effective_lines,
                occurrences: group_path_counts(corpus, group, project_root)?,
            })
        })
        .collect::<Result<Vec<_>, BaselineError>>()?;

    baseline_groups.sort();

    for pair in baseline_groups.windows(2) {
        if pair[0].fingerprint == pair[1].fingerprint {
            return Err(BaselineError::DuplicateFingerprint {
                fingerprint: pair[0].fingerprint.clone(),
            });
        }
    }

    Ok(Baseline {
        version: BASELINE_SCHEMA_VERSION,
        normalization: normalization.into(),
        groups: baseline_groups,
    })
}

/// Serializes a validated baseline using deterministic pretty JSON.
pub fn serialize_baseline(baseline: &Baseline) -> Result<String, BaselineError> {
    let mut canonical = baseline.clone();
    validate_and_canonicalize_baseline(&mut canonical)?;

    serde_json::to_string_pretty(&canonical).map_err(BaselineError::Serialize)
}

/// Writes a validated baseline with atomic replacement semantics.
pub fn write_baseline(path: &Path, baseline: &Baseline) -> Result<(), BaselineError> {
    let contents = serialize_baseline(baseline)?;
    let mut file = AtomicWriteFile::open(path).map_err(|source| BaselineError::Write {
        path: path.to_path_buf(),
        source,
    })?;

    std::io::Write::write_all(&mut file, contents.as_bytes()).map_err(|source| {
        BaselineError::Write {
            path: path.to_path_buf(),
            source,
        }
    })?;

    file.commit().map_err(|source| BaselineError::Write {
        path: path.to_path_buf(),
        source,
    })
}

/// Reads, validates, and canonicalizes a baseline for the current normalization settings.
pub fn read_baseline(
    path: &Path,
    normalization: NormalizationOptions,
) -> Result<Baseline, BaselineError> {
    let contents = fs::read_to_string(path).map_err(|source| BaselineError::Read {
        path: path.to_path_buf(),
        source,
    })?;

    let mut baseline: Baseline =
        serde_json::from_str(&contents).map_err(|source| BaselineError::Parse {
            path: path.to_path_buf(),
            source,
        })?;

    validate_schema_version(&baseline)?;

    let current = BaselineNormalization::from(normalization);

    if baseline.normalization != current {
        return Err(BaselineError::NormalizationMismatch {
            baseline: baseline.normalization,
            current,
        });
    }

    validate_baseline_groups(&mut baseline)?;

    Ok(baseline)
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

    let (file, lines) = occurrence_lines(corpus, group, occurrence)?;
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

fn validate_and_canonicalize_baseline(baseline: &mut Baseline) -> Result<(), BaselineError> {
    validate_schema_version(baseline)?;
    validate_baseline_groups(baseline)
}

fn validate_schema_version(baseline: &Baseline) -> Result<(), BaselineError> {
    if baseline.version != BASELINE_SCHEMA_VERSION {
        return Err(BaselineError::UnsupportedSchemaVersion {
            found: baseline.version,
            supported: BASELINE_SCHEMA_VERSION,
        });
    }

    Ok(())
}

fn validate_baseline_groups(baseline: &mut Baseline) -> Result<(), BaselineError> {
    for group in &mut baseline.groups {
        if !valid_fingerprint(&group.fingerprint) {
            return Err(BaselineError::InvalidFingerprint {
                fingerprint: group.fingerprint.clone(),
            });
        }

        if group.lines == 0 {
            return Err(BaselineError::InvalidBaselineLines {
                fingerprint: group.fingerprint.clone(),
            });
        }

        if group.occurrences.is_empty() {
            return Err(BaselineError::EmptyBaselineOccurrences {
                fingerprint: group.fingerprint.clone(),
            });
        }

        for occurrence in &group.occurrences {
            if !valid_serialized_path(&occurrence.path) {
                return Err(BaselineError::InvalidBaselinePath {
                    path: occurrence.path.clone(),
                });
            }

            if occurrence.count == 0 {
                return Err(BaselineError::ZeroBaselineOccurrenceCount {
                    fingerprint: group.fingerprint.clone(),
                    path: occurrence.path.clone(),
                });
            }
        }

        group.occurrences.sort();

        for pair in group.occurrences.windows(2) {
            if pair[0].path == pair[1].path {
                return Err(BaselineError::DuplicateBaselinePath {
                    fingerprint: group.fingerprint.clone(),
                    path: pair[0].path.clone(),
                });
            }
        }
    }

    baseline.groups.sort();

    for pair in baseline.groups.windows(2) {
        if pair[0].fingerprint == pair[1].fingerprint {
            return Err(BaselineError::DuplicateBaselineGroup {
                fingerprint: pair[0].fingerprint.clone(),
            });
        }
    }

    Ok(())
}

fn valid_fingerprint(fingerprint: &str) -> bool {
    let Some(digest) = fingerprint.strip_prefix("sha256:") else {
        return false;
    };

    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn valid_serialized_path(path: &str) -> bool {
    if path.is_empty()
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains('\\')
        || path.contains('\0')
    {
        return false;
    }

    let bytes = path.as_bytes();

    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        return false;
    }

    path.split('/')
        .all(|component| !component.is_empty() && component != "." && component != "..")
}

fn group_path_counts(
    corpus: &Corpus,
    group: &DuplicateGroup,
    project_root: &Path,
) -> Result<Vec<BaselinePathCount>, BaselineError> {
    if group.occurrences.is_empty() {
        return Err(BaselineError::EmptyGroup);
    }

    let mut counts = BTreeMap::<String, u32>::new();

    for &occurrence in &group.occurrences {
        let (file, _) = occurrence_lines(corpus, group, occurrence)?;
        let path = baseline_path(&file.path, project_root)?;
        let count = counts.entry(path.clone()).or_default();

        *count = count
            .checked_add(1)
            .ok_or(BaselineError::OccurrenceCountOverflow { path })?;
    }

    Ok(counts
        .into_iter()
        .map(|(path, count)| BaselinePathCount { path, count })
        .collect())
}

fn occurrence_lines<'a>(
    corpus: &'a Corpus,
    group: &DuplicateGroup,
    occurrence: Occurrence,
) -> Result<(&'a PreparedFile, &'a [NormalizedLine]), BaselineError> {
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

    let lines = file
        .lines
        .get(start..end)
        .ok_or_else(|| BaselineError::InvalidOccurrenceRange {
            path: file.path.clone(),
            start: occurrence.normalized_start,
            length: occurrence.normalized_len,
        })?;

    if lines.is_empty() {
        return Err(BaselineError::InvalidOccurrenceRange {
            path: file.path.clone(),
            start: occurrence.normalized_start,
            length: occurrence.normalized_len,
        });
    }

    Ok((file, lines))
}

fn baseline_path(path: &Path, project_root: &Path) -> Result<String, BaselineError> {
    let relative =
        path.strip_prefix(project_root)
            .map_err(|_| BaselineError::PathOutsideProjectRoot {
                path: path.to_path_buf(),
                root: project_root.to_path_buf(),
            })?;

    let mut parts = Vec::new();

    for component in relative.components() {
        let Component::Normal(part) = component else {
            return Err(BaselineError::InvalidProjectRelativePath {
                path: path.to_path_buf(),
            });
        };

        let part = part
            .to_str()
            .ok_or_else(|| BaselineError::NonUtf8ProjectRelativePath {
                path: path.to_path_buf(),
            })?;

        parts.push(part);
    }

    if parts.is_empty() {
        return Err(BaselineError::InvalidProjectRelativePath {
            path: path.to_path_buf(),
        });
    }

    Ok(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use std::ops::Range;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::corpus::build_corpus;
    use crate::model::{
        NormalizedLine, NormalizedSegment, Occurrence, PreparedFile, StructuralContext,
        StructuralScope,
    };

    use super::*;

    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TempFile {
        path: PathBuf,
    }

    impl TempFile {
        fn new(contents: &str) -> Self {
            let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "arid-baseline-test-{}-{id}.json",
                std::process::id()
            ));

            fs::write(&path, contents).unwrap();

            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

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

    fn normalization() -> NormalizationOptions {
        NormalizationOptions {
            ignore_comments: true,
            ignore_docstrings: false,
            ignore_imports: true,
            ignore_signatures: false,
        }
    }

    fn fingerprint(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn baseline_group(fingerprint: String, path: &str, count: u32) -> BaselineGroup {
        BaselineGroup {
            fingerprint,
            lines: 2,
            occurrences: vec![BaselinePathCount {
                path: path.to_owned(),
                count,
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

    #[test]
    fn build_baseline_counts_occurrences_per_project_relative_path() {
        let corpus = build_corpus(vec![
            prepared(
                "project/src/a.py",
                &["alpha = 1", "beta = 2", "alpha = 1", "beta = 2"],
            ),
            prepared("project/src/b.py", &["alpha = 1", "beta = 2"]),
        ])
        .unwrap();

        let group = DuplicateGroup {
            effective_lines: 2,
            normalized_len: 2,
            occurrences: vec![
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
                Occurrence {
                    file: 1,
                    normalized_start: 0,
                    normalized_len: 2,
                },
            ],
        };

        let baseline =
            build_baseline(&corpus, &[group], normalization(), Path::new("project")).unwrap();

        assert_eq!(baseline.version, BASELINE_SCHEMA_VERSION);
        assert_eq!(baseline.normalization, normalization().into());
        assert_eq!(baseline.groups.len(), 1);
        assert_eq!(
            baseline.groups[0].occurrences,
            vec![
                BaselinePathCount {
                    path: "src/a.py".to_owned(),
                    count: 2,
                },
                BaselinePathCount {
                    path: "src/b.py".to_owned(),
                    count: 1,
                },
            ]
        );
    }

    #[test]
    fn build_baseline_is_independent_of_group_input_order() {
        let corpus = build_corpus(vec![
            prepared("project/a.py", &["alpha = 1", "beta = 2"]),
            prepared("project/b.py", &["alpha = 1", "beta = 2"]),
            prepared("project/c.py", &["gamma = 3", "delta = 4"]),
            prepared("project/d.py", &["gamma = 3", "delta = 4"]),
        ])
        .unwrap();

        let first = DuplicateGroup {
            effective_lines: 2,
            normalized_len: 2,
            occurrences: vec![
                Occurrence {
                    file: 0,
                    normalized_start: 0,
                    normalized_len: 2,
                },
                Occurrence {
                    file: 1,
                    normalized_start: 0,
                    normalized_len: 2,
                },
            ],
        };

        let second = DuplicateGroup {
            effective_lines: 2,
            normalized_len: 2,
            occurrences: vec![
                Occurrence {
                    file: 2,
                    normalized_start: 0,
                    normalized_len: 2,
                },
                Occurrence {
                    file: 3,
                    normalized_start: 0,
                    normalized_len: 2,
                },
            ],
        };

        let forward = build_baseline(
            &corpus,
            &[first.clone(), second.clone()],
            normalization(),
            Path::new("project"),
        )
        .unwrap();

        let reversed = build_baseline(
            &corpus,
            &[second, first],
            normalization(),
            Path::new("project"),
        )
        .unwrap();

        assert_eq!(forward, reversed);
        assert_eq!(
            serde_json::to_string_pretty(&forward).unwrap(),
            serde_json::to_string_pretty(&reversed).unwrap()
        );
    }

    #[test]
    fn baseline_path_uses_forward_slashes_between_components() {
        assert_eq!(
            baseline_path(
                Path::new("project").join("src").join("pkg").join("a.py").as_path(),
                Path::new("project"),
            )
            .unwrap(),
            "src/pkg/a.py"
        );
    }

    #[test]
    fn baseline_path_rejects_source_outside_project_root() {
        assert!(matches!(
            baseline_path(Path::new("elsewhere/a.py"), Path::new("project")),
            Err(BaselineError::PathOutsideProjectRoot { .. })
        ));
    }

    #[test]
    fn baseline_path_rejects_parent_components() {
        assert!(matches!(
            baseline_path(Path::new("project/../outside.py"), Path::new("project")),
            Err(BaselineError::InvalidProjectRelativePath { .. })
        ));
    }

    #[test]
    fn serialize_baseline_canonicalizes_group_and_path_order() {
        let mut first = baseline_group(fingerprint('b'), "z.py", 1);
        first.occurrences.push(BaselinePathCount {
            path: "a.py".to_owned(),
            count: 2,
        });

        let second = baseline_group(fingerprint('a'), "m.py", 1);

        let unordered = Baseline {
            version: BASELINE_SCHEMA_VERSION,
            normalization: normalization().into(),
            groups: vec![first, second],
        };

        let serialized = serialize_baseline(&unordered).unwrap();
        let parsed: Baseline = serde_json::from_str(&serialized).unwrap();

        assert_eq!(parsed.groups[0].fingerprint, fingerprint('a'));
        assert_eq!(parsed.groups[1].fingerprint, fingerprint('b'));
        assert_eq!(parsed.groups[1].occurrences[0].path, "a.py");
        assert_eq!(parsed.groups[1].occurrences[1].path, "z.py");
        assert_eq!(serialized, serialize_baseline(&parsed).unwrap());
    }

    #[test]
    fn write_baseline_creates_and_replaces_destination() {
        let file = TempFile::new("old contents");
        let first = Baseline {
            version: BASELINE_SCHEMA_VERSION,
            normalization: normalization().into(),
            groups: vec![baseline_group(fingerprint('a'), "a.py", 1)],
        };

        write_baseline(file.path(), &first).unwrap();
        assert_eq!(
            fs::read_to_string(file.path()).unwrap(),
            serialize_baseline(&first).unwrap()
        );

        let second = Baseline {
            version: BASELINE_SCHEMA_VERSION,
            normalization: normalization().into(),
            groups: vec![baseline_group(fingerprint('b'), "b.py", 2)],
        };

        write_baseline(file.path(), &second).unwrap();
        assert_eq!(
            fs::read_to_string(file.path()).unwrap(),
            serialize_baseline(&second).unwrap()
        );
    }

    #[test]
    fn write_baseline_does_not_touch_destination_when_validation_fails() {
        let file = TempFile::new("preserve me");
        let invalid = Baseline {
            version: BASELINE_SCHEMA_VERSION,
            normalization: normalization().into(),
            groups: vec![baseline_group("not-a-fingerprint".to_owned(), "a.py", 1)],
        };

        assert!(matches!(
            write_baseline(file.path(), &invalid),
            Err(BaselineError::InvalidFingerprint { .. })
        ));
        assert_eq!(fs::read_to_string(file.path()).unwrap(), "preserve me");
    }

    #[test]
    fn read_baseline_validates_normalization_and_canonicalizes_order() {
        let baseline = Baseline {
            version: BASELINE_SCHEMA_VERSION,
            normalization: normalization().into(),
            groups: vec![
                baseline_group(fingerprint('b'), "z.py", 1),
                baseline_group(fingerprint('a'), "a.py", 1),
            ],
        };

        let file = TempFile::new(&serde_json::to_string_pretty(&baseline).unwrap());
        let loaded = read_baseline(file.path(), normalization()).unwrap();

        assert_eq!(loaded.groups[0].fingerprint, fingerprint('a'));
        assert_eq!(loaded.groups[1].fingerprint, fingerprint('b'));
    }

    #[test]
    fn read_baseline_rejects_normalization_mismatch() {
        let baseline = Baseline {
            version: BASELINE_SCHEMA_VERSION,
            normalization: normalization().into(),
            groups: Vec::new(),
        };

        let file = TempFile::new(&serde_json::to_string_pretty(&baseline).unwrap());
        let mut current = normalization();
        current.ignore_comments = false;

        assert!(matches!(
            read_baseline(file.path(), current),
            Err(BaselineError::NormalizationMismatch { .. })
        ));
    }

    #[test]
    fn read_baseline_rejects_unsupported_schema_version() {
        let baseline = Baseline {
            version: BASELINE_SCHEMA_VERSION + 1,
            normalization: normalization().into(),
            groups: Vec::new(),
        };

        let file = TempFile::new(&serde_json::to_string_pretty(&baseline).unwrap());

        assert!(matches!(
            read_baseline(file.path(), normalization()),
            Err(BaselineError::UnsupportedSchemaVersion { .. })
        ));
    }

    #[test]
    fn read_baseline_rejects_invalid_fingerprint() {
        let baseline = Baseline {
            version: BASELINE_SCHEMA_VERSION,
            normalization: normalization().into(),
            groups: vec![baseline_group("sha256:ABC".to_owned(), "a.py", 1)],
        };

        let file = TempFile::new(&serde_json::to_string_pretty(&baseline).unwrap());

        assert!(matches!(
            read_baseline(file.path(), normalization()),
            Err(BaselineError::InvalidFingerprint { .. })
        ));
    }

    #[test]
    fn read_baseline_rejects_duplicate_group_identity() {
        let fingerprint = fingerprint('a');
        let baseline = Baseline {
            version: BASELINE_SCHEMA_VERSION,
            normalization: normalization().into(),
            groups: vec![
                baseline_group(fingerprint.clone(), "a.py", 1),
                baseline_group(fingerprint, "b.py", 1),
            ],
        };

        let file = TempFile::new(&serde_json::to_string_pretty(&baseline).unwrap());

        assert!(matches!(
            read_baseline(file.path(), normalization()),
            Err(BaselineError::DuplicateBaselineGroup { .. })
        ));
    }

    #[test]
    fn read_baseline_rejects_duplicate_path_entry() {
        let mut group = baseline_group(fingerprint('a'), "a.py", 1);
        group.occurrences.push(BaselinePathCount {
            path: "a.py".to_owned(),
            count: 2,
        });

        let baseline = Baseline {
            version: BASELINE_SCHEMA_VERSION,
            normalization: normalization().into(),
            groups: vec![group],
        };

        let file = TempFile::new(&serde_json::to_string_pretty(&baseline).unwrap());

        assert!(matches!(
            read_baseline(file.path(), normalization()),
            Err(BaselineError::DuplicateBaselinePath { .. })
        ));
    }

    #[test]
    fn read_baseline_rejects_malformed_json() {
        let file = TempFile::new("{not-json");

        assert!(matches!(
            read_baseline(file.path(), normalization()),
            Err(BaselineError::Parse { .. })
        ));
    }

    #[test]
    fn read_baseline_rejects_unknown_fields() {
        let contents = r#"{
  "version": 1,
  "normalization": {
    "ignore_comments": true,
    "ignore_docstrings": false,
    "ignore_imports": true,
    "ignore_signatures": false
  },
  "groups": [],
  "unexpected": true
}"#;
        let file = TempFile::new(contents);

        assert!(matches!(
            read_baseline(file.path(), normalization()),
            Err(BaselineError::Parse { .. })
        ));
    }

    #[test]
    fn read_baseline_rejects_nonportable_paths_and_zero_counts() {
        for path in ["/a.py", "../a.py", "src\\a.py", "C:/a.py"] {
            let baseline = Baseline {
                version: BASELINE_SCHEMA_VERSION,
                normalization: normalization().into(),
                groups: vec![baseline_group(fingerprint('a'), path, 1)],
            };
            let file = TempFile::new(&serde_json::to_string_pretty(&baseline).unwrap());

            assert!(matches!(
                read_baseline(file.path(), normalization()),
                Err(BaselineError::InvalidBaselinePath { .. })
            ));
        }

        let baseline = Baseline {
            version: BASELINE_SCHEMA_VERSION,
            normalization: normalization().into(),
            groups: vec![baseline_group(fingerprint('a'), "a.py", 0)],
        };
        let file = TempFile::new(&serde_json::to_string_pretty(&baseline).unwrap());

        assert!(matches!(
            read_baseline(file.path(), normalization()),
            Err(BaselineError::ZeroBaselineOccurrenceCount { .. })
        ));
    }
}
