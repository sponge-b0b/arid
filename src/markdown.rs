use std::fmt::Write;

use crate::report::{FindingContext, FindingDistribution, FindingScope, Report};

#[must_use]
pub fn render_markdown(report: &Report) -> String {
    let mut output = String::new();

    output.push_str("# Arid duplicate-code report\n\n");

    if !report.complete {
        output.push_str(
            "> **Warning:** Scan incomplete. Results include only successfully processed source.\n\n",
        );

        if !report.errors.is_empty() {
            output.push_str("## Processing errors\n\n");

            for error in &report.errors {
                output.push_str("- ");

                if let Some(path) = &error.path {
                    write_inline_code(&mut output, path);
                    output.push_str(": ");
                }

                output.push_str(&error.message);
                output.push('\n');
            }

            output.push('\n');
        }
    }

    if report.findings.is_empty() {
        if report.complete {
            output.push_str("No duplicate code found.\n\n");
        } else {
            output.push_str("No duplicate code found in successfully processed source.\n\n");
        }
    }

    for finding in &report.findings {
        let line_unit = if finding.lines == 1 { "line" } else { "lines" };
        let file_unit = if finding.files == 1 { "file" } else { "files" };

        writeln!(
            &mut output,
            "## `{}` — {} duplicated {line_unit}",
            finding.code, finding.lines,
        )
        .expect("writing to String cannot fail");
        output.push('\n');

        writeln!(
            &mut output,
            "**Context:** {}  ",
            context_name(finding.context)
        )
        .expect("writing to String cannot fail");
        writeln!(&mut output, "**Scope:** {}  ", scope_name(finding.scope))
            .expect("writing to String cannot fail");
        writeln!(
            &mut output,
            "**Occurrences:** {} across {} {file_unit} _({})_",
            finding.occurrences,
            finding.files,
            distribution_name(finding.distribution),
        )
        .expect("writing to String cannot fail");
        output.push('\n');

        for location in &finding.locations {
            output.push_str("### ");
            let location_text = format!(
                "{}:{}-{}",
                location.path, location.start_line, location.end_line
            );
            write_inline_code(&mut output, &location_text);
            output.push_str("\n\n");

            if let Some(source) = &location.source {
                write_source_fence(&mut output, source);
                output.push('\n');
            }
        }
    }

    output.push_str("## Summary\n\n");
    writeln!(&mut output, "- **Files:** {}", report.files).expect("writing to String cannot fail");
    writeln!(&mut output, "- **Source lines:** {}", report.source_lines)
        .expect("writing to String cannot fail");
    writeln!(
        &mut output,
        "- **Analyzed lines:** {}",
        report.analyzed_lines
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut output,
        "- **Duplicate groups:** {}",
        report.duplicate_groups
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut output,
        "- **Duplicate lines:** {}",
        report.duplicate_lines
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut output,
        "- **Duplication:** {:.2}%",
        report.duplication_percent
    )
    .expect("writing to String cannot fail");

    output
}

fn write_inline_code(output: &mut String, value: &str) {
    let fence = "`".repeat(longest_backtick_run(value) + 1);
    let pad = value.starts_with('`') || value.ends_with('`');

    output.push_str(&fence);
    if pad {
        output.push(' ');
    }
    output.push_str(value);
    if pad {
        output.push(' ');
    }
    output.push_str(&fence);
}

fn write_source_fence(output: &mut String, source: &str) {
    let fence = "`".repeat((longest_backtick_run(source) + 1).max(3));

    writeln!(output, "{fence}python").expect("writing to String cannot fail");
    output.push_str(source);
    if !source.ends_with('\n') {
        output.push('\n');
    }
    writeln!(output, "{fence}").expect("writing to String cannot fail");
}

