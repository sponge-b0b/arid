use std::collections::BTreeMap;
use std::path::Path;

use crate::baseline::{Baseline, BaselineError, BaselineGroup, build_baseline};
use crate::corpus::Corpus;
use crate::model::{DuplicateGroup, NormalizationOptions};

/// Removes duplicate groups fully covered by an already validated baseline.
///
/// A baseline accepts up to its recorded occurrence multiplicity for each
/// fingerprint/path pair. Reduced debt is accepted. A new fingerprint, a new
/// path, or an occurrence count above the accepted count keeps the entire
/// current group active so reporting retains complete duplicate context.
pub(crate) fn filter_active_groups(
    corpus: &Corpus,
    groups: Vec<DuplicateGroup>,
    baseline: &Baseline,
    normalization: NormalizationOptions,
    project_root: &Path,
) -> Result<Vec<DuplicateGroup>, BaselineError> {
    let accepted = baseline
        .groups
        .iter()
        .map(|group| (group.fingerprint.as_str(), group))
        .collect::<BTreeMap<_, _>>();

    let mut active = Vec::new();

    for group in groups {
        let current = build_baseline(
            corpus,
            std::slice::from_ref(&group),
            normalization,
            project_root,
        )?;
        let current_group = current
            .groups
            .first()
            .expect("one detected group must build one baseline group");

        let is_active = match accepted.get(current_group.fingerprint.as_str()) {
            Some(accepted_group) => exceeds_accepted_debt(current_group, accepted_group),
            None => true,
        };

        if is_active {
            active.push(group);
        }
    }

    Ok(active)
}

fn exceeds_accepted_debt(current: &BaselineGroup, accepted: &BaselineGroup) -> bool {
    current.occurrences.iter().any(|occurrence| {
        let accepted_count = accepted
            .occurrences
            .binary_search_by(|candidate| candidate.path.cmp(&occurrence.path))
            .ok()
            .map(|index| accepted.occurrences[index].count);

        accepted_count.is_none_or(|count| occurrence.count > count)
    })
}

#[cfg(test)]
mod tests {
    use std::ops::Range;
    use std::path::{Path, PathBuf};

    use crate::baseline::build_baseline;
    use crate::corpus::build_corpus;
    use crate::model::{
        NormalizedLine, NormalizedSegment, Occurrence, PreparedFile, StructuralContext,
        StructuralScope,
    };

    use super::*;

