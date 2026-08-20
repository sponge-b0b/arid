use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::baseline::{
    Baseline, BaselineError, BaselineGroup, BaselinePathCount, build_baseline,
};
use crate::baseline_status::{BaselineStatus, BaselineStatusGroup, BaselineStatusPath, DebtCounts};
use crate::corpus::Corpus;
use crate::model::{DuplicateGroup, NormalizationOptions};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BaselineComparison {
    pub(crate) active_groups: Vec<DuplicateGroup>,
    pub(crate) status: BaselineStatus,
    pub(crate) pruned: Baseline,
}

/// Compares current duplicate debt with an already validated baseline.
///
/// For every fingerprint/path multiplicity, debt is classified as accepted,
/// active, or stale. A current group remains active when any of its debt is
/// unaccepted so reporting retains the complete duplicate group. The pruned
/// baseline contains only the accepted intersection of current and baseline
/// debt, so pruning can never accept new debt.
pub(crate) fn compare_baseline(
    corpus: &Corpus,
    groups: Vec<DuplicateGroup>,
    baseline: &Baseline,
    normalization: NormalizationOptions,
    project_root: &Path,
) -> Result<BaselineComparison, BaselineError> {
    let baseline_groups = baseline
        .groups
        .iter()
        .map(|group| (group.fingerprint.as_str(), group))
        .collect::<BTreeMap<_, _>>();

    let mut active_groups = Vec::new();
    let mut status_groups = Vec::new();
    let mut pruned_groups = Vec::new();
    let mut current_fingerprints = BTreeSet::new();

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
        let baseline_group = baseline_groups
            .get(current_group.fingerprint.as_str())
            .copied();
        let status_group = compare_group_status(Some(current_group), baseline_group);

        if status_group.summary.active > 0 {
            active_groups.push(group);
        }

        if let Some(baseline_group) = baseline_group {
            if let Some(pruned_group) = prune_group(current_group, baseline_group) {
                pruned_groups.push(pruned_group);
            }
        }

        current_fingerprints.insert(current_group.fingerprint.clone());
        status_groups.push(status_group);
    }

    for baseline_group in &baseline.groups {
        if !current_fingerprints.contains(&baseline_group.fingerprint) {
            status_groups.push(compare_group_status(None, Some(baseline_group)));
        }
    }

    status_groups.sort_by(|left, right| left.fingerprint.cmp(&right.fingerprint));
    pruned_groups.sort();

    Ok(BaselineComparison {
        active_groups,
        status: BaselineStatus::new(status_groups),
        pruned: Baseline {
            version: baseline.version,
            normalization: baseline.normalization,
            groups: pruned_groups,
        },
    })
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

fn compare_group_status(
    current: Option<&BaselineGroup>,
    baseline: Option<&BaselineGroup>,
) -> BaselineStatusGroup {
    let identity = current
        .or(baseline)
        .expect("baseline comparison group must exist in current or baseline state");
    let mut paths = BTreeMap::<String, (u32, u32)>::new();

    if let Some(current) = current {
        for occurrence in &current.occurrences {
            paths.entry(occurrence.path.clone()).or_default().0 = occurrence.count;
        }
    }

    if let Some(baseline) = baseline {
        for occurrence in &baseline.occurrences {
            paths.entry(occurrence.path.clone()).or_default().1 = occurrence.count;
        }
    }

    let mut summary = DebtCounts::default();
    let paths = paths
        .into_iter()
        .map(|(path, (current, baseline))| {
            let accepted = u64::from(current.min(baseline));
            let active = u64::from(current.saturating_sub(baseline));
            let stale = u64::from(baseline.saturating_sub(current));

            debug_assert_eq!(accepted + active, u64::from(current));
            debug_assert_eq!(accepted + stale, u64::from(baseline));

            let debt = DebtCounts {
                accepted,
                active,
                stale,
            };
            summary.add(debt);

            BaselineStatusPath {
                path,
                accepted,
                active,
                stale,
            }
        })
        .collect();

    BaselineStatusGroup {
        fingerprint: identity.fingerprint.clone(),
        lines: identity.lines,
        paths,
        summary,
    }
}

