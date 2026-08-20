use std::collections::BTreeMap;
use std::path::Path;

use crate::baseline::{Baseline, BaselineError, BaselineGroup, build_baseline};
use crate::corpus::Corpus;
use crate::model::{DuplicateGroup, NormalizationOptions};

#[derive(Debug, Clone, PartialEq, Eq)]
struct BaselineComparison {
    active_groups: Vec<DuplicateGroup>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct DebtCounts {
    accepted: u64,
    active: u64,
    stale: u64,
}

impl DebtCounts {
    fn add_path(&mut self, current: u32, baseline: u32) {
        let accepted = u64::from(current.min(baseline));
        let active = u64::from(current.saturating_sub(baseline));
        let stale = u64::from(baseline.saturating_sub(current));

        debug_assert_eq!(accepted + active, u64::from(current));
        debug_assert_eq!(accepted + stale, u64::from(baseline));

        self.accepted += accepted;
        self.active += active;
        self.stale += stale;
    }
}

/// Compares current duplicate debt with an already validated baseline.
///
/// For every fingerprint/path multiplicity, debt is classified as accepted,
/// active, or stale. A current group remains active when any of its debt is
/// unaccepted so reporting retains the complete duplicate group.
fn compare_baseline(
    corpus: &Corpus,
    groups: Vec<DuplicateGroup>,
    baseline: &Baseline,
    normalization: NormalizationOptions,
    project_root: &Path,
) -> Result<BaselineComparison, BaselineError> {
    let accepted = baseline
        .groups
        .iter()
        .map(|group| (group.fingerprint.as_str(), group))
        .collect::<BTreeMap<_, _>>();

    let mut active_groups = Vec::new();

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
        let debt = compare_group_debt(
            current_group,
            accepted.get(current_group.fingerprint.as_str()).copied(),
        );

        if debt.active > 0 {
            active_groups.push(group);
        }
    }

    Ok(BaselineComparison { active_groups })
}

/// Removes duplicate groups fully covered by an already validated baseline.
pub(crate) fn filter_active_groups(
    corpus: &Corpus,
    groups: Vec<DuplicateGroup>,
    baseline: &Baseline,
    normalization: NormalizationOptions,
    project_root: &Path,
) -> Result<Vec<DuplicateGroup>, BaselineError> {
    Ok(compare_baseline(corpus, groups, baseline, normalization, project_root)?.active_groups)
}

fn compare_group_debt(current: &BaselineGroup, baseline: Option<&BaselineGroup>) -> DebtCounts {
    let mut paths = BTreeMap::<&str, (u32, u32)>::new();

    for occurrence in &current.occurrences {
        paths.entry(&occurrence.path).or_default().0 = occurrence.count;
    }

    if let Some(baseline) = baseline {
        for occurrence in &baseline.occurrences {
            paths.entry(&occurrence.path).or_default().1 = occurrence.count;
        }
    }

    paths
        .into_values()
        .fold(DebtCounts::default(), |mut debt, (current, baseline)| {
            debt.add_path(current, baseline);
            debt
        })
}

#[cfg(test)]
mod tests {
    use std::ops::Range;
    use std::path::{Path, PathBuf};

    use crate::baseline::{BaselinePathCount, build_baseline};
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

    fn baseline_group(occurrences: &[(&str, u32)]) -> BaselineGroup {
        BaselineGroup {
            fingerprint: format!("sha256:{}", "0".repeat(64)),
            lines: 2,
            occurrences: occurrences
                .iter()
                .map(|(path, count)| BaselinePathCount {
                    path: (*path).to_owned(),
                    count: *count,
                })
                .collect(),
        }
    }

    #[test]
    fn debt_math_classifies_accepted_active_and_stale_counts() {
        let current = baseline_group(&[("a.py", 2), ("b.py", 1), ("c.py", 1)]);
        let baseline = baseline_group(&[("a.py", 1), ("b.py", 2), ("d.py", 3)]);

        assert_eq!(
            compare_group_debt(&current, Some(&baseline)),
            DebtCounts {
                accepted: 2,
                active: 2,
                stale: 4,
            }
        );
    }

    #[test]
    fn missing_baseline_fingerprint_is_wholly_active() {
        let current = baseline_group(&[("a.py", 2), ("b.py", 1)]);

        assert_eq!(
            compare_group_debt(&current, None),
            DebtCounts {
                accepted: 0,
                active: 3,
                stale: 0,
            }
        );
    }

    #[test]
    fn comparison_returns_only_active_groups() {
        let corpus = build_corpus(vec![
            prepared("project/a.py", &["alpha = 1", "beta = 2"]),
            prepared("project/b.py", &["alpha = 1", "beta = 2"]),
            prepared("project/c.py", &["gamma = 3", "delta = 4"]),
            prepared("project/d.py", &["gamma = 3", "delta = 4"]),
        ])
        .unwrap();
        let accepted_group = group(vec![occurrence(0, 0), occurrence(1, 0)]);
        let active_group = group(vec![occurrence(2, 0), occurrence(3, 0)]);
        let baseline = baseline_for(&corpus, &accepted_group);

        let comparison = compare_baseline(
            &corpus,
            vec![accepted_group, active_group.clone()],
            &baseline,
            normalization(),
            Path::new("project"),
        )
        .unwrap();

        assert_eq!(comparison.active_groups, vec![active_group]);
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
