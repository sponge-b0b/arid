use std::path::PathBuf;

use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
    Markdown,
    Sarif,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ColorWhen {
    Auto,
    Always,
    Never,
}

/// Fast Python duplicate-code checker written in Rust.
#[derive(Debug, Parser)]
#[command(name = "arid", version, about)]
pub struct Cli {
    /// Files or directories to scan. Defaults to the current directory.
    #[arg(value_name = "PATH")]
    pub paths: Vec<PathBuf>,

    /// Minimum number of effective lines required for a duplicate.
    #[arg(long, value_name = "N")]
    pub min_lines: Option<u32>,

    /// Ignore comments when normalizing source.
    #[arg(long, conflicts_with = "no_ignore_comments")]
    pub ignore_comments: bool,

    /// Do not ignore comments when normalizing source.
    #[arg(long, conflicts_with = "ignore_comments")]
    pub no_ignore_comments: bool,

    /// Ignore docstrings when normalizing source.
    #[arg(long, conflicts_with = "no_ignore_docstrings")]
    pub ignore_docstrings: bool,

    /// Do not ignore docstrings when normalizing source.
    #[arg(long, conflicts_with = "ignore_docstrings")]
    pub no_ignore_docstrings: bool,

    /// Ignore imports when normalizing source.
    #[arg(long, conflicts_with = "no_ignore_imports")]
    pub ignore_imports: bool,

    /// Do not ignore imports when normalizing source.
    #[arg(long, conflicts_with = "ignore_imports")]
    pub no_ignore_imports: bool,

    /// Ignore function and method signatures when normalizing source.
    #[arg(long, conflicts_with = "no_ignore_signatures")]
    pub ignore_signatures: bool,

    /// Do not ignore function and method signatures when normalizing source.
    #[arg(long, conflicts_with = "ignore_signatures")]
    pub no_ignore_signatures: bool,

    /// Detect duplicates within the same file.
    #[arg(long, conflicts_with = "no_same_file")]
    pub same_file: bool,

    /// Do not detect duplicates within the same file.
    #[arg(long, conflicts_with = "same_file")]
    pub no_same_file: bool,

    /// Include hidden files and directories during discovery.
    #[arg(long, conflicts_with = "no_hidden")]
    pub hidden: bool,

    /// Do not include hidden files and directories during discovery.
    #[arg(long, conflicts_with = "hidden")]
    pub no_hidden: bool,

    /// Exclude paths matching PATTERN. May be repeated.
    #[arg(long, value_name = "PATTERN")]
    pub exclude: Vec<String>,

    /// Number of workers used for file preparation.
    #[arg(
        long,
        value_name = "N",
        default_value_t = 1,
        value_parser = parse_worker_count,
    )]
    pub workers: usize,

    /// Select the report representation.
    #[arg(long, value_enum, value_name = "FORMAT", conflicts_with = "json")]
    pub format: Option<OutputFormat>,

    /// Control color in text output.
    #[arg(long, value_enum, value_name = "WHEN")]
    pub color: Option<ColorWhen>,

    /// Emit JSON instead of text output. Equivalent to --format json.
    #[arg(long, conflicts_with = "format")]
    pub json: bool,

    /// Include source text in reported duplicate locations.
    #[arg(long)]
    pub show_source: bool,
}

impl Cli {
    #[must_use]
    pub fn output_format(&self) -> OutputFormat {
        if self.json {
            OutputFormat::Json
        } else {
            self.format.unwrap_or(OutputFormat::Text)
        }
    }
}

