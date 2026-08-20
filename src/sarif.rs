use serde::Serialize;
use thiserror::Error;

use crate::report::{Finding, FindingContext, FindingDistribution, FindingScope, Location, Report};

const SARIF_VERSION: &str = "2.1.0";
const SARIF_SCHEMA: &str =
    "https://docs.oasis-open.org/sarif/sarif/v2.1.0/errata01/os/schemas/sarif-schema-2.1.0.json";
#[cfg(test)]
const ARID_FINDING_FINGERPRINT_KEY: &str = "aridFindingFingerprint/v1";

#[derive(Serialize)]
struct SarifDocument {
    version: &'static str,
    #[serde(rename = "$schema")]
    schema: &'static str,
    runs: Vec<SarifRun>,
}

#[derive(Serialize)]
struct SarifRun {
    tool: SarifTool,
    results: Vec<SarifResult>,
}

#[derive(Serialize)]
struct SarifTool {
    driver: SarifDriver,
}

#[derive(Serialize)]
struct SarifDriver {
    name: &'static str,
    version: &'static str,
    rules: Vec<SarifRule>,
}

#[derive(Serialize)]
struct SarifRule {
    id: &'static str,
    #[serde(rename = "shortDescription")]
    short_description: SarifMessage,
}

#[derive(Serialize)]
struct SarifResult {
    #[serde(rename = "ruleId")]
    rule_id: String,
    message: SarifMessage,
    #[serde(rename = "partialFingerprints")]
    partial_fingerprints: SarifPartialFingerprints,
    locations: Vec<SarifLocation>,
    #[serde(rename = "relatedLocations", skip_serializing_if = "Vec::is_empty")]
    related_locations: Vec<SarifLocation>,
    properties: SarifProperties,
}

#[derive(Serialize)]
struct SarifPartialFingerprints {
    #[serde(rename = "aridFindingFingerprint/v1")]
    arid_finding_fingerprint_v1: String,
}

#[derive(Serialize)]
struct SarifMessage {
    text: String,
}

#[derive(Serialize)]
struct SarifLocation {
    #[serde(rename = "physicalLocation")]
    physical_location: SarifPhysicalLocation,
}

#[derive(Serialize)]
struct SarifPhysicalLocation {
    #[serde(rename = "artifactLocation")]
    artifact_location: SarifArtifactLocation,
    region: SarifRegion,
}

#[derive(Serialize)]
struct SarifArtifactLocation {
    uri: String,
}

#[derive(Serialize)]
struct SarifRegion {
    #[serde(rename = "startLine")]
    start_line: u64,
    #[serde(rename = "endLine")]
    end_line: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    snippet: Option<SarifMessage>,
}

#[derive(Serialize)]
struct SarifProperties {
    lines: u32,
    context: &'static str,
    scope: &'static str,
    occurrences: u32,
    files: u32,
    distribution: &'static str,
}

#[derive(Debug, Error)]
pub(crate) enum SarifError {
    #[error("cannot render SARIF for an incomplete report")]
    IncompleteReport,

    #[error("failed to serialize SARIF report: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn render_sarif(report: &Report) -> Result<String, SarifError> {
    if !report.complete {
        return Err(SarifError::IncompleteReport);
    }

    let document = SarifDocument {
        version: SARIF_VERSION,
        schema: SARIF_SCHEMA,
        runs: vec![SarifRun {
            tool: SarifTool {
                driver: SarifDriver {
                    name: "Arid",
                    version: report.tool_version,
                    rules: vec![SarifRule {
                        id: "DUP001",
                        short_description: SarifMessage {
                            text: "Duplicate code".to_owned(),
                        },
                    }],
                },
            },
            results: report.findings.iter().map(build_result).collect(),
        }],
    };

    Ok(serde_json::to_string_pretty(&document)?)
}

fn build_result(finding: &Finding) -> SarifResult {
    let (primary, related) = finding
        .locations
        .split_first()
        .expect("Arid findings always contain at least one location");

    SarifResult {
        rule_id: finding.code.clone(),
        message: SarifMessage {
            text: format!(
                "{} duplicated {} across {} occurrences in {} {}",
                finding.lines,
                if finding.lines == 1 { "line" } else { "lines" },
                finding.occurrences,
                finding.files,
                if finding.files == 1 { "file" } else { "files" },
            ),
        },
        partial_fingerprints: SarifPartialFingerprints {
            arid_finding_fingerprint_v1: finding.fingerprint.clone(),
        },
        locations: vec![build_location(primary)],
        related_locations: related.iter().map(build_location).collect(),
        properties: SarifProperties {
            lines: finding.lines,
            context: context_name(finding.context),
            scope: scope_name(finding.scope),
            occurrences: finding.occurrences,
            files: finding.files,
            distribution: distribution_name(finding.distribution),
        },
    }
}

