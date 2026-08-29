use serde::Serialize;

use crate::error::OperationalError;
use crate::summary::{
    ContextSummary, DistributionSummary, Hotspot, ScopeSummary, Summary, SummaryAnalysis,
};

#[derive(Serialize)]
struct SummaryDocument<'a> {
    schema_version: u8,
    tool_version: &'a str,
    complete: bool,
    analysis: AnalysisDocument<'a>,
    errors: &'a [OperationalError],
    files: u64,
    files_with_duplicates: u64,
    source_lines: u64,
    analyzed_lines: u64,
    duplicate_groups: u64,
    occurrences: u64,
    duplicate_lines: u64,
    duplication_percent: f64,
    context: ContextDocument,
    scope: ScopeDocument,
    distribution: DistributionDocument,
    hotspots: Vec<HotspotDocument<'a>>,
}

#[derive(Serialize)]
struct AnalysisDocument<'a> {
    min_lines: u32,
    ignore_comments: bool,
    ignore_docstrings: bool,
    ignore_imports: bool,
    ignore_signatures: bool,
    same_file: bool,
    hidden: bool,
    ignore_files: bool,
    exclude: &'a [String],
    baseline_enabled: bool,
    focus: &'a [String],
    virtual_source: &'a Option<String>,
    keep_going: bool,
}

#[derive(Serialize)]
struct ContextDocument {
    executable: u64,
    declarative: u64,
    mixed: u64,
}

#[derive(Serialize)]
struct ScopeDocument {
    function: u64,
    module: u64,
    class: u64,
    mixed: u64,
}

#[derive(Serialize)]
struct DistributionDocument {
    cross_file: u64,
    same_file: u64,
    hybrid: u64,
}

#[derive(Serialize)]
struct HotspotDocument<'a> {
    path: &'a str,
    groups: u64,
    occurrences: u64,
}

impl<'a> From<&'a Summary> for SummaryDocument<'a> {
    fn from(summary: &'a Summary) -> Self {
        Self {
            schema_version: summary.schema_version,
            tool_version: summary.tool_version,
            complete: summary.complete,
            analysis: AnalysisDocument::from(&summary.analysis),
            errors: &summary.errors,
            files: summary.files,
            files_with_duplicates: summary.files_with_duplicates,
            source_lines: summary.source_lines,
            analyzed_lines: summary.analyzed_lines,
            duplicate_groups: summary.duplicate_groups,
            occurrences: summary.occurrences,
            duplicate_lines: summary.duplicate_lines,
            duplication_percent: summary.duplication_percent,
            context: ContextDocument::from(summary.context),
            scope: ScopeDocument::from(summary.scope),
            distribution: DistributionDocument::from(summary.distribution),
            hotspots: summary.hotspots.iter().map(HotspotDocument::from).collect(),
        }
    }
}

impl<'a> From<&'a SummaryAnalysis> for AnalysisDocument<'a> {
    fn from(analysis: &'a SummaryAnalysis) -> Self {
        Self {
            min_lines: analysis.min_lines,
            ignore_comments: analysis.ignore_comments,
            ignore_docstrings: analysis.ignore_docstrings,
            ignore_imports: analysis.ignore_imports,
            ignore_signatures: analysis.ignore_signatures,
            same_file: analysis.same_file,
            hidden: analysis.hidden,
            ignore_files: analysis.ignore_files,
            exclude: &analysis.exclude,
            baseline_enabled: analysis.baseline_enabled,
            focus: &analysis.focus,
            virtual_source: &analysis.virtual_source,
            keep_going: analysis.keep_going,
        }
    }
}

impl From<ContextSummary> for ContextDocument {
    fn from(context: ContextSummary) -> Self {
        Self {
            executable: context.executable,
            declarative: context.declarative,
            mixed: context.mixed,
        }
    }
}

impl From<ScopeSummary> for ScopeDocument {
    fn from(scope: ScopeSummary) -> Self {
        Self {
            function: scope.function,
            module: scope.module,
            class: scope.class,
            mixed: scope.mixed,
        }
    }
}

impl From<DistributionSummary> for DistributionDocument {
    fn from(distribution: DistributionSummary) -> Self {
        Self {
            cross_file: distribution.cross_file,
            same_file: distribution.same_file,
            hybrid: distribution.hybrid,
        }
    }
}

impl<'a> From<&'a Hotspot> for HotspotDocument<'a> {
    fn from(hotspot: &'a Hotspot) -> Self {
        Self {
            path: &hotspot.path,
            groups: hotspot.groups,
            occurrences: hotspot.occurrences,
        }
    }
}

