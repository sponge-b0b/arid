use std::collections::{BTreeMap, BTreeSet};

use crate::error::OperationalError;
use crate::report::{FindingContext, FindingDistribution, FindingScope, Report};

pub(crate) const SUMMARY_SCHEMA_VERSION: u8 = 1;
const HOTSPOT_LIMIT: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SummaryOptions {
    pub(crate) ignore_files: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SummaryAnalysis {
    pub(crate) min_lines: u32,
    pub(crate) ignore_comments: bool,
    pub(crate) ignore_docstrings: bool,
    pub(crate) ignore_imports: bool,
    pub(crate) ignore_signatures: bool,
    pub(crate) same_file: bool,
    pub(crate) hidden: bool,
    pub(crate) ignore_files: bool,
    pub(crate) exclude: Vec<String>,
    pub(crate) baseline_enabled: bool,
    pub(crate) focus: Vec<String>,
    pub(crate) virtual_source: Option<String>,
    pub(crate) keep_going: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ContextSummary {
    pub(crate) executable: u64,
    pub(crate) declarative: u64,
    pub(crate) mixed: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ScopeSummary {
    pub(crate) function: u64,
    pub(crate) module: u64,
    pub(crate) class: u64,
    pub(crate) mixed: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DistributionSummary {
    pub(crate) cross_file: u64,
    pub(crate) same_file: u64,
    pub(crate) hybrid: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hotspot {
    pub(crate) path: String,
    pub(crate) groups: u64,
    pub(crate) occurrences: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Summary {
    pub(crate) schema_version: u8,
    pub(crate) tool_version: &'static str,
    pub(crate) complete: bool,
    pub(crate) analysis: SummaryAnalysis,
    pub(crate) errors: Vec<OperationalError>,
    pub(crate) files: u64,
    pub(crate) files_with_duplicates: u64,
    pub(crate) source_lines: u64,
    pub(crate) analyzed_lines: u64,
    pub(crate) duplicate_groups: u64,
    pub(crate) occurrences: u64,
    pub(crate) duplicate_lines: u64,
    pub(crate) duplication_percent: f64,
    pub(crate) context: ContextSummary,
    pub(crate) scope: ScopeSummary,
    pub(crate) distribution: DistributionSummary,
    pub(crate) hotspots: Vec<Hotspot>,
}

#[must_use]
pub(crate) fn build_summary(report: &Report, options: SummaryOptions) -> Summary {
    let mut occurrences = 0_u64;
    let mut context = ContextSummary::default();
    let mut scope = ScopeSummary::default();
    let mut distribution = DistributionSummary::default();
    let mut hotspots = BTreeMap::<String, (u64, u64)>::new();

    for finding in &report.findings {
        occurrences += u64::from(finding.occurrences);

        match finding.context {
            FindingContext::Executable => context.executable += 1,
            FindingContext::Declarative => context.declarative += 1,
            FindingContext::Mixed => context.mixed += 1,
        }

        match finding.scope {
            FindingScope::Function => scope.function += 1,
            FindingScope::Module => scope.module += 1,
            FindingScope::Class => scope.class += 1,
            FindingScope::Mixed => scope.mixed += 1,
        }

        match finding.distribution {
            FindingDistribution::CrossFile => distribution.cross_file += 1,
            FindingDistribution::SameFile => distribution.same_file += 1,
            FindingDistribution::Hybrid => distribution.hybrid += 1,
        }

        let mut group_paths = BTreeSet::new();

        for location in &finding.locations {
            hotspots.entry(location.path.clone()).or_default().1 += 1;
            group_paths.insert(location.path.as_str());
        }

        for path in group_paths {
            hotspots.entry(path.to_owned()).or_default().0 += 1;
        }
    }

    debug_assert_eq!(
        context.executable + context.declarative + context.mixed,
        report.duplicate_groups,
    );
    debug_assert_eq!(
        scope.function + scope.module + scope.class + scope.mixed,
        report.duplicate_groups,
    );
    debug_assert_eq!(
        distribution.cross_file + distribution.same_file + distribution.hybrid,
        report.duplicate_groups,
    );

    let files_with_duplicates = hotspots.len() as u64;
    let mut hotspots = hotspots
        .into_iter()
        .map(|(path, (groups, occurrences))| Hotspot {
            path,
            groups,
            occurrences,
        })
        .collect::<Vec<_>>();

    hotspots.sort_by(|left, right| {
        right
            .groups
            .cmp(&left.groups)
            .then_with(|| right.occurrences.cmp(&left.occurrences))
            .then_with(|| left.path.cmp(&right.path))
    });
    hotspots.truncate(HOTSPOT_LIMIT);

    let analysis = &report.analysis;

    Summary {
        schema_version: SUMMARY_SCHEMA_VERSION,
        tool_version: report.tool_version,
        complete: report.complete,
        analysis: SummaryAnalysis {
            min_lines: analysis.min_lines,
            ignore_comments: analysis.ignore_comments,
            ignore_docstrings: analysis.ignore_docstrings,
            ignore_imports: analysis.ignore_imports,
            ignore_signatures: analysis.ignore_signatures,
            same_file: analysis.same_file,
            hidden: analysis.hidden,
            ignore_files: options.ignore_files,
            exclude: analysis.exclude.clone(),
            baseline_enabled: analysis.baseline_enabled,
            focus: analysis.focus.clone(),
            virtual_source: analysis.virtual_source.clone(),
            keep_going: analysis.keep_going,
        },
        errors: report.errors.clone(),
        files: report.files,
        files_with_duplicates,
        source_lines: report.source_lines,
        analyzed_lines: report.analyzed_lines,
        duplicate_groups: report.duplicate_groups,
        occurrences,
        duplicate_lines: report.duplicate_lines,
        duplication_percent: report.duplication_percent,
        context,
        scope,
        distribution,
        hotspots,
    }
}

#[cfg(test)]
mod tests {
    use crate::error::{ErrorKind, OperationalError};
    use crate::report::{AnalysisMetadata, Finding, Location};

    use super::*;

    fn location(path: &str, start_line: u64) -> Location {
        Location {
            path: path.to_owned(),
            start_line,
            end_line: start_line + 1,
            source: None,
        }
    }

    fn finding(
        context: FindingContext,
        scope: FindingScope,
        distribution: FindingDistribution,
        locations: Vec<Location>,
    ) -> Finding {
        let files = locations
            .iter()
            .map(|location| location.path.as_str())
            .collect::<BTreeSet<_>>()
            .len() as u32;

        Finding {
            code: "DUP001".to_owned(),
            fingerprint: format!("arid-finding-v1:sha256:{}", "0".repeat(64)),
            lines: 4,
            context,
            scope,
            occurrences: locations.len() as u32,
            files,
            distribution,
            locations,
        }
    }

    fn report(findings: Vec<Finding>) -> Report {
        Report {
            schema_version: 4,
            tool_version: env!("CARGO_PKG_VERSION"),
            complete: true,
            analysis: AnalysisMetadata::default(),
            errors: Vec::new(),
            files: 8,
            source_lines: 800,
            analyzed_lines: 600,
            duplicate_groups: findings.len() as u64,
            duplicate_lines: 120,
            duplication_percent: 20.0,
            findings,
        }
    }

    #[test]
    fn aggregates_overall_counts_and_fixed_buckets() {
        let report = report(vec![
            finding(
                FindingContext::Executable,
                FindingScope::Function,
                FindingDistribution::CrossFile,
                vec![location("a.py", 1), location("b.py", 1)],
            ),
            finding(
                FindingContext::Declarative,
                FindingScope::Module,
                FindingDistribution::SameFile,
                vec![location("c.py", 1), location("c.py", 10)],
            ),
            finding(
                FindingContext::Mixed,
                FindingScope::Mixed,
                FindingDistribution::Hybrid,
                vec![
                    location("d.py", 1),
                    location("d.py", 10),
                    location("e.py", 1),
                ],
            ),
            finding(
                FindingContext::Executable,
                FindingScope::Class,
                FindingDistribution::CrossFile,
                vec![location("f.py", 1), location("g.py", 1)],
            ),
        ]);

        let summary = build_summary(&report, SummaryOptions { ignore_files: true });

        assert_eq!(summary.schema_version, 1);
        assert_eq!(summary.files, 8);
        assert_eq!(summary.files_with_duplicates, 7);
        assert_eq!(summary.source_lines, 800);
        assert_eq!(summary.analyzed_lines, 600);
        assert_eq!(summary.duplicate_groups, 4);
        assert_eq!(summary.occurrences, 9);
        assert_eq!(summary.duplicate_lines, 120);
        assert_eq!(summary.duplication_percent, 20.0);
        assert_eq!(
            summary.context,
            ContextSummary {
                executable: 2,
                declarative: 1,
                mixed: 1,
            }
        );
        assert_eq!(
            summary.scope,
            ScopeSummary {
                function: 1,
                module: 1,
                class: 1,
                mixed: 1,
            }
        );
        assert_eq!(
            summary.distribution,
            DistributionSummary {
                cross_file: 2,
                same_file: 1,
                hybrid: 1,
            }
        );
    }

    #[test]
    fn hotspot_counts_each_group_once_per_path_but_every_location() {
        let report = report(vec![
            finding(
                FindingContext::Executable,
                FindingScope::Function,
                FindingDistribution::Hybrid,
                vec![
                    location("a.py", 1),
                    location("a.py", 10),
                    location("b.py", 1),
                ],
            ),
            finding(
                FindingContext::Executable,
                FindingScope::Function,
                FindingDistribution::CrossFile,
                vec![location("a.py", 20), location("c.py", 1)],
            ),
        ]);

        let summary = build_summary(&report, SummaryOptions { ignore_files: true });

        assert_eq!(
            summary.hotspots,
            vec![
                Hotspot {
                    path: "a.py".to_owned(),
                    groups: 2,
                    occurrences: 3,
                },
                Hotspot {
                    path: "b.py".to_owned(),
                    groups: 1,
                    occurrences: 1,
                },
                Hotspot {
                    path: "c.py".to_owned(),
                    groups: 1,
                    occurrences: 1,
                },
            ]
        );
    }

    #[test]
    fn hotspot_order_is_deterministic_and_truncated_to_five() {
        let findings = ["f.py", "e.py", "d.py", "c.py", "b.py", "a.py"]
            .into_iter()
            .map(|path| {
                finding(
                    FindingContext::Executable,
                    FindingScope::Function,
                    FindingDistribution::SameFile,
                    vec![location(path, 1), location(path, 10)],
                )
            })
            .collect();

        let summary = build_summary(&report(findings), SummaryOptions { ignore_files: true });

        assert_eq!(
            summary
                .hotspots
                .iter()
                .map(|hotspot| hotspot.path.as_str())
                .collect::<Vec<_>>(),
            vec!["a.py", "b.py", "c.py", "d.py", "e.py"]
        );
    }

    #[test]
    fn zero_findings_produce_zero_aggregate_state() {
        let mut report = report(Vec::new());
        report.duplicate_lines = 0;
        report.duplication_percent = 0.0;

        let summary = build_summary(&report, SummaryOptions { ignore_files: true });

        assert_eq!(summary.files_with_duplicates, 0);
        assert_eq!(summary.duplicate_groups, 0);
        assert_eq!(summary.occurrences, 0);
        assert_eq!(summary.context, ContextSummary::default());
        assert_eq!(summary.scope, ScopeSummary::default());
        assert_eq!(summary.distribution, DistributionSummary::default());
        assert!(summary.hotspots.is_empty());
    }

    #[test]
    fn preserves_completeness_errors_and_effective_analysis_policy() {
        let mut report = report(Vec::new());
        report.complete = false;
        report.analysis.min_lines = 7;
        report.analysis.hidden = true;
        report.analysis.focus = vec!["src".to_owned()];
        report.analysis.keep_going = true;
        report.errors = vec![OperationalError::new(ErrorKind::Parse, "broken source")];

        let summary = build_summary(
            &report,
            SummaryOptions {
                ignore_files: false,
            },
        );

        assert!(!summary.complete);
        assert_eq!(summary.errors, report.errors);
        assert_eq!(summary.analysis.min_lines, 7);
        assert!(summary.analysis.hidden);
        assert_eq!(summary.analysis.focus, vec!["src"]);
        assert!(summary.analysis.keep_going);
        assert!(!summary.analysis.ignore_files);
    }
}
