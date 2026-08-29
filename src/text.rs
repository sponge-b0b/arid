use std::fmt::{Display, Write};

use clap::builder::styling::{Ansi256Color, AnsiColor, Style};

use crate::report::{FindingContext, FindingDistribution, FindingScope, Report};
use crate::summary::Summary;
#[cfg(test)]
use crate::summary::{SummaryOptions, build_summary};

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
    const fn plain() -> Self {
        Self {
            diagnostic: Style::new(),
            problem: Style::new(),
            success: Style::new(),
            heading: Style::new(),
            classification: Style::new(),
            distribution: Style::new(),
            group: Style::new(),
            path: Style::new(),
            location: Style::new(),
            source_gutter: Style::new(),
        }
    }

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

#[derive(Debug, Clone, Copy)]
enum Alignment {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy)]
struct Column {
    heading: &'static str,
    alignment: Alignment,
}

#[derive(Debug, Clone)]
struct Cell {
    text: String,
    style: Style,
}

impl Cell {
    fn plain(value: impl ToString) -> Self {
        Self {
            text: value.to_string(),
            style: Style::new(),
        }
    }

    fn styled(value: impl ToString, style: Style) -> Self {
        Self {
            text: value.to_string(),
            style,
        }
    }
}

#[derive(Debug, Clone)]
struct Row {
    cells: Vec<Cell>,
    separator_before: bool,
}

impl Row {
    fn new(cells: Vec<Cell>) -> Self {
        Self {
            cells,
            separator_before: false,
        }
    }

    fn separated(cells: Vec<Cell>) -> Self {
        Self {
            cells,
            separator_before: true,
        }
    }
}

#[cfg(test)]
fn render_text(report: &Report, color: bool) -> String {
    let summary = build_summary(report, SummaryOptions { ignore_files: true });
    render_text_with_summary(report, &summary, color)
}

#[must_use]
pub(crate) fn render_text_with_summary(report: &Report, summary: &Summary, color: bool) -> String {
    let styles = if color {
        TextStyles::colored()
    } else {
        TextStyles::plain()
    };
    let mut output = String::new();

    render_findings(&mut output, report, styles);
    render_summary_text(&mut output, summary, styles);

    output
}

fn render_findings(output: &mut String, report: &Report, styles: TextStyles) {
    for finding in &report.findings {
        let unit = if finding.lines == 1 { "line" } else { "lines" };
        let file_unit = if finding.files == 1 { "file" } else { "files" };

        write_styled(output, styles.diagnostic, &finding.code);
        output.push(' ');
        write_styled(
            output,
            styles.problem,
            format_args!("{} duplicated {unit}", finding.lines),
        );
        output.push('\n');

        write_styled(output, styles.heading, "Context:");
        output.push(' ');
        write_context(output, styles, finding.context);
        output.push('\n');

        write_styled(output, styles.heading, "Scope:");
        output.push(' ');
        write_scope(output, styles, finding.scope);
        output.push('\n');

        write_styled(output, styles.heading, "Occurrences:");
        output.push(' ');
        write_styled(output, styles.heading, finding.occurrences);
        output.push_str(" across ");
        write_styled(output, styles.heading, finding.files);
        output.push(' ');
        output.push_str(file_unit);
        output.push(' ');
        write_styled(
            output,
            styles.distribution,
            format_args!("({})", distribution_name(finding.distribution)),
        );
        output.push('\n');
        output.push('\n');

        for location in &finding.locations {
            output.push_str("  ");
            write_styled(output, styles.path, &location.path);
            write_styled(
                output,
                styles.location,
                format_args!(":{}-{}", location.start_line, location.end_line),
            );
            output.push('\n');

            if let Some(source) = &location.source {
                for (offset, line) in source.lines().enumerate() {
                    let line_number = location.start_line + offset as u64;
                    write_styled(
                        output,
                        styles.source_gutter,
                        format_args!("    {line_number:>4} | "),
                    );
                    writeln!(output, "{line}").expect("writing to String cannot fail");
                }
            }
        }

        output.push('\n');
    }
}

