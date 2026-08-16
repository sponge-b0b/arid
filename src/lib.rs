use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;

use rayon::prelude::*;

pub mod cli;
pub mod config;
pub mod corpus;
pub mod detect;
pub mod files;
pub mod metrics;
pub mod model;
pub mod normalize;
pub mod outcome;
pub mod report;
pub mod suffix;
mod python;
mod text;

use cli::{Cli, ColorWhen, OutputFormat};
use config::{SettingsOverrides, load_settings};
use corpus::build_corpus;
use detect::detect_duplicates;
use files::discover_python_files;
use model::{NormalizationOptions, PreparedFile};
use normalize::prepare_file;
use outcome::ExitStatus;
use report::{ReportOptions, build_report, render_json};
use text::render_text;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ColorEnvironment {
    pub no_color: bool,
    pub clicolor_force: bool,
    pub clicolor_disabled: bool,
}

impl ColorEnvironment {
    #[must_use]
    pub fn from_process() -> Self {
        Self {
            no_color: std::env::var_os("NO_COLOR")
                .is_some_and(|value| !value.as_os_str().is_empty()),
            clicolor_force: std::env::var_os("CLICOLOR_FORCE")
                .is_some_and(|value| value.as_os_str() != OsStr::new("0")),
            clicolor_disabled: std::env::var_os("CLICOLOR")
                .is_some_and(|value| value.as_os_str() == OsStr::new("0")),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RunContext {
    pub text_color_capable: bool,
    pub color_environment: ColorEnvironment,
}

impl RunContext {
    #[must_use]
    pub const fn non_terminal() -> Self {
        Self {
            text_color_capable: false,
            color_environment: ColorEnvironment {
                no_color: false,
                clicolor_force: false,
                clicolor_disabled: false,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunResult {
    pub output: String,
    pub exit_status: ExitStatus,
}

/// Runs one complete Arid scan with deterministic non-terminal defaults.
pub fn run(cli: &Cli) -> Result<RunResult, String> {
    run_with_context(cli, RunContext::non_terminal())
}

/// Runs one complete Arid scan with explicit output-environment context.
///
/// Application pipeline:
///
/// discover → read → normalize → corpus → detect → report
pub fn run_with_context(cli: &Cli, context: RunContext) -> Result<RunResult, String> {
    let output_format = cli.output_format();

    validate_output_options(cli, output_format)?;

    let paths = scan_paths(cli);

    let loaded = load_settings(&paths[0], settings_overrides(cli))
        .map_err(|error| format!("failed to load configuration: {error}"))?;

    let discovered = discover_python_files(&paths, &loaded.settings, &loaded.project_root)
        .map_err(|error| format!("failed to discover Python files: {error}"))?;

    let prepared = prepare_files(
        discovered,
        loaded.settings.normalization_options(),
        cli.workers,
    )?;

    let corpus = build_corpus(prepared)
        .map_err(|error| format!("failed to build source corpus: {error}"))?;

    let groups = detect_duplicates(&corpus, loaded.settings.detection_options())
        .map_err(|error| format!("failed to detect duplicates: {error}"))?;

    let report = build_report(
        &corpus,
        &groups,
        &ReportOptions {
            show_source: cli.show_source,
            path_root: Some(loaded.project_root),
        },
    )
    .map_err(|error| format!("failed to build report: {error}"))?;

    let output = match output_format {
        OutputFormat::Text => render_text(&report, resolve_text_color(cli.color, context)),
        OutputFormat::Json => {
            render_json(&report).map_err(|error| format!("failed to render JSON report: {error}"))?
        }
        OutputFormat::Markdown | OutputFormat::Sarif => {
            unreachable!("unsupported formats are rejected before scanning")
        }
    };

    Ok(RunResult {
        exit_status: report.exit_status(),
        output,
    })
}

fn validate_output_options(cli: &Cli, output_format: OutputFormat) -> Result<(), String> {
    if cli.color.is_some() && output_format != OutputFormat::Text {
        return Err("--color is only valid with text output".to_owned());
    }

    match output_format {
        OutputFormat::Text | OutputFormat::Json => Ok(()),
        OutputFormat::Markdown => Err("Markdown output is not implemented yet".to_owned()),
        OutputFormat::Sarif => Err("SARIF output is not implemented yet".to_owned()),
    }
}

fn resolve_text_color(explicit: Option<ColorWhen>, context: RunContext) -> bool {
    if let Some(choice) = explicit {
        return match choice {
            ColorWhen::Auto => context.text_color_capable,
            ColorWhen::Always => true,
            ColorWhen::Never => false,
        };
    }

    let environment = context.color_environment;

    if environment.no_color {
        false
    } else if environment.clicolor_force {
        true
    } else if environment.clicolor_disabled {
        false
    } else {
        context.text_color_capable
    }
}

fn prepare_files(
    paths: Vec<PathBuf>,
    options: NormalizationOptions,
    workers: usize,
) -> Result<Vec<PreparedFile>, String> {
    if workers == 0 {
        return Err("worker count must be at least 1".to_owned());
    }

    if workers == 1 || paths.len() < 2 {
        return paths
            .into_iter()
            .map(|path| prepare_path(path, options))
            .collect();
    }

    let worker_count = workers.min(paths.len());

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(worker_count)
        .build()
        .map_err(|error| format!("failed to create worker pool: {error}"))?;

    let results = pool.install(|| {
        paths
            .into_par_iter()
            .map(|path| prepare_path(path, options))
            .collect::<Vec<_>>()
    });

    results.into_iter().collect()
}

fn prepare_path(path: PathBuf, options: NormalizationOptions) -> Result<PreparedFile, String> {
    let source = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

    prepare_file(path, source, options)
        .map_err(|error| format!("failed to prepare Python source: {error}"))
}

fn scan_paths(cli: &Cli) -> Vec<PathBuf> {
    if cli.paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        cli.paths.clone()
    }
}

fn settings_overrides(cli: &Cli) -> SettingsOverrides {
    SettingsOverrides {
        min_lines: cli.min_lines,
        ignore_comments: boolean_override(cli.ignore_comments, cli.no_ignore_comments),
        ignore_docstrings: boolean_override(cli.ignore_docstrings, cli.no_ignore_docstrings),
        ignore_imports: boolean_override(cli.ignore_imports, cli.no_ignore_imports),
        ignore_signatures: boolean_override(cli.ignore_signatures, cli.no_ignore_signatures),
        same_file: boolean_override(cli.same_file, cli.no_same_file),
        hidden: boolean_override(cli.hidden, cli.no_hidden),
        exclude: (!cli.exclude.is_empty()).then(|| cli.exclude.clone()),
    }
}

fn boolean_override(enabled: bool, disabled: bool) -> Option<bool> {
    match (enabled, disabled) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        (false, false) => None,
        (true, true) => {
            unreachable!("conflicting CLI flags must be rejected by clap")
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);

            let path =
                std::env::temp_dir().join(format!("arid-run-test-{}-{id}", std::process::id()));

            fs::create_dir_all(&path).unwrap();

            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn write(&self, relative: &str, contents: &str) -> PathBuf {
            let path = self.path.join(relative);

            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }

            fs::write(&path, contents).unwrap();

            path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn write_test_config(temp: &TempDir) {
        temp.write(
            "pyproject.toml",
            r#"
[tool.arid]
min-lines = 2
"#,
        );
    }

    fn test_cli(paths: Vec<PathBuf>) -> Cli {
        Cli {
            paths,
            min_lines: None,
            ignore_comments: false,
            no_ignore_comments: false,
            ignore_docstrings: false,
            no_ignore_docstrings: false,
            ignore_imports: false,
            no_ignore_imports: false,
            ignore_signatures: false,
            no_ignore_signatures: false,
            same_file: false,
            no_same_file: false,
            hidden: false,
            no_hidden: false,
            exclude: Vec::new(),
            workers: 1,
            format: None,
            color: None,
            json: false,
            show_source: false,
        }
    }

    fn duplicate_fixture() -> (TempDir, Cli) {
        let temp = TempDir::new();
        write_test_config(&temp);

        temp.write("a.py", "alpha = 1\nbeta = 2\n");
        temp.write("b.py", "alpha = 1\nbeta = 2\n");

        let cli = test_cli(vec![temp.path().to_path_buf()]);

        (temp, cli)
    }

    #[test]
    fn end_to_end_scan_reports_real_duplicate() {
        let (_temp, cli) = duplicate_fixture();

        let result = run(&cli).unwrap();

        assert_eq!(result.exit_status, ExitStatus::Findings);

        assert!(result.output.contains("DUP001"));
        assert!(result.output.contains("a.py:1-2"));
        assert!(result.output.contains("b.py:1-2"));
        assert!(result.output.contains("Found 1 duplicate group."));
        assert!(!result.output.contains('\u{1b}'));
    }

    #[test]
    fn cli_min_lines_overrides_project_config() {
        let temp = TempDir::new();
        write_test_config(&temp);

        temp.write("a.py", "alpha = 1\nbeta = 2\n");
        temp.write("b.py", "alpha = 1\nbeta = 2\n");

        let mut cli = test_cli(vec![temp.path().to_path_buf()]);
        cli.min_lines = Some(3);

        let result = run(&cli).unwrap();

        assert_eq!(result.exit_status, ExitStatus::Success);
        assert_eq!(
            result.output,
            concat!("No duplicate code found.\n", "0 duplicate lines (0.00%).\n",)
        );
    }

    #[test]
    fn end_to_end_scan_succeeds_without_duplicates() {
        let temp = TempDir::new();
        write_test_config(&temp);

        temp.write("a.py", "alpha = 1\nbeta = 2\n");
        temp.write("b.py", "gamma = 3\ndelta = 4\n");

        let result = run(&test_cli(vec![temp.path().to_path_buf()])).unwrap();

        assert_eq!(result.exit_status, ExitStatus::Success);
        assert_eq!(
            result.output,
            concat!("No duplicate code found.\n", "0 duplicate lines (0.00%).\n",)
        );
    }

    #[test]
    fn end_to_end_scan_can_emit_json() {
        let (_temp, mut cli) = duplicate_fixture();
        cli.json = true;

        let result = run(&cli).unwrap();

        assert_eq!(result.exit_status, ExitStatus::Findings);

        let value: serde_json::Value = serde_json::from_str(&result.output).unwrap();

        assert_eq!(value["version"], 3);
        assert_eq!(value["duplicate_groups"], 1);
        assert_eq!(value["findings"][0]["code"], "DUP001");
    }

    #[test]
    fn format_json_matches_json_flag() {
        let (_temp, cli) = duplicate_fixture();

        let mut via_flag = test_cli(cli.paths.clone());
        via_flag.json = true;

        let mut via_format = test_cli(cli.paths);
        via_format.format = Some(OutputFormat::Json);

        assert_eq!(run(&via_format).unwrap(), run(&via_flag).unwrap());
    }

    #[test]
    fn unsupported_formats_return_clear_errors() {
        let temp = TempDir::new();
        write_test_config(&temp);
        temp.write("example.py", "alpha = 1\nbeta = 2\n");

        for (format, expected) in [
            (OutputFormat::Markdown, "Markdown output is not implemented yet"),
            (OutputFormat::Sarif, "SARIF output is not implemented yet"),
        ] {
            let mut cli = test_cli(vec![temp.path().to_path_buf()]);
            cli.format = Some(format);

            assert_eq!(run(&cli).unwrap_err(), expected);
        }
    }

    #[test]
    fn explicit_color_overrides_environment() {
        let (_temp, mut cli) = duplicate_fixture();
        let hostile_environment = ColorEnvironment {
            no_color: true,
            clicolor_force: true,
            clicolor_disabled: true,
        };

        cli.color = Some(ColorWhen::Always);
        let result = run_with_context(
            &cli,
            RunContext {
                text_color_capable: false,
                color_environment: hostile_environment,
            },
        )
        .unwrap();
        assert!(result.output.contains('\u{1b}'));

        cli.color = Some(ColorWhen::Never);
        let result = run_with_context(
            &cli,
            RunContext {
                text_color_capable: true,
                color_environment: hostile_environment,
            },
        )
        .unwrap();
        assert!(!result.output.contains('\u{1b}'));

        cli.color = Some(ColorWhen::Auto);
        let result = run_with_context(
            &cli,
            RunContext {
                text_color_capable: false,
                color_environment: hostile_environment,
            },
        )
        .unwrap();
        assert!(!result.output.contains('\u{1b}'));
    }

    #[test]
    fn color_environment_precedence_is_deterministic() {
        let terminal = |environment| RunContext {
            text_color_capable: true,
            color_environment: environment,
        };
        let redirected = |environment| RunContext {
            text_color_capable: false,
            color_environment: environment,
        };

        assert!(!resolve_text_color(
            None,
            terminal(ColorEnvironment {
                no_color: true,
                clicolor_force: true,
                clicolor_disabled: false,
            }),
        ));

        assert!(resolve_text_color(
            None,
            redirected(ColorEnvironment {
                no_color: false,
                clicolor_force: true,
                clicolor_disabled: true,
            }),
        ));

        assert!(!resolve_text_color(
            None,
            terminal(ColorEnvironment {
                no_color: false,
                clicolor_force: false,
                clicolor_disabled: true,
            }),
        ));

        assert!(resolve_text_color(None, terminal(ColorEnvironment::default())));
        assert!(!resolve_text_color(
            None,
            redirected(ColorEnvironment::default()),
        ));
    }

    #[test]
    fn json_never_contains_terminal_color() {
        let (_temp, mut cli) = duplicate_fixture();
        cli.json = true;

        let result = run_with_context(
            &cli,
            RunContext {
                text_color_capable: true,
                color_environment: ColorEnvironment {
                    no_color: false,
                    clicolor_force: true,
                    clicolor_disabled: false,
                },
            },
        )
        .unwrap();

        assert!(!result.output.contains('\u{1b}'));
        serde_json::from_str::<serde_json::Value>(&result.output).unwrap();
    }

    #[test]
    fn color_is_rejected_for_non_text_output() {
        let (_temp, mut cli) = duplicate_fixture();
        cli.json = true;
        cli.color = Some(ColorWhen::Always);

        assert_eq!(
            run(&cli).unwrap_err(),
            "--color is only valid with text output"
        );
    }

    #[test]
    fn parse_failure_aborts_entire_scan() {
        let temp = TempDir::new();
        write_test_config(&temp);

        temp.write("a.py", "alpha = 1\nbeta = 2\n");
        temp.write("b.py", "alpha = 1\nbeta = 2\n");
        temp.write("broken.py", "def broken(:\n");

        for workers in [1, 4] {
            let mut cli = test_cli(vec![temp.path().to_path_buf()]);
            cli.workers = workers;

            let error = run(&cli).unwrap_err();

            assert!(error.contains("broken.py"));
            assert!(error.contains("invalid Python syntax"));
        }
    }

    #[test]
    fn worker_counts_produce_identical_json() {
        let temp = TempDir::new();
        write_test_config(&temp);

        for index in 0..8 {
            let path = format!("file_{index}.py");
            temp.write(&path, "alpha = 1\nbeta = 2\n");
        }

        let mut serial = test_cli(vec![temp.path().to_path_buf()]);
        serial.json = true;

        let expected = run(&serial).unwrap();

        for workers in [2, 4, 8] {
            let mut parallel = test_cli(vec![temp.path().to_path_buf()]);
            parallel.json = true;
            parallel.workers = workers;

            let actual = run(&parallel).unwrap();

            assert_eq!(actual.exit_status, expected.exit_status);
            assert_eq!(actual.output, expected.output);
        }
    }

    #[test]
    fn run_rejects_zero_workers() {
        let temp = TempDir::new();
        write_test_config(&temp);
        temp.write("example.py", "alpha = 1\nbeta = 2\n");

        let mut cli = test_cli(vec![temp.path().to_path_buf()]);
        cli.workers = 0;

        let error = run(&cli).unwrap_err();

        assert_eq!(error, "worker count must be at least 1");
    }

    #[test]
    fn cli_boolean_flags_produce_settings_overrides() {
        let mut cli = test_cli(Vec::new());

        cli.ignore_comments = true;
        cli.no_ignore_docstrings = true;
        cli.ignore_imports = true;
        cli.no_ignore_signatures = true;
        cli.no_same_file = true;
        cli.hidden = true;

        let overrides = settings_overrides(&cli);

        assert_eq!(overrides.ignore_comments, Some(true));
        assert_eq!(overrides.ignore_docstrings, Some(false));
        assert_eq!(overrides.ignore_imports, Some(true));
        assert_eq!(overrides.ignore_signatures, Some(false));
        assert_eq!(overrides.same_file, Some(false));
        assert_eq!(overrides.hidden, Some(true));
    }

    #[test]
    fn absent_cli_flags_do_not_override_project_settings() {
        let cli = test_cli(Vec::new());

        assert_eq!(settings_overrides(&cli), SettingsOverrides::default());
    }

    #[test]
    fn cli_excludes_replace_project_excludes() {
        let mut cli = test_cli(Vec::new());
        cli.exclude = vec!["generated/**".to_owned(), "vendor/**".to_owned()];

        assert_eq!(
            settings_overrides(&cli).exclude,
            Some(vec!["generated/**".to_owned(), "vendor/**".to_owned(),])
        );
    }
}