fn build_location(location: &Location) -> SarifLocation {
    SarifLocation {
        physical_location: SarifPhysicalLocation {
            artifact_location: SarifArtifactLocation {
                uri: relative_uri(&location.path),
            },
            region: SarifRegion {
                start_line: location.start_line,
                end_line: location.end_line,
                snippet: location.source.as_ref().map(|source| SarifMessage {
                    text: source.clone(),
                }),
            },
        },
    }
}

fn relative_uri(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let mut uri = String::with_capacity(normalized.len());

    for byte in normalized.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/') {
            uri.push(char::from(byte));
        } else {
            uri.push('%');
            uri.push(hex_digit(byte >> 4));
            uri.push(hex_digit(byte & 0x0f));
        }
    }

    uri
}

const fn hex_digit(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        10..=15 => (b'A' + value - 10) as char,
        _ => unreachable!(),
    }
}

const fn context_name(context: FindingContext) -> &'static str {
    match context {
        FindingContext::Declarative => "declarative",
        FindingContext::Executable => "executable",
        FindingContext::Mixed => "mixed",
    }
}

const fn scope_name(scope: FindingScope) -> &'static str {
    match scope {
        FindingScope::Module => "module",
        FindingScope::Class => "class",
        FindingScope::Function => "function",
        FindingScope::Mixed => "mixed",
    }
}