fn render_summary_text(output: &mut String, summary: &Summary, styles: TextStyles) {
    if !summary.complete {
        let unit = if summary.errors.len() == 1 {
            "error"
        } else {
            "errors"
        };
        write_styled(
            output,
            styles.problem,
            format_args!(
                "Scan incomplete: {} source processing {unit}.",
                summary.errors.len()
            ),
        );
        output.push('\n');
        output.push('\n');
    } else if summary.duplicate_groups == 0 {
        write_styled(output, styles.success, "No duplicate code found.");
        output.push('\n');
        output.push('\n');
    }

    write_styled(output, styles.heading, "Summary");
    output.push('\n');
    output.push('\n');

    let duplicate_style = if summary.duplicate_groups == 0 {
        styles.success
    } else {
        styles.problem
    };
    let group_style = if summary.duplicate_groups == 0 {
        styles.success
    } else {
        styles.group
    };
    let files_with_duplicates = if summary.files == 0 {
        format_count(summary.files_with_duplicates)
    } else {
        format!(
            "{} ({:.2}%)",
            format_count(summary.files_with_duplicates),
            percentage(summary.files_with_duplicates, summary.files),
        )
    };

    let summary_columns = [
        Column {
            heading: "",
            alignment: Alignment::Left,
        },
        Column {
            heading: "",
            alignment: Alignment::Right,
        },
    ];
    let summary_rows = vec![
        Row::new(vec![
            Cell::plain("Files analyzed"),
            Cell::plain(format_count(summary.files)),
        ]),
        Row::new(vec![
            Cell::plain("Files with duplicates"),
            Cell::styled(files_with_duplicates, group_style),
        ]),
        Row::new(vec![
            Cell::plain("Source lines"),
            Cell::plain(format_count(summary.source_lines)),
        ]),
        Row::new(vec![
            Cell::plain("Analyzed lines"),
            Cell::plain(format_count(summary.analyzed_lines)),
        ]),
        Row::new(vec![
            Cell::plain("Duplicate groups"),
            Cell::styled(format_count(summary.duplicate_groups), duplicate_style),
        ]),
        Row::new(vec![
            Cell::plain("Occurrences"),
            Cell::styled(format_count(summary.occurrences), group_style),
        ]),
        Row::new(vec![
            Cell::plain("Duplicate lines"),
            Cell::styled(format_count(summary.duplicate_lines), duplicate_style),
        ]),
        Row::new(vec![
            Cell::plain("Duplication"),
            Cell::styled(
                format!("{:.2}%", summary.duplication_percent),
                duplicate_style,
            ),
        ]),
    ];
    render_table(
        output,
        &summary_columns,
        &summary_rows,
        styles.heading,
        false,
    );

    if summary.duplicate_groups == 0 {
        return;
    }

    output.push('\n');
    write_styled(output, styles.heading, "Breakdown");
    output.push('\n');
    output.push('\n');

    let breakdown_columns = [
        Column {
            heading: "Dimension",
            alignment: Alignment::Left,
        },
        Column {
            heading: "Value",
            alignment: Alignment::Left,
        },
        Column {
            heading: "Groups",
            alignment: Alignment::Right,
        },
        Column {
            heading: "Percent",
            alignment: Alignment::Right,
        },
    ];
    let breakdown_rows = vec![
        breakdown_row(
            "Context",
            "executable",
            summary.context.executable,
            summary.duplicate_groups,
            styles.classification,
            styles,
            false,
        ),
        breakdown_row(
            "",
            "declarative",
            summary.context.declarative,
            summary.duplicate_groups,
            styles.classification,
            styles,
            false,
        ),
        breakdown_row(
            "",
            "mixed",
            summary.context.mixed,
            summary.duplicate_groups,
            styles.classification,
            styles,
            false,
        ),
        breakdown_row(
            "Scope",
            "function",
            summary.scope.function,
            summary.duplicate_groups,
            styles.classification,
            styles,
            true,
        ),
        breakdown_row(
            "",
            "module",
            summary.scope.module,
            summary.duplicate_groups,
            styles.classification,
            styles,
            false,
        ),
        breakdown_row(
            "",
            "class",
            summary.scope.class,
            summary.duplicate_groups,
            styles.classification,
            styles,
            false,
        ),
        breakdown_row(
            "",
            "mixed",
            summary.scope.mixed,
            summary.duplicate_groups,
            styles.classification,
            styles,
            false,
        ),
        breakdown_row(
            "Distribution",
            "cross-file",
            summary.distribution.cross_file,
            summary.duplicate_groups,
            styles.distribution,
            styles,
            true,
        ),
        breakdown_row(
            "",
            "same-file",
            summary.distribution.same_file,
            summary.duplicate_groups,
            styles.distribution,
            styles,
            false,
        ),
        breakdown_row(
            "",
            "hybrid",
            summary.distribution.hybrid,
            summary.duplicate_groups,
            styles.distribution,
            styles,
            false,
        ),
    ];
    render_table(
        output,
        &breakdown_columns,
        &breakdown_rows,
        styles.heading,
        true,
    );

    if summary.hotspots.is_empty() {
        return;
    }

    output.push('\n');
    write_styled(output, styles.heading, "Hotspots");
    output.push('\n');
    output.push('\n');

    let hotspot_columns = [
        Column {
            heading: "Path",
            alignment: Alignment::Left,
        },
        Column {
            heading: "Groups",
            alignment: Alignment::Right,
        },
        Column {
            heading: "Occurrences",
            alignment: Alignment::Right,
        },
    ];
    let hotspot_rows = summary
        .hotspots
        .iter()
        .map(|hotspot| {
            Row::new(vec![
                Cell::styled(&hotspot.path, styles.path),
                Cell::styled(format_count(hotspot.groups), styles.group),
                Cell::styled(format_count(hotspot.occurrences), styles.group),
            ])
        })
        .collect::<Vec<_>>();
    render_table(
        output,
        &hotspot_columns,
        &hotspot_rows,
        styles.heading,
        true,
    );
}