    fn normalization() -> NormalizationOptions {
        NormalizationOptions {
            ignore_comments: true,
            ignore_docstrings: true,
            ignore_imports: true,
            ignore_signatures: true,
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

    fn occurrence(file: u32, start: u32) -> Occurrence {
        Occurrence {
            file,
            normalized_start: start,
            normalized_len: 2,
        }
    }

    fn group(occurrences: Vec<Occurrence>) -> DuplicateGroup {
        DuplicateGroup {
            effective_lines: 2,
            normalized_len: 2,
            occurrences,
        }
    }

    fn baseline_for(corpus: &Corpus, group: &DuplicateGroup) -> Baseline {
        build_baseline(
            corpus,
            std::slice::from_ref(group),
            normalization(),
            Path::new("project"),
        )
        .unwrap()
    }

    #[test]
    fn exact_baseline_debt_is_accepted() {
        let corpus = build_corpus(vec![
            prepared("project/a.py", &["alpha = 1", "beta = 2"]),
            prepared("project/b.py", &["alpha = 1", "beta = 2"]),
        ])
        .unwrap();
        let group = group(vec![occurrence(0, 0), occurrence(1, 0)]);
        let baseline = baseline_for(&corpus, &group);

        assert!(
            filter_active_groups(
                &corpus,
                vec![group],
                &baseline,
                normalization(),
                Path::new("project"),
            )
            .unwrap()
            .is_empty()
        );
    }

    #[test]
    fn physical_line_shifts_do_not_reactivate_debt() {
        let baseline_corpus = build_corpus(vec![
            prepared("project/a.py", &["alpha = 1", "beta = 2"]),
            prepared("project/b.py", &["alpha = 1", "beta = 2"]),
        ])
        .unwrap();
        let group = group(vec![occurrence(0, 0), occurrence(1, 0)]);
        let baseline = baseline_for(&baseline_corpus, &group);

        let mut shifted_a = prepared("project/a.py", &["alpha = 1", "beta = 2"]);
        shifted_a.lines[0].source_line = 40;
        shifted_a.lines[1].source_line = 41;
        let mut shifted_b = prepared("project/b.py", &["alpha = 1", "beta = 2"]);
        shifted_b.lines[0].source_line = 90;
        shifted_b.lines[1].source_line = 91;
        let current_corpus = build_corpus(vec![shifted_a, shifted_b]).unwrap();

        assert!(
            filter_active_groups(
                &current_corpus,
                vec![group],
                &baseline,
                normalization(),
                Path::new("project"),
            )
            .unwrap()
            .is_empty()
        );
    }

    #[test]
    fn reduced_occurrence_debt_is_accepted() {
        let corpus = build_corpus(vec![
            prepared(
                "project/a.py",
                &["alpha = 1", "beta = 2", "alpha = 1", "beta = 2"],
            ),
            prepared("project/b.py", &["alpha = 1", "beta = 2"]),
        ])
        .unwrap();
        let baseline_group = group(vec![occurrence(0, 0), occurrence(0, 2), occurrence(1, 0)]);
        let baseline = baseline_for(&corpus, &baseline_group);
        let current = group(vec![occurrence(0, 0), occurrence(1, 0)]);

        assert!(
            filter_active_groups(
                &corpus,
                vec![current],
                &baseline,
                normalization(),
                Path::new("project"),
            )
            .unwrap()
            .is_empty()
        );
    }

    #[test]
    fn increased_occurrence_debt_keeps_entire_group_active() {
        let corpus = build_corpus(vec![
            prepared(
                "project/a.py",
                &["alpha = 1", "beta = 2", "alpha = 1", "beta = 2"],
            ),
            prepared("project/b.py", &["alpha = 1", "beta = 2"]),
        ])
        .unwrap();
        let baseline_group = group(vec![occurrence(0, 0), occurrence(1, 0)]);
        let baseline = baseline_for(&corpus, &baseline_group);
        let current = group(vec![occurrence(0, 0), occurrence(0, 2), occurrence(1, 0)]);

        let active = filter_active_groups(
            &corpus,
            vec![current.clone()],
            &baseline,
            normalization(),
            Path::new("project"),
        )
        .unwrap();

        assert_eq!(active, vec![current]);
        assert_eq!(active[0].occurrences.len(), 3);
    }

    #[test]
    fn new_occurrence_path_keeps_group_active() {
        let corpus = build_corpus(vec![
            prepared("project/a.py", &["alpha = 1", "beta = 2"]),
            prepared("project/b.py", &["alpha = 1", "beta = 2"]),
            prepared("project/c.py", &["alpha = 1", "beta = 2"]),
        ])
        .unwrap();
        let baseline_group = group(vec![occurrence(0, 0), occurrence(1, 0)]);
        let baseline = baseline_for(&corpus, &baseline_group);
        let current = group(vec![occurrence(0, 0), occurrence(1, 0), occurrence(2, 0)]);

        assert_eq!(
            filter_active_groups(
                &corpus,
                vec![current.clone()],
                &baseline,
                normalization(),
                Path::new("project"),
            )
            .unwrap(),
            vec![current]
        );
    }

    #[test]
    fn renamed_occurrence_path_is_conservatively_active() {
        let corpus = build_corpus(vec![
            prepared("project/a.py", &["alpha = 1", "beta = 2"]),
            prepared("project/b.py", &["alpha = 1", "beta = 2"]),
            prepared("project/c.py", &["alpha = 1", "beta = 2"]),
        ])
        .unwrap();
        let baseline_group = group(vec![occurrence(0, 0), occurrence(1, 0)]);
        let baseline = baseline_for(&corpus, &baseline_group);
        let current = group(vec![occurrence(0, 0), occurrence(2, 0)]);

        assert_eq!(
            filter_active_groups(
                &corpus,
                vec![current.clone()],
                &baseline,
                normalization(),
                Path::new("project"),
            )
            .unwrap(),
            vec![current]
        );
    }

    #[test]
    fn new_fingerprint_is_active() {
        let baseline_corpus = build_corpus(vec![
            prepared("project/a.py", &["alpha = 1", "beta = 2"]),
            prepared("project/b.py", &["alpha = 1", "beta = 2"]),
        ])
        .unwrap();
        let baseline_group = group(vec![occurrence(0, 0), occurrence(1, 0)]);
        let baseline = baseline_for(&baseline_corpus, &baseline_group);

        let current_corpus = build_corpus(vec![
            prepared("project/a.py", &["gamma = 3", "delta = 4"]),
            prepared("project/b.py", &["gamma = 3", "delta = 4"]),
        ])
        .unwrap();
        let current = group(vec![occurrence(0, 0), occurrence(1, 0)]);

        assert_eq!(
            filter_active_groups(
                &current_corpus,
                vec![current.clone()],
                &baseline,
                normalization(),
                Path::new("project"),
            )
            .unwrap(),
            vec![current]
        );
    }
}
