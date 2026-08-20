use std::path::PathBuf;

use clap::{Parser, ValueEnum};

const MAX_AUTO_WORKERS: usize = 4;

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

    /// Use this exact pyproject.toml instead of ancestor configuration discovery.
    #[arg(long, value_name = "PATH", conflicts_with = "no_config")]
    pub config: Option<PathBuf>,

    /// Disable project configuration discovery and use built-in defaults plus CLI overrides.
    #[arg(long, conflicts_with = "config")]
    pub no_config: bool,

    /// Set the project root used for configuration lookup and project-relative identity.
    #[arg(long, value_name = "PATH")]
    pub project_root: Option<PathBuf>,

    /// Show the resolved project configuration and exit.
    #[arg(
        long,
        conflicts_with_all = ["list_files", "baseline_status", "prune_baseline", "write_baseline"]
    )]
    pub show_config: bool,

    /// List Python files selected by discovery and exit.
    #[arg(
        long,
        conflicts_with_all = ["show_config", "baseline_status", "prune_baseline", "write_baseline"]
    )]
    pub list_files: bool,

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

    /// Number of workers used for file preparation, or "auto" for automatic selection capped at 4.
    #[arg(
        long,
        value_name = "N|auto",
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

    /// Accept duplicate debt recorded in the baseline file.
    #[arg(long, value_name = "PATH", conflicts_with = "write_baseline")]
    pub baseline: Option<PathBuf>,

    /// Report accepted, active, and stale debt against a baseline.
    #[arg(
        long,
        value_name = "PATH",
        conflicts_with_all = ["baseline", "write_baseline", "prune_baseline"]
    )]
    pub baseline_status: Option<PathBuf>,

    /// Prune stale accepted debt from a baseline.
    #[arg(
        long,
        value_name = "PATH",
        conflicts_with_all = ["baseline", "baseline_status", "write_baseline"]
    )]
    pub prune_baseline: Option<PathBuf>,

    /// Write the current duplicate debt as a baseline and exit successfully.
    #[arg(
        long,
        value_name = "PATH",
        conflicts_with_all = ["baseline", "baseline_status", "prune_baseline", "format", "json", "color", "show_source"]
    )]
    pub write_baseline: Option<PathBuf>,
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
    if value == "auto" {
        return Ok(auto_worker_count(
            std::thread::available_parallelism().ok().map(usize::from),
        ));
    }

    let workers = value
        .parse::<usize>()
        .map_err(|_| "worker count must be a positive integer or 'auto'".to_owned())?;

    if workers == 0 {
        return Err("worker count must be at least 1".to_owned());
    }

    Ok(workers)
}

