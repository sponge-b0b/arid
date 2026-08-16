use std::fmt::{Display, Write};

use crate::report::{
    FindingContext, FindingDistribution, FindingScope, Report, render_human,
};

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const CYAN: &str = "\x1b[36m";
const BOLD_CYAN: &str = "\x1b[1;36m";
const BOLD_YELLOW: &str = "\x1b[1;33m";

#[derive(Debug, Clone, Copy)]
struct TextStyles {
    diagnostic: &'static str,
    heading: &'static str,
    path: &'static str,
    location: &'static str,
    secondary: &'static str,
    source_gutter: &'static str,
}

impl TextStyles {
    const fn colored() -> Self {
        Self {
            diagnostic: BOLD_YELLOW,
            heading: BOLD,
            path: BOLD_CYAN,
            location: CYAN,
            secondary: DIM,
            source_gutter: DIM,
        }
    }
}

#[must_use]
pub fn render_text(report: &Report, color: bool) -> String {
    if !color {
        return render_human(report);
    }

    let styles = TextStyles::colored();
    let mut output = String::new();

    for finding in &report.findings {
        let unit = if finding.lines == 1 { "line" } else { "lines" };
        let file_unit = if finding.files == 1 { "file" } else { "files" };

        write_styled(&mut output, styles.diagnostic, &finding.code);
        output.push(' ');
        write_styled(&mut output, styles.heading, finding.lines);
        writeln!(&mut output, " duplicated {unit}").expect("writing to String cannot fail");

        write_styled(&mut output, styles.heading, "Context:");
        writeln!(&mut output, " {}", context_name(finding.context))
            .expect("writing to String cannot fail");

        write_styled(&mut output, styles.heading, "Scope:");
        writeln!(&mut output, " {}", scope_name(finding.scope))
            .expect("writing to String cannot fail");

        write_styled(&mut output, styles.heading, "Occurrences:");
        output.push(' ');
        write_styled(&mut output, styles.heading, finding.occurrences);
        output.push_str(" across ");
        write_styled(&mut output, styles.heading, finding.files);
        output.push(' ');
        output.push_str(file_unit);
        output.push(' ');
        write_styled(
            &mut output,
            styles.secondary,
            format_args!("({})", distribution_name(finding.distribution)),
        );
        output.push('\n');
        output.push('\n');

        for location in &finding.locations {
            output.push_str("  ");
            write_styled(&mut output, styles.path, &location.path);
            write_styled(
                &mut output,
                styles.location,
                format_args!(":{}-{}", location.start_line, location.end_line),
            );
            output.push('\n');

            if let Some(source) = &location.source {
                for (offset, line) in source.lines().enumerate() {
                    let line_number = location.start_line + offset as u64;
                    write_styled(
                        &mut output,
                        styles.source_gutter,
                        format_args!("    {line_number:>4} | "),
                    );
                    writeln!(&mut output, "{line}").expect("writing to String cannot fail");
                }
            }
        }

        output.push('\n');
    }

    if report.findings.is_empty() {
        write_styled(&mut output, styles.heading, "No duplicate code found.");
        output.push('\n');
    } else {
        let group_unit = if report.duplicate_groups == 1 {
            "group"
        } else {
            "groups"
        };

        write_styled(
            &mut output,
            styles.heading,
            format_args!("Found {} duplicate {group_unit}.", report.duplicate_groups),
        );
        output.push('\n');
    }

    let line_unit = if report.duplicate_lines == 1 {
        "line"
    } else {
        "lines"
    };

    write_styled(
        &mut output,
        styles.heading,
        format_args!(
            "{} duplicate {line_unit} ({:.2}%).",
            report.duplicate_lines, report.duplication_percent,
        ),
    );
    output.push('\n');

    output
}

fn write_styled(output: &mut String, style: &str, value: impl Display) {
    write!(output, "{style}{value}{RESET}").expect("writing to String cannot fail");
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
        FindingDistribution::Mixed => "mixed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{Finding, Location};

    fn report() -> Report {
        Report {
            version: 3,
            files: 2,
            source_lines: 4,
            analyzed_lines: 4,
            duplicate_groups: 1,
            duplicate_lines: 2,
            duplication_percent: 50.0,
            findings: vec![Finding {
                code: "DUP001".to_owned(),
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
                        source: Some("alpha()\nbeta()".to_owned()),
                    },
                    Location {
                        path: "b.py".to_owned(),
                        start_line: 3,
                        end_line: 4,
                        source: Some("alpha()\nbeta()".to_owned()),
                    },
                ],
            }],
        }
    }

    #[test]
    fn plain_text_matches_v1_renderer() {
        let report = report();

        assert_eq!(render_text(&report, false), render_human(&report));
    }

    #[test]
    fn colored_text_uses_semantic_styles() {
        let rendered = render_text(&report(), true);

        assert!(rendered.contains("\x1b[1;33mDUP001\x1b[0m"));
        assert!(rendered.contains("\x1b[1;36ma.py\x1b[0m"));
        assert!(rendered.contains("\x1b[36m:1-2\x1b[0m"));
        assert!(rendered.contains("\x1b[2m       1 | \x1b[0malpha()"));
    }
}