pub(crate) fn render_summary_json(summary: &Summary) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&SummaryDocument::from(summary))
}

#[cfg(test)]
mod tests {
    use crate::error::{ErrorKind, OperationalError};
    use crate::summary::{
        ContextSummary, DistributionSummary, Hotspot, ScopeSummary, Summary, SummaryAnalysis,
        SUMMARY_SCHEMA_VERSION,
    };

    use super::*;

    fn analysis() -> SummaryAnalysis {
        SummaryAnalysis {
            min_lines: 4,
            ignore_comments: true,
            ignore_docstrings: true,
            ignore_imports: true,
            ignore_signatures: true,
            same_file: true,
            hidden: false,
            ignore_files: true,
            exclude: Vec::new(),
            baseline_enabled: false,
            focus: Vec::new(),
            virtual_source: None,
            keep_going: false,
        }
    }

    fn complete_summary() -> Summary {
        Summary {
            schema_version: SUMMARY_SCHEMA_VERSION,
            tool_version: "2.2.0",
            complete: true,
            analysis: analysis(),
            errors: Vec::new(),
            files: 8,
            files_with_duplicates: 3,
            source_lines: 800,
            analyzed_lines: 600,
            duplicate_groups: 4,
            occurrences: 9,
            duplicate_lines: 120,
            duplication_percent: 20.0,
            context: ContextSummary {
                executable: 2,
                declarative: 1,
                mixed: 1,
            },
            scope: ScopeSummary {
                function: 1,
                module: 1,
                class: 1,
                mixed: 1,
            },
            distribution: DistributionSummary {
                cross_file: 2,
                same_file: 1,
                hybrid: 1,
            },
            hotspots: vec![
                Hotspot {
                    path: "src/a.py".to_owned(),
                    groups: 3,
                    occurrences: 5,
                },
                Hotspot {
                    path: "src/b.py".to_owned(),
                    groups: 2,
                    occurrences: 2,
                },
            ],
        }
    }

    fn zero_summary() -> Summary {
        Summary {
            schema_version: SUMMARY_SCHEMA_VERSION,
            tool_version: "2.2.0",
            complete: true,
            analysis: analysis(),
            errors: Vec::new(),
            files: 2,
            files_with_duplicates: 0,
            source_lines: 20,
            analyzed_lines: 16,
            duplicate_groups: 0,
            occurrences: 0,
            duplicate_lines: 0,
            duplication_percent: 0.0,
            context: ContextSummary::default(),
            scope: ScopeSummary::default(),
            distribution: DistributionSummary::default(),
            hotspots: Vec::new(),
        }
    }

    fn incomplete_summary() -> Summary {
        let mut analysis = analysis();
        analysis.keep_going = true;
        analysis.ignore_files = false;

        Summary {
            schema_version: SUMMARY_SCHEMA_VERSION,
            tool_version: "2.2.0",
            complete: false,
            analysis,
            errors: vec![OperationalError::new(ErrorKind::Parse, "broken source")],
            files: 1,
            files_with_duplicates: 0,
            source_lines: 10,
            analyzed_lines: 8,
            duplicate_groups: 0,
            occurrences: 0,
            duplicate_lines: 0,
            duplication_percent: 0.0,
            context: ContextSummary::default(),
            scope: ScopeSummary::default(),
            distribution: DistributionSummary::default(),
            hotspots: Vec::new(),
        }
    }

    #[test]
    fn complete_document_matches_frozen_fixture() {
        let json = render_summary_json(&complete_summary()).unwrap();

        assert_eq!(
            json,
            include_str!("../schemas/fixtures/summary-v1-complete.json").trim_end()
        );
    }

    #[test]
    fn zero_document_matches_frozen_fixture() {
        let json = render_summary_json(&zero_summary()).unwrap();

        assert_eq!(
            json,
            include_str!("../schemas/fixtures/summary-v1-zero.json").trim_end()
        );
    }

    #[test]
    fn incomplete_document_matches_frozen_fixture() {
        let json = render_summary_json(&incomplete_summary()).unwrap();

        assert_eq!(
            json,
            include_str!("../schemas/fixtures/summary-v1-incomplete.json").trim_end()
        );
    }

    #[test]
    fn repeated_rendering_is_byte_identical_and_contains_no_runtime_metadata() {
        let summary = complete_summary();
        let first = render_summary_json(&summary).unwrap();
        let second = render_summary_json(&summary).unwrap();
        let value: serde_json::Value = serde_json::from_str(&first).unwrap();

        assert_eq!(first, second);
        assert!(value.get("workers").is_none());
        assert!(value.get("elapsed").is_none());
        assert!(value.get("total_time").is_none());
    }
}
