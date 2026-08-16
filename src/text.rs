use std::fmt::{Display, Write};

use clap::builder::styling::{Ansi256Color, AnsiColor, Style};

use crate::report::{FindingContext, FindingDistribution, FindingScope, Report, render_text_plain};

#[derive(Debug, Clone, Copy)]
struct TextStyles {
    diagnostic: Style,
    problem: Style,
    success: Style,
    heading: Style,
    classification: Style,
    distribution: Style,
    group: Style,
    path: Style,
    location: Style,
    source_gutter: Style,
}

impl TextStyles {
    const fn colored() -> Self {
        Self {
            diagnostic: AnsiColor::Yellow.on_default().bold(),
            problem: AnsiColor::Red.on_default().bold(),
            success: AnsiColor::Green.on_default().bold(),
            heading: Style::new().bold(),
            classification: AnsiColor::Magenta.on_default().bold(),
            distribution: AnsiColor::Blue.on_default().bold(),
            group: Ansi256Color(208).on_default().bold(),
            path: AnsiColor::Cyan.on_default().bold(),
            location: AnsiColor::Cyan.on_default(),
            source_gutter: Style::new().dimmed(),
        }
    }
}

#[must_use]
pub fn render_text(report: &Report, color: bool) -> String {
    if !color {
        return render_text_plain(report);
    }

    let styles = TextStyles::colored();
    let mut output = String::new();

    for finding in &report.findings {
        let unit = if finding.lines == 1 { "line" } else { "lines" };
        let file_unit = if finding.files == 1 { "file" } else { "files" };

        write_styled(&mut output, styles.diagnostic, &finding.code);
        output.push(' ');
        write_styled(
            &mut output,
            styles.problem,
            format_args!("{} duplicated {unit}", finding.lines),
        );
        output.push('\n');

        write_styled(&mut output, styles.heading, "Context:");
        output.push(' ');
        write_context(&mut output, styles, finding.context);
        output.push('\n');

        write_styled(&mut output, styles.heading, "Scope:");
        output.push(' ');
        write_scope(&mut output, styles, finding.scope);
        output.push('\n');

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
            styles.distribution,
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
        write_styled(&mut output, styles.success, "No duplicate code found.");
        output.push('\n');
    } else {
        let group_unit = if report.duplicate_groups == 1 {
            "group"
        } else {
            "groups"
        };

        output.push_str("Found ");
        write_styled(
            &mut output,
            styles.group,
            format_args!("{} duplicate {group_unit}", report.duplicate_groups),
        );
        output.push_str(".\n");
    }

    let line_unit = if report.duplicate_lines == 1 {
        "line"
    } else {
        "lines"
    };
    let line_style = if report.duplicate_lines == 0 {
        styles.success
    } else {
        styles.problem
    };

    write_styled(
        &mut output,
        line_style,
        format_args!("{} duplicate {line_unit}", report.duplicate_lines),
    );
    output.push(' ');
    write_styled(
        &mut output,
        styles.heading,
        format_args!("({:.2}%)", report.duplication_percent),
    );
    output.push_str(".\n");

    output
}

fn write_styled(output: &mut String, style: Style, value: impl Display) {
    write!(output, "{style}{value}{style:#}").expect("writing to String cannot fail");
}

fn write_context(output: &mut String, styles: TextStyles, context: FindingContext) {
    let value = context_name(context);

    if context == FindingContext::Mixed {
        write_styled(output, styles.classification, value);
    } else {
        output.push_str(value);
    }
}

fn write_scope(output: &mut String, styles: TextStyles, scope: FindingScope) {
    let value = scope_name(scope);

    if scope == FindingScope::Mixed {
        write_styled(output, styles.classification, value);
    } else {
        output.push_str(value);
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

        assert_eq!(render_text(&report, false), render_text_plain(&report));
    }

    #[test]
    fn colored_text_uses_semantic_styles() {
        let styles = TextStyles::colored();
        let rendered = render_text(&report(), true);

        assert!(rendered.contains(&format!(
            "{}DUP001{:#}",
            styles.diagnostic, styles.diagnostic,
        )));
        assert!(rendered.contains(&format!(
            "{}2 duplicated lines{:#}",
            styles.problem, styles.problem,
        )));
        assert!(rendered.contains(&format!(
            "{}(cross-file){:#}",
            styles.distribution, styles.distribution,
        )));
        assert!(rendered.contains(&format!(
            "{}1 duplicate group{:#}",
            styles.group, styles.group,
        )));
        assert!(rendered.contains(&format!(
            "{}(50.00%){:#}",
            styles.heading, styles.heading,
        )));
        assert!(rendered.contains(&format!("{}a.py{:#}", styles.path, styles.path)));
        assert!(rendered.contains(&format!(
            "{}:1-2{:#}",
            styles.location, styles.location,
        )));
        assert!(rendered.contains(&format!(
            "{}       1 | {:#}alpha()",
            styles.source_gutter, styles.source_gutter,
        )));
    }

    #[test]
    fn mixed_context_and_scope_use_classification_style() {
        let styles = TextStyles::colored();
        let mut report = report();
        report.findings[0].context = FindingContext::Mixed;
        report.findings[0].scope = FindingScope::Mixed;

        let rendered = render_text(&report, true);
        let mixed = format!(
            "{}mixed{:#}",
            styles.classification, styles.classification,
        );

        assert_eq!(rendered.matches(&mixed).count(), 2);
    }

    #[test]
    fn mixed_distribution_uses_distribution_style() {
        let styles = TextStyles::colored();
        let mut report = report();
        report.findings[0].distribution = FindingDistribution::Mixed;

        let rendered = render_text(&report, true);

        assert!(rendered.contains(&format!(
            "{}(mixed){:#}",
            styles.distribution, styles.distribution,
        )));
    }

    #[test]
    fn zero_duplication_uses_success_style() {
        let styles = TextStyles::colored();
        let report = Report {
            version: 3,
            files: 1,
            source_lines: 1,
            analyzed_lines: 1,
            duplicate_groups: 0,
            duplicate_lines: 0,
            duplication_percent: 0.0,
            findings: Vec::new(),
        };

        let rendered = render_text(&report, true);

        assert!(rendered.contains(&format!(
            "{}No duplicate code found.{:#}",
            styles.success, styles.success,
        )));
        assert!(rendered.contains(&format!(
            "{}0 duplicate lines{:#}",
            styles.success, styles.success,
        )));
        assert!(rendered.contains(&format!(
            "{}(0.00%){:#}",
            styles.heading, styles.heading,
        )));
    }
}