fn auto_worker_count(available: Option<usize>) -> usize {
    available.unwrap_or(1).clamp(1, MAX_AUTO_WORKERS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_no_paths() {
        let cli = Cli::try_parse_from(["arid"]).unwrap();

        assert!(cli.paths.is_empty());
        assert_eq!(cli.config, None);
        assert!(!cli.no_config);
        assert_eq!(cli.project_root, None);
        assert!(!cli.show_config);
        assert!(!cli.list_files);
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
        assert_eq!(cli.baseline, None);
        assert_eq!(cli.baseline_status, None);
        assert_eq!(cli.prune_baseline, None);
        assert_eq!(cli.write_baseline, None);
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
    fn accepts_project_selection_options() {
        let cli = Cli::try_parse_from([
            "arid",
            "--config",
            "workspace/pyproject.toml",
            "--project-root",
            "workspace",
        ])
        .unwrap();
        assert_eq!(cli.config, Some(PathBuf::from("workspace/pyproject.toml")));
        assert!(!cli.no_config);
        assert_eq!(cli.project_root, Some(PathBuf::from("workspace")));

        let cli =
            Cli::try_parse_from(["arid", "--no-config", "--project-root", "workspace"]).unwrap();
        assert_eq!(cli.config, None);
        assert!(cli.no_config);
        assert_eq!(cli.project_root, Some(PathBuf::from("workspace")));
    }

    #[test]
    fn accepts_introspection_modes() {
        let cli = Cli::try_parse_from(["arid", "--show-config", "--json"]).unwrap();
        assert!(cli.show_config);
        assert!(!cli.list_files);
        assert_eq!(cli.output_format(), OutputFormat::Json);

        let cli = Cli::try_parse_from(["arid", "--list-files"]).unwrap();
        assert!(!cli.show_config);
        assert!(cli.list_files);
    }

    #[test]
    fn rejects_config_with_no_config() {
        assert!(
            Cli::try_parse_from(["arid", "--config", "pyproject.toml", "--no-config",]).is_err()
        );
    }

    #[test]
    fn rejects_conflicting_introspection_modes() {
        assert!(Cli::try_parse_from(["arid", "--show-config", "--list-files"]).is_err());
        assert!(
            Cli::try_parse_from(["arid", "--show-config", "--baseline-status", "debt.json"])
                .is_err()
        );
        assert!(
            Cli::try_parse_from(["arid", "--list-files", "--prune-baseline", "debt.json"]).is_err()
        );
    }

    #[test]
    fn accepts_auto_workers() {
        let cli = Cli::try_parse_from(["arid", "--workers", "auto"]).unwrap();
        assert!((1..=MAX_AUTO_WORKERS).contains(&cli.workers));
    }

    #[test]
    fn help_documents_auto_workers() {
        use clap::CommandFactory;
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("--workers <N|auto>"));
        assert!(help.contains("automatic selection capped at 4"));
    }

    #[test]
    fn automatic_worker_selection_is_bounded_and_has_serial_fallback() {
        assert_eq!(auto_worker_count(None), 1);
        assert_eq!(auto_worker_count(Some(1)), 1);
        assert_eq!(auto_worker_count(Some(2)), 2);
        assert_eq!(auto_worker_count(Some(MAX_AUTO_WORKERS)), MAX_AUTO_WORKERS);
        assert_eq!(
            auto_worker_count(Some(MAX_AUTO_WORKERS + 8)),
            MAX_AUTO_WORKERS
        );
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
    fn accepts_baseline_options() {
        let cli = Cli::try_parse_from(["arid", "--baseline", "debt.json"]).unwrap();
        assert_eq!(cli.baseline, Some(PathBuf::from("debt.json")));

        let cli = Cli::try_parse_from(["arid", "--baseline-status", "debt.json"]).unwrap();
        assert_eq!(cli.baseline_status, Some(PathBuf::from("debt.json")));

        let cli = Cli::try_parse_from(["arid", "--prune-baseline", "debt.json"]).unwrap();
        assert_eq!(cli.prune_baseline, Some(PathBuf::from("debt.json")));

        let cli = Cli::try_parse_from(["arid", "--write-baseline", "debt.json"]).unwrap();
        assert_eq!(cli.write_baseline, Some(PathBuf::from("debt.json")));
    }

    #[test]
    fn rejects_json_with_explicit_format() {
        assert!(Cli::try_parse_from(["arid", "--json", "--format", "json"]).is_err());
    }

    #[test]
    fn rejects_conflicting_baseline_modes() {
        for args in [
            [
                "arid",
                "--baseline",
                "old.json",
                "--write-baseline",
                "new.json",
            ],
            [
                "arid",
                "--baseline",
                "old.json",
                "--baseline-status",
                "new.json",
            ],
            [
                "arid",
                "--baseline",
                "old.json",
                "--prune-baseline",
                "new.json",
            ],
            [
                "arid",
                "--baseline-status",
                "old.json",
                "--write-baseline",
                "new.json",
            ],
            [
                "arid",
                "--baseline-status",
                "old.json",
                "--prune-baseline",
                "new.json",
            ],
            [
                "arid",
                "--prune-baseline",
                "old.json",
                "--write-baseline",
                "new.json",
            ],
        ] {
            assert!(Cli::try_parse_from(args).is_err());
        }
    }

    #[test]
    fn write_baseline_rejects_presentation_options() {
        for option in ["--json", "--show-source"] {
            assert!(
                Cli::try_parse_from(["arid", "--write-baseline", "debt.json", option]).is_err()
            );
        }
        assert!(
            Cli::try_parse_from(["arid", "--write-baseline", "debt.json", "--format", "text"])
                .is_err()
        );
        assert!(
            Cli::try_parse_from(["arid", "--write-baseline", "debt.json", "--color", "never"])
                .is_err()
        );
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
        assert!(
            Cli::try_parse_from(["arid", "--ignore-comments", "--no-ignore-comments"]).is_err()
        );
    }

    #[test]
    fn rejects_conflicting_same_file_overrides() {
        assert!(Cli::try_parse_from(["arid", "--same-file", "--no-same-file"]).is_err());
    }

    #[test]
    fn rejects_conflicting_hidden_overrides() {
        assert!(Cli::try_parse_from(["arid", "--hidden", "--no-hidden"]).is_err());
    }

    #[test]
    fn rejects_zero_workers() {
        assert!(Cli::try_parse_from(["arid", "--workers", "0"]).is_err());
    }

    #[test]
    fn rejects_invalid_workers() {
        let error = Cli::try_parse_from(["arid", "--workers", "many"]).unwrap_err();
        assert!(error.to_string().contains("positive integer or 'auto'"));
    }
}