const fn distribution_name(distribution: FindingDistribution) -> &'static str {
    match distribution {
        FindingDistribution::SameFile => "same-file",
        FindingDistribution::CrossFile => "cross-file",
        FindingDistribution::Hybrid => "hybrid",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::AnalysisMetadata;

    fn report(show_source: bool) -> Report {
        Report {
            schema_version: 4,
            tool_version: env!("CARGO_PKG_VERSION"),
            complete: true,
            analysis: AnalysisMetadata::default(),
            errors: Vec::new(),
            files: 2,
            source_lines: 4,
            analyzed_lines: 4,
            duplicate_groups: 1,
            duplicate_lines: 2,
            duplication_percent: 50.0,
            findings: vec![Finding {
                code: "DUP001".to_owned(),
                fingerprint: format!("arid-finding-v1:sha256:{}", "0".repeat(64)),
                lines: 2,
                context: FindingContext::Executable,
                scope: FindingScope::Function,
                occurrences: 2,
                files: 2,
                distribution: FindingDistribution::CrossFile,
                locations: vec![
                    Location {
                        path: "src/a file.py".to_owned(),
                        start_line: 1,
                        end_line: 2,
                        source: show_source.then(|| "alpha()\nbeta()".to_owned()),
                    },
                    Location {
                        path: "src/naïve.py".to_owned(),
                        start_line: 3,
                        end_line: 4,
                        source: show_source.then(|| "alpha()\nbeta()".to_owned()),
                    },
                ],
            }],
        }
    }

    #[test]
    fn renders_sarif_21_with_one_result_per_finding() {
        let rendered = render_sarif(&report(false)).unwrap();
        let value: serde_json::Value = serde_json::from_str(&rendered).unwrap();

        assert_eq!(value["version"], "2.1.0");
        assert_eq!(value["$schema"], SARIF_SCHEMA);
        assert_eq!(value["runs"].as_array().unwrap().len(), 1);
        assert_eq!(value["runs"][0]["tool"]["driver"]["name"], "Arid");
        assert_eq!(
            value["runs"][0]["tool"]["driver"]["version"],
            report(false).tool_version
        );
        assert_eq!(
            value["runs"][0]["tool"]["driver"]["rules"][0]["id"],
            "DUP001"
        );
        assert_eq!(value["runs"][0]["results"].as_array().unwrap().len(), 1);
        assert_eq!(value["runs"][0]["results"][0]["ruleId"], "DUP001");
        assert!(value["runs"][0]["results"][0].get("level").is_none());
    }

    #[test]
    fn exposes_arid_finding_identity_as_partial_fingerprint() {
        let report = report(false);
        let expected = report.findings[0].fingerprint.clone();
        let value: serde_json::Value =
            serde_json::from_str(&render_sarif(&report).unwrap()).unwrap();
        let result = &value["runs"][0]["results"][0];

        assert_eq!(
            result["partialFingerprints"][ARID_FINDING_FINGERPRINT_KEY],
            expected
        );
        assert!(
            result["partialFingerprints"]
                .get("primaryLocationLineHash")
                .is_none()
        );
    }

    #[test]
    fn first_occurrence_is_primary_and_rest_are_related() {
        let value: serde_json::Value =
            serde_json::from_str(&render_sarif(&report(false)).unwrap()).unwrap();
        let result = &value["runs"][0]["results"][0];

        assert_eq!(result["locations"].as_array().unwrap().len(), 1);
        assert_eq!(result["relatedLocations"].as_array().unwrap().len(), 1);
        assert_eq!(
            result["locations"][0]["physicalLocation"]["region"]["startLine"],
            1
        );
        assert_eq!(
            result["relatedLocations"][0]["physicalLocation"]["region"]["startLine"],
            3
        );
    }

    #[test]
    fn properties_preserve_objective_finding_metadata() {
        let value: serde_json::Value =
            serde_json::from_str(&render_sarif(&report(false)).unwrap()).unwrap();
        let properties = &value["runs"][0]["results"][0]["properties"];

        assert_eq!(properties["lines"], 2);
        assert_eq!(properties["context"], "executable");
        assert_eq!(properties["scope"], "function");
        assert_eq!(properties["occurrences"], 2);
        assert_eq!(properties["files"], 2);
        assert_eq!(properties["distribution"], "cross-file");
    }

    #[test]
    fn hybrid_distribution_is_preserved() {
        let mut report = report(false);
        report.findings[0].distribution = FindingDistribution::Hybrid;

        let value: serde_json::Value =
            serde_json::from_str(&render_sarif(&report).unwrap()).unwrap();

        assert_eq!(
            value["runs"][0]["results"][0]["properties"]["distribution"],
            "hybrid"
        );
    }

    #[test]
    fn relative_uri_normalizes_separators_and_percent_encodes_utf8() {
        assert_eq!(
            relative_uri("tests\\unit/a file.py"),
            "tests/unit/a%20file.py"
        );
        assert_eq!(relative_uri("src/naïve.py"), "src/na%C3%AFve.py");
        assert_eq!(relative_uri("src/a#b?.py"), "src/a%23b%3F.py");
    }

    #[test]
    fn show_source_adds_region_snippets() {
        let value: serde_json::Value =
            serde_json::from_str(&render_sarif(&report(true)).unwrap()).unwrap();
        let result = &value["runs"][0]["results"][0];

        assert_eq!(
            result["locations"][0]["physicalLocation"]["region"]["snippet"]["text"],
            "alpha()\nbeta()"
        );
        assert_eq!(
            result["relatedLocations"][0]["physicalLocation"]["region"]["snippet"]["text"],
            "alpha()\nbeta()"
        );
    }

    #[test]
    fn repeated_rendering_is_byte_identical() {
        let report = report(true);
        let first = render_sarif(&report).unwrap();
        let second = render_sarif(&report).unwrap();

        assert_eq!(first, second);
        assert!(!first.contains('\u{1b}'));
    }

    #[test]
    fn empty_report_has_no_results() {
        let report = Report {
            schema_version: 4,
            tool_version: env!("CARGO_PKG_VERSION"),
            complete: true,
            analysis: AnalysisMetadata::default(),
            errors: Vec::new(),
            files: 1,
            source_lines: 2,
            analyzed_lines: 2,
            duplicate_groups: 0,
            duplicate_lines: 0,
            duplication_percent: 0.0,
            findings: Vec::new(),
        };

        let value: serde_json::Value =
            serde_json::from_str(&render_sarif(&report).unwrap()).unwrap();

        assert!(value["runs"][0]["results"].as_array().unwrap().is_empty());
    }

    #[test]
    fn incomplete_report_cannot_be_rendered_as_sarif() {
        let mut report = report(false);
        report.complete = false;

        assert!(matches!(
            render_sarif(&report),
            Err(SarifError::IncompleteReport)
        ));
    }
}