fn parse_worker_count(value: &str) -> Result<usize, String> {
    let workers = value
        .parse::<usize>()
        .map_err(|_| "worker count must be a positive integer".to_owned())?;

    if workers == 0 {
        return Err("worker count must be at least 1".to_owned());
    }

    Ok(workers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_no_paths() {
        let cli = Cli::try_parse_from(["arid"]).unwrap();

        assert!(cli.paths.is_empty());
        assert_eq!(cli.min_lines, None);

        assert!(!cli.ignore_comments);
        assert!(!cli.no_ignore_comments);
        assert!(!cli.ignore_docstrings);
        assert!(!cli.no_ignore_docstrings);
        assert!(!cli.ignore_imports);
        assert!(!cli.no_ignore_imports);
        assert!(!cli.ignore_signatures);
        assert!(!cli.no_ignore_signatures);
        assert!(!cli.same_file);
        assert!(!cli.no_same_file);
        assert!(!cli.hidden);
        assert!(!cli.no_hidden);

        assert!(cli.exclude.is_empty());
        assert_eq!(cli.workers, 1);
        assert_eq!(cli.format, None);
        assert_eq!(cli.output_format(), OutputFormat::Text);
        assert_eq!(cli.color, None);
        assert!(!cli.json);
        assert!(!cli.show_source);
    }

    #[test]
    fn accepts_paths_and_output_options() {
        let cli = Cli::try_parse_from([
            "arid",
            "--min-lines",
            "6",
            "--workers",
            "4",
            "--json",
            "--show-source",
            "src",
            "tests/example.py",
        ])
        .unwrap();

        assert_eq!(
            cli.paths,
            vec![PathBuf::from("src"), PathBuf::from("tests/example.py")]
        );

        assert_eq!(cli.min_lines, Some(6));
        assert_eq!(cli.workers, 4);
        assert_eq!(cli.output_format(), OutputFormat::Json);
        assert!(cli.json);
        assert!(cli.show_source);
    }

    #[test]
    fn accepts_output_formats() {
        for (value, expected) in [
            ("text", OutputFormat::Text),
            ("json", OutputFormat::Json),
            ("markdown", OutputFormat::Markdown),
            ("sarif", OutputFormat::Sarif),
        ] {
            let cli = Cli::try_parse_from(["arid", "--format", value]).unwrap();

            assert_eq!(cli.format, Some(expected));
            assert_eq!(cli.output_format(), expected);
        }
    }

    #[test]
    fn accepts_color_choices() {
        for (value, expected) in [
            ("auto", ColorWhen::Auto),
            ("always", ColorWhen::Always),
            ("never", ColorWhen::Never),
        ] {
            let cli = Cli::try_parse_from(["arid", "--color", value]).unwrap();

            assert_eq!(cli.color, Some(expected));
        }
    }

    #[test]
    fn rejects_json_with_explicit_format() {
        let result = Cli::try_parse_from(["arid", "--json", "--format", "json"]);

        assert!(result.is_err());
    }

    #[test]
    fn accepts_boolean_overrides_and_excludes() {
        let cli = Cli::try_parse_from([
            "arid",
            "--ignore-comments",
            "--no-ignore-docstrings",
            "--ignore-imports",
            "--no-ignore-signatures",
            "--no-same-file",
            "--hidden",
            "--exclude",
            "generated/**",
            "--exclude",
            "vendor/**",
        ])
        .unwrap();

        assert!(cli.ignore_comments);
        assert!(!cli.no_ignore_comments);
        assert!(!cli.ignore_docstrings);
        assert!(cli.no_ignore_docstrings);
        assert!(cli.ignore_imports);
        assert!(!cli.no_ignore_imports);
        assert!(!cli.ignore_signatures);
        assert!(cli.no_ignore_signatures);
        assert!(!cli.same_file);
        assert!(cli.no_same_file);
        assert!(cli.hidden);
        assert!(!cli.no_hidden);

        assert_eq!(cli.exclude, vec!["generated/**", "vendor/**"]);
    }

    #[test]
    fn rejects_conflicting_boolean_overrides() {
        let result = Cli::try_parse_from(["arid", "--ignore-comments", "--no-ignore-comments"]);

        assert!(result.is_err());
    }

    #[test]
    fn rejects_conflicting_same_file_overrides() {
        let result = Cli::try_parse_from(["arid", "--same-file", "--no-same-file"]);

        assert!(result.is_err());
    }

    #[test]
    fn rejects_conflicting_hidden_overrides() {
        let result = Cli::try_parse_from(["arid", "--hidden", "--no-hidden"]);

        assert!(result.is_err());
    }

    #[test]
    fn rejects_zero_workers() {
        let result = Cli::try_parse_from(["arid", "--workers", "0"]);

        assert!(result.is_err());
    }
}