fn prune_group(current: &BaselineGroup, baseline: &BaselineGroup) -> Option<BaselineGroup> {
    let current_counts = current
        .occurrences
        .iter()
        .map(|occurrence| (occurrence.path.as_str(), occurrence.count))
        .collect::<BTreeMap<_, _>>();

    let occurrences = baseline
        .occurrences
        .iter()
        .filter_map(|accepted| {
            let current = current_counts
                .get(accepted.path.as_str())
                .copied()
                .unwrap_or(0);
            let count = current.min(accepted.count);

            (count > 0).then(|| BaselinePathCount {
                path: accepted.path.clone(),
                count,
            })
        })
        .collect::<Vec<_>>();

    (!occurrences.is_empty()).then(|| BaselineGroup {
        fingerprint: baseline.fingerprint.clone(),
        lines: baseline.lines,
        occurrences,
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
        let status = compare_group_status(Some(&current), Some(&baseline));

        assert_eq!(
            status.summary,
            DebtCounts {
                accepted: 2,
                active: 2,
                stale: 4,
            }
        );
        assert_eq!(status.paths[0].path, "a.py");
        assert_eq!(status.paths[3].path, "d.py");
    }

    #[test]
    fn pruning_keeps_only_accepted_intersection() {
        let current = baseline_group(&[("a.py", 2), ("b.py", 1), ("c.py", 4)]);
        let baseline = baseline_group(&[("a.py", 1), ("b.py", 3), ("d.py", 2)]);

        assert_eq!(
            prune_group(&current, &baseline).unwrap().occurrences,
            vec![
                BaselinePathCount {
                    path: "a.py".to_owned(),
                    count: 1,
                },
                BaselinePathCount {
                    path: "b.py".to_owned(),
                    count: 1,
                },
            ]
        );
    }

    #[test]
    fn missing_baseline_fingerprint_is_wholly_active() {
        let current = baseline_group(&[("a.py", 2), ("b.py", 1)]);
        let status = compare_group_status(Some(&current), None);

        assert_eq!(
            status.summary,
            DebtCounts {
                accepted: 0,
                active: 3,
                stale: 0,
            }
        );
    }

    #[test]
    fn missing_current_fingerprint_is_wholly_stale() {
        let baseline = baseline_group(&[("a.py", 2), ("b.py", 1)]);
        let status = compare_group_status(None, Some(&baseline));

        assert_eq!(
            status.summary,
            DebtCounts {
                accepted: 0,
                active: 0,
                stale: 3,
            }
        );
    }

    #[test]
    fn comparison_returns_active_status_and_pruned_baseline() {
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
        assert_eq!(comparison.status.summary.accepted, 2);
        assert_eq!(comparison.status.summary.active, 2);
        assert_eq!(comparison.status.summary.stale, 0);
        assert_eq!(comparison.status.groups.len(), 2);
        assert_eq!(comparison.pruned, baseline);
    }

    #[test]
    fn comparison_prunes_wholly_stale_group() {
        let corpus = build_corpus(vec![
            prepared("project/a.py", &["alpha = 1", "beta = 2"]),
            prepared("project/b.py", &["alpha = 1", "beta = 2"]),
        ])
        .unwrap();
        let baseline_group = group(vec![occurrence(0, 0), occurrence(1, 0)]);
        let baseline = baseline_for(&corpus, &baseline_group);

        let comparison = compare_baseline(
            &corpus,
            Vec::new(),
            &baseline,
            normalization(),
            Path::new("project"),
        )
        .unwrap();

        assert!(comparison.active_groups.is_empty());
        assert_eq!(comparison.status.summary.accepted, 0);
        assert_eq!(comparison.status.summary.active, 0);
        assert_eq!(comparison.status.summary.stale, 2);
        assert!(comparison.pruned.groups.is_empty());
        assert_eq!(comparison.pruned.version, baseline.version);
        assert_eq!(comparison.pruned.normalization, baseline.normalization);
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
