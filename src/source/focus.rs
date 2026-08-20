use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::corpus::Corpus;
use crate::error::{ErrorKind, OperationalError};
use crate::model::DuplicateGroup;
use crate::project_path::project_relative_path;
use crate::source::SourceInput;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct FocusSet {
    selectors: Vec<String>,
}

impl FocusSet {
    pub(crate) fn selectors(&self) -> &[String] {
        &self.selectors
    }

    pub(crate) fn filter_groups(
        &self,
        corpus: &Corpus,
        groups: Vec<DuplicateGroup>,
        project_root: &Path,
    ) -> Vec<DuplicateGroup> {
        if self.selectors.is_empty() {
            return groups;
        }

        let focused_files = corpus
            .files
            .iter()
            .enumerate()
            .filter_map(|(index, file)| {
                let project_path = project_relative_path(&file.path, project_root).ok()?;
                self.matches_source(&project_path)
                    .then(|| u32::try_from(index).expect("corpus file count exceeds u32::MAX"))
            })
            .collect::<HashSet<_>>();

        groups
            .into_iter()
            .filter(|group| {
                group
                    .occurrences
                    .iter()
                    .any(|occurrence| focused_files.contains(&occurrence.file))
            })
            .collect()
    }

    fn matches_source(&self, source: &str) -> bool {
        self.selectors
            .iter()
            .any(|selector| selector_matches_source(selector, source))
    }
}

pub(crate) fn resolve_focus(
    requested: &[PathBuf],
    inputs: &[SourceInput],
    project_root: &Path,
) -> Result<FocusSet, OperationalError> {
    if requested.is_empty() {
        return Ok(FocusSet::default());
    }

    let mut selectors = requested
        .iter()
        .map(|path| normalize_selector(path, project_root))
        .collect::<Result<Vec<_>, _>>()?;
    selectors.sort();
    selectors.dedup();

    let source_paths = inputs
        .iter()
        .filter_map(|input| project_relative_path(input.path(), project_root).ok())
        .collect::<Vec<_>>();

    for selector in &selectors {
        if !source_paths
            .iter()
            .any(|source| selector_matches_source(selector, source))
        {
            return Err(OperationalError::new(
                ErrorKind::Configuration,
                format!("--focus does not match any Python source: {selector}"),
            ));
        }
    }

    Ok(FocusSet { selectors })
}

fn normalize_selector(path: &Path, project_root: &Path) -> Result<String, OperationalError> {
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_root.join(path)
    };

    project_relative_path(&resolved, project_root).map_err(|error| {
        OperationalError::new(
            ErrorKind::Configuration,
            format!("invalid --focus path {}: {error}", path.display()),
        )
    })
}

fn selector_matches_source(selector: &str, source: &str) -> bool {
    source == selector
        || source
            .strip_prefix(selector)
            .is_some_and(|remainder| remainder.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use crate::corpus::build_corpus;
    use crate::model::{
        DuplicateGroup, NormalizedLine, NormalizedSegment, Occurrence, PreparedFile,
        StructuralContext, StructuralScope,
    };

    use super::*;

    fn disk(path: impl Into<PathBuf>) -> SourceInput {
        SourceInput::Disk(path.into())
    }

    fn prepared(path: PathBuf) -> PreparedFile {
        PreparedFile {
            path,
            source: "value = 1\n".to_owned(),
            normalized: "value=1\n".to_owned(),
            lines: vec![NormalizedLine {
                text_range: Range { start: 0, end: 7 },
                source_line: 0,
                effective: true,
                context: StructuralContext::Executable,
                scope: StructuralScope::Module,
            }],
            segments: vec![NormalizedSegment { start: 0, end: 1 }],
        }
    }

    #[test]
    fn canonicalizes_deduplicates_and_sorts_selectors() {
        let root = Path::new("project");
        let inputs = vec![
            disk(root.join("src/a.py")),
            disk(root.join("tests/test_a.py")),
        ];

        let focus = resolve_focus(
            &[
                PathBuf::from("tests"),
                PathBuf::from("src/a.py"),
                PathBuf::from("tests"),
            ],
            &inputs,
            root,
        )
        .unwrap();

        assert_eq!(focus.selectors(), ["src/a.py", "tests"]);
    }

    #[test]
    fn directory_selector_uses_path_component_ancestry() {
        assert!(selector_matches_source("src", "src/a.py"));
        assert!(selector_matches_source("src", "src/pkg/a.py"));
        assert!(!selector_matches_source("src", "src2/a.py"));
        assert!(!selector_matches_source("src/pkg", "src/package/a.py"));
    }

    #[test]
    fn validates_against_virtual_source_that_is_not_on_disk() {
        let root = Path::new("project");
        let virtual_path = root.join("src/proposed.py");
        let inputs = vec![SourceInput::Virtual {
            path: virtual_path,
            source: "value = 1\n".to_owned(),
        }];

        let focus = resolve_focus(&[PathBuf::from("src/proposed.py")], &inputs, root).unwrap();

        assert_eq!(focus.selectors(), ["src/proposed.py"]);
    }

    #[test]
    fn rejects_unmatched_selector() {
        let root = Path::new("project");
        let inputs = vec![disk(root.join("src/a.py"))];

        let error = resolve_focus(&[PathBuf::from("tests")], &inputs, root).unwrap_err();

        assert_eq!(error.kind, ErrorKind::Configuration);
        assert!(error.to_string().contains("tests"));
    }

    #[test]
    fn rejects_selector_outside_project_root() {
        let root = Path::new("project");
        let inputs = vec![disk(root.join("src/a.py"))];

        let error = resolve_focus(&[PathBuf::from("../outside.py")], &inputs, root).unwrap_err();

        assert_eq!(error.kind, ErrorKind::Configuration);
        assert!(error.to_string().contains("invalid --focus path"));
    }

    #[test]
    fn filtering_keeps_complete_matching_groups() {
        let root = Path::new("project");
        let corpus = build_corpus(vec![
            prepared(root.join("a.py")),
            prepared(root.join("b.py")),
            prepared(root.join("c.py")),
        ])
        .unwrap();
        let inputs = vec![
            disk(root.join("a.py")),
            disk(root.join("b.py")),
            disk(root.join("c.py")),
        ];
        let focus = resolve_focus(&[PathBuf::from("b.py")], &inputs, root).unwrap();
        let matching = DuplicateGroup {
            effective_lines: 1,
            normalized_len: 1,
            occurrences: vec![
                Occurrence {
                    file: 0,
                    normalized_start: 0,
                    normalized_len: 1,
                },
                Occurrence {
                    file: 1,
                    normalized_start: 0,
                    normalized_len: 1,
                },
            ],
        };
        let unrelated = DuplicateGroup {
            effective_lines: 1,
            normalized_len: 1,
            occurrences: vec![Occurrence {
                file: 2,
                normalized_start: 0,
                normalized_len: 1,
            }],
        };

        let filtered = focus.filter_groups(&corpus, vec![matching.clone(), unrelated], root);

        assert_eq!(filtered, vec![matching]);
        assert_eq!(filtered[0].occurrences.len(), 2);
    }
}