fn longest_backtick_run(value: &str) -> usize {
    let mut longest = 0;
    let mut current = 0;

    for byte in value.bytes() {
        if byte == b'`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }

    longest
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
    use std::path::Path;

    use super::*;
    use crate::error::{ErrorKind, OperationalError};
    use crate::report::{AnalysisMetadata, Finding, Location};

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
                        path: "a.py".to_owned(),
                        start_line: 1,
                        end_line: 2,
                        source: show_source.then(|| "alpha()\nbeta()".to_owned()),
                    },
                    Location {
                        path: "b.py".to_owned(),
                        start_line: 3,
                        end_line: 4,
                        source: show_source.then(|| "alpha()\nbeta()".to_owned()),
                    },
                ],
            }],
        }
    }

    #[test]
    fn renders_finding_metadata_locations_and_summary() {
        let rendered = render_markdown(&report(false));

        assert!(rendered.starts_with("# Arid duplicate-code report\n\n"));
        assert!(rendered.contains("## `DUP001` — 2 duplicated lines"));
        assert!(rendered.contains("**Context:** executable"));
        assert!(rendered.contains("**Scope:** function"));
        assert!(rendered.contains("**Occurrences:** 2 across 2 files _(cross-file)_"));
        assert!(rendered.contains("### `a.py:1-2`"));
        assert!(rendered.contains("### `b.py:3-4`"));
        assert!(rendered.contains("- **Duplicate groups:** 1"));
        assert!(rendered.contains("- **Duplicate lines:** 2"));
        assert!(rendered.contains("- **Duplication:** 50.00%"));
        assert!(!rendered.contains('\u{1b}'));
    }

    #[test]
    fn source_uses_raw_python_without_line_gutters() {
        let rendered = render_markdown(&report(true));

        assert!(rendered.contains("```python\nalpha()\nbeta()\n```"));
        assert!(!rendered.contains("1 |"));
        assert!(!rendered.contains("3 |"));
    }

    #[test]
    fn inline_code_outgrows_backticks_in_location() {
        let mut report = report(false);
        report.findings[0].locations[0].path = "odd`name.py".to_owned();

        let rendered = render_markdown(&report);

        assert!(rendered.contains("### ``odd`name.py:1-2``"));
    }

    #[test]
    fn source_fence_outgrows_backticks_in_source() {
        let mut report = report(true);
        report.findings[0].locations[0].source = Some("value = \"```\"".to_owned());

        let rendered = render_markdown(&report);

        assert!(rendered.contains("````python\nvalue = \"```\"\n````"));
    }

    #[test]
    fn repeated_rendering_is_byte_identical() {
        let report = report(true);
        let first = render_markdown(&report);
        let second = render_markdown(&report);

        assert_eq!(first, second);
    }

    #[test]
    fn renders_empty_report() {
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

        let rendered = render_markdown(&report);

        assert!(rendered.contains("No duplicate code found."));
        assert!(rendered.contains("- **Duplicate groups:** 0"));
        assert!(rendered.contains("- **Duplication:** 0.00%"));
    }

    #[test]
    fn incomplete_report_is_explicit_and_lists_errors() {
        let mut report = Report {
            schema_version: 4,
            tool_version: env!("CARGO_PKG_VERSION"),
            complete: false,
            analysis: AnalysisMetadata::default(),
            errors: vec![
                OperationalError::new(ErrorKind::Parse, "failed to parse source")
                    .with_project_path(Path::new("project/broken.py"), Path::new("project")),
            ],
            files: 1,
            source_lines: 2,
            analyzed_lines: 0,
            duplicate_groups: 0,
            duplicate_lines: 0,
            duplication_percent: 0.0,
            findings: Vec::new(),
        };

        report.analysis.keep_going = true;

        let rendered = render_markdown(&report);

        assert!(rendered.contains(
            "> **Warning:** Scan incomplete. Results include only successfully processed source."
        ));
        assert!(rendered.contains("## Processing errors"));
        assert!(rendered.contains("- `broken.py`: failed to parse source"));
        assert!(rendered.contains("No duplicate code found in successfully processed source."));
        assert!(!rendered.contains("\nNo duplicate code found.\n"));
    }
}
