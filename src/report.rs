use std::fmt::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;

use crate::corpus::Corpus;
pub use crate::error::OperationalError;
use crate::fingerprint::finding_fingerprint;
use crate::metrics::{MetricsError, calculate_metrics};
use crate::model::{
    DuplicateGroup, NormalizedLine, Occurrence, StructuralContext, StructuralScope,
};
use crate::outcome::ExitStatus;

pub const DUPLICATE_CODE: &str = "DUP001";
pub const JSON_SCHEMA_VERSION: u8 = 4;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AnalysisMetadata {
    pub min_lines: u32,
    pub ignore_comments: bool,
    pub ignore_docstrings: bool,
    pub ignore_imports: bool,
    pub ignore_signatures: bool,
    pub same_file: bool,
    pub hidden: bool,
    pub exclude: Vec<String>,
    pub baseline_enabled: bool,
    pub focus: Vec<String>,
    pub virtual_source: Option<String>,
    pub keep_going: bool,
}

impl Default for AnalysisMetadata {
    fn default() -> Self {
        Self {
            min_lines: 4,
            ignore_comments: true,
            ignore_docstrings: true,
            ignore_imports: true,
            ignore_signatures: true,
            same_file: true,
            hidden: false,
            exclude: Vec::new(),
            baseline_enabled: false,
            focus: Vec::new(),
            virtual_source: None,
            keep_going: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportOptions {
    pub show_source: bool,

    /// Optional root removed from displayed paths.
    ///
    /// The CLI will normally set this to the resolved project root so
    /// diagnostics use concise project-relative paths.
    pub path_root: Option<PathBuf>,

    pub analysis: AnalysisMetadata,
    pub complete: bool,
    pub errors: Vec<OperationalError>,
}

impl Default for ReportOptions {
    fn default() -> Self {
        Self {
            show_source: false,
            path_root: None,
            analysis: AnalysisMetadata::default(),
            complete: true,
            errors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    pub schema_version: u8,
    pub tool_version: &'static str,
    pub complete: bool,
    pub analysis: AnalysisMetadata,
    pub errors: Vec<OperationalError>,
    pub files: u64,
    pub source_lines: u64,
    pub analyzed_lines: u64,
    pub duplicate_groups: u64,
    pub duplicate_lines: u64,
    pub duplication_percent: f64,
    pub findings: Vec<Finding>,
}

impl Report {
    #[must_use]
    pub fn has_findings(&self) -> bool {
        !self.findings.is_empty()
    }

    #[must_use]
    pub fn exit_status(&self) -> ExitStatus {
        if !self.complete {
            ExitStatus::Error
        } else if self.has_findings() {
            ExitStatus::Findings
        } else {
            ExitStatus::Success
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FindingContext {
    Declarative,
    Executable,
    Mixed,
}

impl FindingContext {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Declarative => "declarative",
            Self::Executable => "executable",
            Self::Mixed => "mixed",
        }
    }
}

impl From<StructuralContext> for FindingContext {
    fn from(context: StructuralContext) -> Self {
        match context {
            StructuralContext::Declarative => Self::Declarative,
            StructuralContext::Executable => Self::Executable,
            StructuralContext::Mixed => Self::Mixed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FindingScope {
    Module,
    Class,
    Function,
    Mixed,
}

impl FindingScope {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Module => "module",
            Self::Class => "class",
            Self::Function => "function",
            Self::Mixed => "mixed",
        }
    }
}

impl From<StructuralScope> for FindingScope {
    fn from(scope: StructuralScope) -> Self {
        match scope {
            StructuralScope::Module => Self::Module,
            StructuralScope::Class => Self::Class,
            StructuralScope::Function => Self::Function,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FindingDistribution {
    SameFile,
    CrossFile,
    Hybrid,
}

impl FindingDistribution {
    const fn as_str(self) -> &'static str {
        match self {
            Self::SameFile => "same-file",
            Self::CrossFile => "cross-file",
            Self::Hybrid => "hybrid",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub code: String,
    pub fingerprint: String,
    pub lines: u32,
    pub context: FindingContext,
    pub scope: FindingScope,
    pub occurrences: u32,
    pub files: u32,
    pub distribution: FindingDistribution,
    pub locations: Vec<Location>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Location {
    pub path: String,
    pub start_line: u64,
    pub end_line: u64,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Error)]
pub enum ReportError {
    #[error(transparent)]
    Metrics(#[from] MetricsError),

    #[error("failed to fingerprint duplicate finding: {0}")]
    Fingerprint(String),

    #[error("duplicate occurrence references unknown file {file}")]
    UnknownFile { file: u32 },

    #[error(
        "duplicate occurrence in file {file} has invalid normalized range \
         {start}..{end}"
    )]
    InvalidOccurrence { file: u32, start: u32, end: u64 },

    #[error("failed to serialize JSON report: {0}")]
    Json(#[from] serde_json::Error),
}

/// Builds the stable report-v4 document from duplicate groups.
///
/// Metrics are calculated from the same corpus and findings rather than being
/// accepted separately, preventing callers from accidentally combining stale
/// metrics with different findings.
pub fn build_report(
    corpus: &Corpus,
    groups: &[DuplicateGroup],
    options: &ReportOptions,
) -> Result<Report, ReportError> {
    let metrics = calculate_metrics(corpus, groups)?;

    let mut findings = groups
        .iter()
        .map(|group| build_finding(corpus, group, options))
        .collect::<Result<Vec<_>, _>>()?;

    sort_findings(&mut findings);

    Ok(Report {
        schema_version: JSON_SCHEMA_VERSION,
        tool_version: env!("CARGO_PKG_VERSION"),
        complete: options.complete,
        analysis: options.analysis.clone(),
        errors: options.errors.clone(),
        files: metrics.files,
        source_lines: metrics.source_lines,
        analyzed_lines: metrics.analyzed_lines,
        duplicate_groups: metrics.duplicate_groups,
        duplicate_lines: metrics.duplicate_lines,
        duplication_percent: metrics.duplication_percent,
        findings,
    })
}

/// Renders the stable plain-text diagnostic output.
#[must_use]
pub fn render_text_plain(report: &Report) -> String {
    let mut output = String::new();

    for finding in &report.findings {
        let unit = if finding.lines == 1 { "line" } else { "lines" };
        let file_unit = if finding.files == 1 { "file" } else { "files" };

        writeln!(
            &mut output,
            "{} {} duplicated {}",
            finding.code, finding.lines, unit,
        )
        .expect("writing to String cannot fail");

        writeln!(&mut output, "Context: {}", finding.context.as_str())
            .expect("writing to String cannot fail");

        writeln!(&mut output, "Scope: {}", finding.scope.as_str())
            .expect("writing to String cannot fail");

        writeln!(
            &mut output,
            "Occurrences: {} across {} {} ({})",
            finding.occurrences,
            finding.files,
            file_unit,
            finding.distribution.as_str(),
        )
        .expect("writing to String cannot fail");

        writeln!(&mut output).expect("writing to String cannot fail");

        for location in &finding.locations {
            writeln!(
                &mut output,
                "  {}:{}-{}",
                location.path, location.start_line, location.end_line,
            )
            .expect("writing to String cannot fail");

            if let Some(source) = &location.source {
                for (offset, line) in source.lines().enumerate() {
                    let line_number = location.start_line + offset as u64;

                    writeln!(&mut output, "    {:>4} | {}", line_number, line,)
                        .expect("writing to String cannot fail");
                }
            }
        }

        writeln!(&mut output).expect("writing to String cannot fail");
    }

    if report.findings.is_empty() {
        writeln!(&mut output, "No duplicate code found.").expect("writing to String cannot fail");
    } else {
        let group_unit = if report.duplicate_groups == 1 {
            "group"
        } else {
            "groups"
        };

        writeln!(
            &mut output,
            "Found {} duplicate {}.",
            report.duplicate_groups, group_unit,
        )
        .expect("writing to String cannot fail");
    }

    let line_unit = if report.duplicate_lines == 1 {
        "line"
    } else {
        "lines"
    };

    writeln!(
        &mut output,
        "{} duplicate {} ({:.2}%).",
        report.duplicate_lines, line_unit, report.duplication_percent,
    )
    .expect("writing to String cannot fail");

    output
}

/// Serializes report v4 using deterministic pretty JSON formatting.
pub fn render_json(report: &Report) -> Result<String, ReportError> {
    Ok(serde_json::to_string_pretty(report)?)
}

fn build_finding(
    corpus: &Corpus,
    group: &DuplicateGroup,
    options: &ReportOptions,
) -> Result<Finding, ReportError> {
    let fingerprint = finding_fingerprint(corpus, group)
        .map_err(|error| ReportError::Fingerprint(error.to_string()))?;
    let mut occurrences = group.occurrences.clone();
    occurrences.sort_unstable();

    let (file_count, distribution) = classify_distribution(&occurrences);

    let occurrence_count = u32::try_from(occurrences.len())
        .expect("a duplicate group cannot contain more than u32::MAX occurrences");

    let mut context = None;
    let mut scope = None;
    let mut locations = Vec::with_capacity(occurrences.len());

    for occurrence in occurrences {
        let (location, occurrence_context, occurrence_scope) =
            build_location(corpus, occurrence, options)?;

        context = Some(match context {
            None => occurrence_context,
            Some(current) if current == occurrence_context => current,
            Some(_) => FindingContext::Mixed,
        });

        scope = Some(match scope {
            None => occurrence_scope,
            Some(current) if current == occurrence_scope => current,
            Some(_) => FindingScope::Mixed,
        });

        locations.push(location);
    }

    locations.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.start_line.cmp(&right.start_line))
            .then_with(|| left.end_line.cmp(&right.end_line))
    });

    Ok(Finding {
        code: DUPLICATE_CODE.to_owned(),
        fingerprint,
        lines: group.effective_lines,
        context: context.expect("validated duplicate group must contain an occurrence"),
        scope: scope.expect("validated duplicate group must contain an occurrence"),
        occurrences: occurrence_count,
        files: file_count,
        distribution,
        locations,
    })
}

fn classify_distribution(occurrences: &[Occurrence]) -> (u32, FindingDistribution) {
    let mut file_count = 0_u32;
    let mut previous_file = None;
    let mut repeated_file = false;

    for occurrence in occurrences {
        if previous_file == Some(occurrence.file) {
            repeated_file = true;
        } else {
            file_count = file_count
                .checked_add(1)
                .expect("a duplicate group cannot reference more than u32::MAX files");

            previous_file = Some(occurrence.file);
        }
    }

    let distribution = if file_count <= 1 {
        FindingDistribution::SameFile
    } else if repeated_file {
        FindingDistribution::Hybrid
    } else {
        FindingDistribution::CrossFile
    };

    (file_count, distribution)
}

fn build_location(
    corpus: &Corpus,
    occurrence: Occurrence,
    options: &ReportOptions,
) -> Result<(Location, FindingContext, FindingScope), ReportError> {
    let Some(file) = corpus.files.get(occurrence.file as usize) else {
        return Err(ReportError::UnknownFile {
            file: occurrence.file,
        });
    };

    let start = occurrence.normalized_start as usize;
    let end = u64::from(occurrence.normalized_start) + u64::from(occurrence.normalized_len);

    let Some(lines) = file.lines.get(start..end as usize) else {
        return Err(ReportError::InvalidOccurrence {
            file: occurrence.file,
            start: occurrence.normalized_start,
            end,
        });
    };

    let Some(first) = lines.first() else {
        return Err(ReportError::InvalidOccurrence {
            file: occurrence.file,
            start: occurrence.normalized_start,
            end,
        });
    };

    let Some(last) = lines.last() else {
        return Err(ReportError::InvalidOccurrence {
            file: occurrence.file,
            start: occurrence.normalized_start,
            end,
        });
    };

    let (context, scope) = classify_lines(lines);

    let start_source_line = first.source_line;
    let end_source_line = last.source_line;

    let source = options
        .show_source
        .then(|| extract_source(&file.source, start_source_line, end_source_line));

    Ok((
        Location {
            path: display_path(&file.path, options.path_root.as_deref()),
            start_line: u64::from(start_source_line) + 1,
            end_line: u64::from(end_source_line) + 1,
            source,
        },
        context,
        scope,
    ))
}

fn classify_lines(lines: &[NormalizedLine]) -> (FindingContext, FindingScope) {
    let first = lines
        .first()
        .expect("validated occurrence must contain a normalized line");

    let mut context = FindingContext::from(first.context);
    let mut scope = FindingScope::from(first.scope);

    for line in &lines[1..] {
        let next_context = FindingContext::from(line.context);
        let next_scope = FindingScope::from(line.scope);

        if context != next_context {
            context = FindingContext::Mixed;
        }

        if scope != next_scope {
            scope = FindingScope::Mixed;
        }
    }

    (context, scope)
}

fn extract_source(source: &str, start_line: u32, end_line: u32) -> String {
    let count = u64::from(end_line) - u64::from(start_line) + 1;

    source
        .lines()
        .skip(start_line as usize)
        .take(count as usize)
        .collect::<Vec<_>>()
        .join("\n")
}

fn display_path(path: &Path, root: Option<&Path>) -> String {
    let display = root
        .and_then(|root| path.strip_prefix(root).ok())
        .unwrap_or(path);

    display.to_string_lossy().into_owned()
}

fn sort_findings(findings: &mut [Finding]) {
    findings.sort_by(|left, right| {
        let left_location = left
            .locations
            .first()
            .expect("validated finding must contain a location");

        let right_location = right
            .locations
            .first()
            .expect("validated finding must contain a location");

        left_location
            .path
            .cmp(&right_location.path)
            .then_with(|| left_location.start_line.cmp(&right_location.start_line))
            .then_with(|| right.lines.cmp(&left.lines))
            .then_with(|| compare_locations(&left.locations, &right.locations))
    });
}

fn compare_locations(left: &[Location], right: &[Location]) -> std::cmp::Ordering {
    for (left_location, right_location) in left.iter().zip(right) {
        let ordering = left_location
            .path
            .cmp(&right_location.path)
            .then_with(|| left_location.start_line.cmp(&right_location.start_line))
            .then_with(|| left_location.end_line.cmp(&right_location.end_line));

        if !ordering.is_eq() {
            return ordering;
        }
    }

    left.len().cmp(&right.len())
}

#[cfg(test)]
mod tests {
    use std::ops::Range;
    use std::path::PathBuf;

    use crate::corpus::build_corpus;
    use crate::model::{
        NormalizedLine, NormalizedSegment, PreparedFile, StructuralContext, StructuralScope,
    };

    use super::*;

    fn prepared(path: &str, source: &str, normalized: &[(&str, u32, bool)]) -> PreparedFile {
        let mut normalized_source = String::new();
        let mut lines = Vec::new();

        for &(text, source_line, effective) in normalized {
            let start = normalized_source.len() as u32;
            normalized_source.push_str(text);
            let end = normalized_source.len() as u32;
            normalized_source.push('\n');

            lines.push(NormalizedLine {
                text_range: Range { start, end },
                source_line,
                effective,
                context: StructuralContext::Executable,
                scope: StructuralScope::Module,
            });
        }

        let line_count = lines.len() as u32;

        PreparedFile {
            path: PathBuf::from(path),
            source: source.to_owned(),
            normalized: normalized_source,
            lines,
            segments: if line_count == 0 {
                Vec::new()
            } else {
                vec![NormalizedSegment {
                    start: 0,
                    end: line_count,
                }]
            },
        }
    }

    fn occurrence(file: u32, start: u32, length: u32) -> Occurrence {
        Occurrence {
            file,
            normalized_start: start,
            normalized_len: length,
        }
    }

    #[test]
    fn builds_physical_source_locations() {
        let corpus = build_corpus(vec![
            prepared(
                "a.py",
                "alpha()\n# ignored\nbeta()\n",
                &[("alpha()", 0, true), ("beta()", 2, true)],
            ),
            prepared(
                "b.py",
                "alpha()\n# ignored\nbeta()\n",
                &[("alpha()", 0, true), ("beta()", 2, true)],
            ),
        ])
        .unwrap();

        let groups = vec![DuplicateGroup {
            effective_lines: 2,
            normalized_len: 2,
            occurrences: vec![occurrence(0, 0, 2), occurrence(1, 0, 2)],
        }];

        let report = build_report(&corpus, &groups, &ReportOptions::default()).unwrap();

        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].locations,
            vec![
                Location {
                    path: "a.py".to_owned(),
                    start_line: 1,
                    end_line: 3,
                    source: None,
                },
                Location {
                    path: "b.py".to_owned(),
                    start_line: 1,
                    end_line: 3,
                    source: None,
                },
            ]
        );
    }

    #[test]
    fn includes_original_source_when_requested() {
        let corpus = build_corpus(vec![
            prepared(
                "a.py",
                "alpha()\n# ignored\nbeta()\n",
                &[("alpha()", 0, true), ("beta()", 2, true)],
            ),
            prepared(
                "b.py",
                "alpha()\n# ignored\nbeta()\n",
                &[("alpha()", 0, true), ("beta()", 2, true)],
            ),
        ])
        .unwrap();

        let groups = vec![DuplicateGroup {
            effective_lines: 2,
            normalized_len: 2,
            occurrences: vec![occurrence(0, 0, 2), occurrence(1, 0, 2)],
        }];

        let report = build_report(
            &corpus,
            &groups,
            &ReportOptions {
                show_source: true,
                ..ReportOptions::default()
            },
        )
        .unwrap();

        assert_eq!(
            report.findings[0].locations[0].source.as_deref(),
            Some("alpha()\n# ignored\nbeta()")
        );
    }

    #[test]
    fn strips_configured_path_root() {
        let corpus = build_corpus(vec![
            prepared(
                "/project/src/a.py",
                "alpha()\nbeta()\n",
                &[("alpha()", 0, true), ("beta()", 1, true)],
            ),
            prepared(
                "/project/src/b.py",
                "alpha()\nbeta()\n",
                &[("alpha()", 0, true), ("beta()", 1, true)],
            ),
        ])
        .unwrap();

        let groups = vec![DuplicateGroup {
            effective_lines: 2,
            normalized_len: 2,
            occurrences: vec![occurrence(0, 0, 2), occurrence(1, 0, 2)],
        }];

        let report = build_report(
            &corpus,
            &groups,
            &ReportOptions {
                path_root: Some(PathBuf::from("/project")),
                ..ReportOptions::default()
            },
        )
        .unwrap();

        assert_eq!(report.findings[0].locations[0].path, "src/a.py");
    }

    #[test]
    fn report_contains_v4_metadata_and_duplication_metrics() {
        let common = [("alpha()", 0, true), ("beta()", 1, true)];
        let corpus = build_corpus(vec![
            prepared("a.py", "alpha()\nbeta()\n", &common),
            prepared("b.py", "alpha()\nbeta()\n", &common),
        ])
        .unwrap();
        let groups = vec![DuplicateGroup {
            effective_lines: 2,
            normalized_len: 2,
            occurrences: vec![occurrence(0, 0, 2), occurrence(1, 0, 2)],
        }];

        let report = build_report(&corpus, &groups, &ReportOptions::default()).unwrap();

        assert_eq!(report.schema_version, 4);
        assert_eq!(report.tool_version, env!("CARGO_PKG_VERSION"));
        assert!(report.complete);
        assert!(report.errors.is_empty());
        assert_eq!(report.files, 2);
        assert_eq!(report.source_lines, 4);
        assert_eq!(report.analyzed_lines, 4);
        assert_eq!(report.duplicate_groups, 1);
        assert_eq!(report.duplicate_lines, 2);
        assert_eq!(report.duplication_percent, 50.0);
        assert_eq!(
            report.findings[0].fingerprint,
            "arid-finding-v1:sha256:3ebe4dc689f0253ce65179b1ff6bc32d3cc1a33a0e4b3884a6e16b5ee6f9a0b6"
        );
    }

    #[test]
    fn text_output_matches_diagnostic_contract() {
        let common = [("alpha()", 0, true), ("beta()", 1, true)];
        let corpus = build_corpus(vec![
            prepared("a.py", "alpha()\nbeta()\n", &common),
            prepared("b.py", "alpha()\nbeta()\n", &common),
        ])
        .unwrap();
        let groups = vec![DuplicateGroup {
            effective_lines: 2,
            normalized_len: 2,
            occurrences: vec![occurrence(0, 0, 2), occurrence(1, 0, 2)],
        }];
        let report = build_report(&corpus, &groups, &ReportOptions::default()).unwrap();

        assert_eq!(
            render_text_plain(&report),
            concat!(
                "DUP001 2 duplicated lines\n",
                "Context: executable\n",
                "Scope: module\n",
                "Occurrences: 2 across 2 files (cross-file)\n",
                "\n",
                "  a.py:1-2\n",
                "  b.py:1-2\n",
                "\n",
                "Found 1 duplicate group.\n",
                "2 duplicate lines (50.00%).\n",
            )
        );
    }

    #[test]
    fn finding_structure_is_mixed_across_different_occurrence_contexts() {
        let mut class_file = prepared("a.py", "value = 1\n", &[("value = 1", 0, true)]);
        class_file.lines[0].context = StructuralContext::Declarative;
        class_file.lines[0].scope = StructuralScope::Class;

        let mut function_file = prepared("b.py", "value = 1\n", &[("value = 1", 0, true)]);
        function_file.lines[0].context = StructuralContext::Executable;
        function_file.lines[0].scope = StructuralScope::Function;

        let corpus = build_corpus(vec![class_file, function_file]).unwrap();
        let groups = vec![DuplicateGroup {
            effective_lines: 1,
            normalized_len: 1,
            occurrences: vec![occurrence(0, 0, 1), occurrence(1, 0, 1)],
        }];
        let report = build_report(&corpus, &groups, &ReportOptions::default()).unwrap();

        assert_eq!(report.findings[0].context, FindingContext::Mixed);
        assert_eq!(report.findings[0].scope, FindingScope::Mixed);
    }

    #[test]
    fn finding_reports_hybrid_distribution() {
        let corpus = build_corpus(vec![
            prepared(
                "a.py",
                "alpha()\nalpha()\n",
                &[("alpha()", 0, true), ("alpha()", 1, true)],
            ),
            prepared("b.py", "alpha()\n", &[("alpha()", 0, true)]),
        ])
        .unwrap();
        let groups = vec![DuplicateGroup {
            effective_lines: 1,
            normalized_len: 1,
            occurrences: vec![
                occurrence(0, 0, 1),
                occurrence(0, 1, 1),
                occurrence(1, 0, 1),
            ],
        }];
        let report = build_report(&corpus, &groups, &ReportOptions::default()).unwrap();

        assert_eq!(report.findings[0].distribution, FindingDistribution::Hybrid);
    }

    #[test]
    fn incomplete_report_has_error_status() {
        let corpus = build_corpus(Vec::new()).unwrap();
        let report = build_report(
            &corpus,
            &[],
            &ReportOptions {
                complete: false,
                ..ReportOptions::default()
            },
        )
        .unwrap();

        assert_eq!(report.exit_status(), ExitStatus::Error);
    }

    #[test]
    fn json_uses_schema_version_four() {
        let corpus = build_corpus(Vec::new()).unwrap();
        let report = build_report(&corpus, &[], &ReportOptions::default()).unwrap();
        let json = render_json(&report).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["schema_version"], 4);
        assert!(value.get("version").is_none());
        assert_eq!(value["tool_version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(value["complete"], true);
        assert_eq!(value["analysis"]["min_lines"], 4);
        assert_eq!(value["analysis"]["focus"], serde_json::json!([]));
        assert_eq!(value["analysis"]["virtual_source"], serde_json::Value::Null);
        assert_eq!(value["errors"], serde_json::json!([]));
        assert_eq!(value["findings"], serde_json::json!([]));
    }

    #[test]
    fn json_finding_contains_fingerprint() {
        let corpus = build_corpus(vec![
            prepared("a.py", "alpha()\n", &[("alpha()", 0, true)]),
            prepared("b.py", "alpha()\n", &[("alpha()", 0, true)]),
        ])
        .unwrap();
        let groups = vec![DuplicateGroup {
            effective_lines: 1,
            normalized_len: 1,
            occurrences: vec![occurrence(0, 0, 1), occurrence(1, 0, 1)],
        }];
        let report = build_report(&corpus, &groups, &ReportOptions::default()).unwrap();
        let value: serde_json::Value =
            serde_json::from_str(&render_json(&report).unwrap()).unwrap();

        assert!(
            value["findings"][0]["fingerprint"]
                .as_str()
                .unwrap()
                .starts_with("arid-finding-v1:sha256:")
        );
        assert_eq!(value["findings"][0]["distribution"], "cross-file");
    }

    #[test]
    fn findings_are_sorted_by_source_location() {
        let corpus = build_corpus(vec![
            prepared(
                "a.py",
                "one()\ntwo()\nthree()\nfour()\n",
                &[
                    ("one()", 0, true),
                    ("two()", 1, true),
                    ("three()", 2, true),
                    ("four()", 3, true),
                ],
            ),
            prepared(
                "b.py",
                "one()\ntwo()\nthree()\nfour()\n",
                &[
                    ("one()", 0, true),
                    ("two()", 1, true),
                    ("three()", 2, true),
                    ("four()", 3, true),
                ],
            ),
        ])
        .unwrap();
        let groups = vec![
            DuplicateGroup {
                effective_lines: 2,
                normalized_len: 2,
                occurrences: vec![occurrence(0, 2, 2), occurrence(1, 2, 2)],
            },
            DuplicateGroup {
                effective_lines: 2,
                normalized_len: 2,
                occurrences: vec![occurrence(0, 0, 2), occurrence(1, 0, 2)],
            },
        ];
        let report = build_report(&corpus, &groups, &ReportOptions::default()).unwrap();

        assert_eq!(report.findings[0].locations[0].start_line, 1);
        assert_eq!(report.findings[1].locations[0].start_line, 3);
    }
}