fn breakdown_row(
    dimension: &str,
    value: &str,
    groups: u64,
    total_groups: u64,
    value_style: Style,
    styles: TextStyles,
    separated: bool,
) -> Row {
    let cells = vec![
        Cell::plain(dimension),
        Cell::styled(value, value_style),
        Cell::styled(format_count(groups), styles.group),
        Cell::styled(
            format!("{:.2}%", percentage(groups, total_groups)),
            styles.heading,
        ),
    ];

    if separated {
        Row::separated(cells)
    } else {
        Row::new(cells)
    }
}

fn render_table(
    output: &mut String,
    columns: &[Column],
    rows: &[Row],
    heading_style: Style,
    show_heading: bool,
) {
    debug_assert!(
        rows.iter().all(|row| row.cells.len() == columns.len()),
        "table row width must match column count"
    );

    let mut widths = columns
        .iter()
        .map(|column| visible_width(column.heading))
        .collect::<Vec<_>>();

    for row in rows {
        for (index, cell) in row.cells.iter().enumerate() {
            widths[index] = widths[index].max(visible_width(&cell.text));
        }
    }

    write_border(output, '┌', '┬', '┐', &widths);
    if show_heading {
        let heading_cells = columns
            .iter()
            .map(|column| Cell::styled(column.heading, heading_style))
            .collect::<Vec<_>>();
        write_row(output, columns, &widths, &heading_cells);
        write_border(output, '├', '┼', '┤', &widths);
    }

    for row in rows {
        if row.separator_before {
            write_border(output, '├', '┼', '┤', &widths);
        }
        write_row(output, columns, &widths, &row.cells);
    }

    write_border(output, '└', '┴', '┘', &widths);
}

fn write_border(output: &mut String, left: char, middle: char, right: char, widths: &[usize]) {
    output.push(left);
    for (index, width) in widths.iter().enumerate() {
        for _ in 0..(*width + 2) {
            output.push('─');
        }
        output.push(if index + 1 == widths.len() {
            right
        } else {
            middle
        });
    }
    output.push('\n');
}

fn write_row(output: &mut String, columns: &[Column], widths: &[usize], cells: &[Cell]) {
    output.push('│');

    for ((column, width), cell) in columns.iter().zip(widths).zip(cells) {
        output.push(' ');
        let padding = width.saturating_sub(visible_width(&cell.text));
        if matches!(column.alignment, Alignment::Right) {
            for _ in 0..padding {
                output.push(' ');
            }
        }
        write_styled(output, cell.style, &cell.text);
        if matches!(column.alignment, Alignment::Left) {
            for _ in 0..padding {
                output.push(' ');
            }
        }
        output.push(' ');
        output.push('│');
    }

    output.push('\n');
}

fn write_styled(output: &mut String, style: Style, value: impl Display) {
    write!(output, "{style}{value}{style:#}").expect("writing to String cannot fail");
}

fn write_context(output: &mut String, styles: TextStyles, context: FindingContext) {
    write_styled(output, styles.classification, context_name(context));
}

fn write_scope(output: &mut String, styles: TextStyles, scope: FindingScope) {
    write_styled(output, styles.classification, scope_name(scope));
}

fn format_count(value: u64) -> String {
    let digits = value.to_string();
    let mut output = String::with_capacity(digits.len() + digits.len() / 3);

    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            output.push(',');
        }
        output.push(digit);
    }

    output
}

fn percentage(part: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        part as f64 / total as f64 * 100.0
    }
}

