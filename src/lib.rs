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

use cli::Cli;
use config::{SettingsOverrides, load_settings};
use corpus::build_corpus;
use detect::detect_duplicates;
use files::discover_python_files;
use model::{NormalizationOptions, PreparedFile};
use normalize::prepare_file;
use outcome::ExitStatus;
use report::{ReportOptions, build_report, render_human, render_json};

#[derive(Debug, Clone, PartialEq)]
pub struct RunResult {
    pub output: String,
    pub exit_status: ExitStatus,
}

/// Runs one complete Arid scan.
///
/// Application pipeline:
///
/// discover → read → normalize → corpus → detect → report
pub fn run(cli: &Cli) -> Result<RunResult, String> {
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

    let output = if cli.json {
        render_json(&report).map_err(|error| format!("failed to render JSON report: {error}"))?
    } else {
        render_human(&report)
    };

    Ok(RunResult {
        exit_status: report.exit_status(),
        output,
    })
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
            json: false,
            show_source: false,
        }
    }

    #[test]
    fn end_to_end_scan_reports_real_duplicate() {
        let temp = TempDir::new();
        write_test_config(&temp);

        temp.write("a.py", "alpha = 1\nbeta = 2\n");

        temp.write("b.py", "alpha = 1\nbeta = 2\n");

        let result = run(&test_cli(vec![temp.path().to_path_buf()])).unwrap();

        assert_eq!(result.exit_status, ExitStatus::Findings);

        assert!(result.output.contains("DUP001"));
        assert!(result.output.contains("a.py:1-2"));
        assert!(result.output.contains("b.py:1-2"));
        assert!(result.output.contains("Found 1 duplicate group."));
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
        let temp = TempDir::new();
        write_test_config(&temp);

        temp.write("a.py", "alpha = 1\nbeta = 2\n");

        temp.write("b.py", "alpha = 1\nbeta = 2\n");

        let mut cli = test_cli(vec![temp.path().to_path_buf()]);

        cli.json = true;

        let result = run(&cli).unwrap();

        assert_eq!(result.exit_status, ExitStatus::Findings);

        let value: serde_json::Value = serde_json::from_str(&result.output).unwrap();

        assert_eq!(value["version"], 3);
        assert_eq!(value["duplicate_groups"], 1);

        assert_eq!(value["findings"][0]["code"], "DUP001");
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