fn visible_width(value: &str) -> usize {
    value.chars().count()
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
    use crate::error::{ErrorKind, OperationalError};
    use crate::report::{AnalysisMetadata, Finding, Location};

    fn report() -> Report {
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
    fn plain_text_contains_rich_summary_breakdown_and_hotspots() {
        let rendered = render_text(&report(), false);

        assert!(rendered.contains("DUP001 2 duplicated lines"));
        assert!(rendered.contains("Summary\n\n┌"));
        assert!(rendered.contains("│ Files analyzed"));
        assert!(rendered.contains("│ Files with duplicates"));
        assert!(rendered.contains("2 (100.00%)"));
        assert!(rendered.contains("│ Duplicate groups"));
        assert!(rendered.contains("│ Occurrences"));
        assert!(rendered.contains("Breakdown\n\n┌"));
        assert!(rendered.contains("│ Context"));
        assert!(rendered.contains("executable"));
        assert!(rendered.contains("│ Scope"));
        assert!(rendered.contains("function"));
        assert!(rendered.contains("│ Distribution"));
        assert!(rendered.contains("cross-file"));
        assert!(rendered.contains("Hotspots\n\n┌"));
        assert!(rendered.contains("a.py"));
        assert!(rendered.contains("b.py"));
        assert!(!rendered.contains("Found 1 duplicate group."));
        assert!(!rendered.contains('\u{1b}'));
    }

    #[test]
    fn colored_text_uses_semantic_styles_for_findings_and_summary() {
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
            "{}executable{:#}",
            styles.classification, styles.classification,
        )));
        assert!(rendered.contains(&format!(
            "{}cross-file{:#}",
            styles.distribution, styles.distribution,
        )));
        assert!(rendered.contains(&format!("{}1{:#}", styles.group, styles.group)));
        assert!(rendered.contains(&format!("{}a.py{:#}", styles.path, styles.path)));
        assert!(rendered.contains(&format!("{}50.00%{:#}", styles.problem, styles.problem)));
    }

    #[test]
    fn colored_and_plain_output_have_identical_visible_text() {
        let plain = render_text(&report(), false);
        let colored = render_text(&report(), true);

        assert_eq!(strip_ansi(&colored), plain);
    }

    #[test]
    fn mixed_context_and_scope_use_classification_style() {
        let styles = TextStyles::colored();
        let mut report = report();
        report.findings[0].context = FindingContext::Mixed;
        report.findings[0].scope = FindingScope::Mixed;

        let rendered = render_text(&report, true);
        let mixed = format!("{}mixed{:#}", styles.classification, styles.classification,);

        assert!(rendered.matches(&mixed).count() >= 2);
    }

    #[test]
    fn hybrid_distribution_uses_distribution_style() {
        let styles = TextStyles::colored();
        let mut report = report();
        report.findings[0].distribution = FindingDistribution::Hybrid;

        let rendered = render_text(&report, true);

        assert!(rendered.contains(&format!(
            "{}(hybrid){:#}",
            styles.distribution, styles.distribution,
        )));
        assert!(rendered.contains(&format!(
            "{}hybrid{:#}",
            styles.distribution, styles.distribution,
        )));
    }

    #[test]
    fn zero_duplication_is_concise_and_uses_success_style() {
        let styles = TextStyles::colored();
        let report = Report {
            schema_version: 4,
            tool_version: env!("CARGO_PKG_VERSION"),
            complete: true,
            analysis: AnalysisMetadata::default(),
            errors: Vec::new(),
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
        assert!(rendered.contains(&format!("{}0{:#}", styles.success, styles.success)));
        assert!(rendered.contains(&format!(
            "{}0.00%{:#}",
            styles.success, styles.success,
        )));
        assert!(!rendered.contains("Breakdown"));
        assert!(!rendered.contains("Hotspots"));
    }

    #[test]
    fn incomplete_summary_is_explicit() {
        let mut report = report();
        report.complete = false;
        report.errors = vec![OperationalError::new(ErrorKind::Parse, "broken source")];

        let rendered = render_text(&report, false);

        assert!(rendered.contains("Scan incomplete: 1 source processing error."));
        assert!(rendered.contains("Summary"));
    }

    #[test]
    fn unicode_hotspot_paths_remain_visible() {
        let mut report = report();
        report.findings[0].locations[0].path = "src/café.py".to_owned();

        let rendered = render_text(&report, false);

        assert!(rendered.contains("src/café.py"));
    }

    #[test]
    fn formats_counts_with_thousands_separators() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(1_000), "1,000");
        assert_eq!(format_count(12_345_678), "12,345,678");
    }

    fn strip_ansi(value: &str) -> String {
        let mut output = String::new();
        let mut chars = value.chars().peekable();

        while let Some(character) = chars.next() {
            if character == '\u{1b}' && chars.peek() == Some(&'[') {
                chars.next();
                for code in chars.by_ref() {
                    if code == 'm' {
                        break;
                    }
                }
            } else {
                output.push(character);
            }
        }

        output
    }
}
