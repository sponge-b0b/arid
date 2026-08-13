use std::fs;
use std::path::PathBuf;

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
/// This is the application-level pipeline:
///
/// discover → read → normalize → corpus → detect → report
pub fn run(cli: &Cli) -> Result<RunResult, String> {
    let paths = scan_paths(cli);

    let loaded = load_settings(
        &paths[0],
        SettingsOverrides {
            min_lines: cli.min_lines,
            ..SettingsOverrides::default()
        },
    )
    .map_err(|error| format!("failed to load configuration: {error}"))?;

    let discovered = discover_python_files(&paths, &loaded.settings, &loaded.project_root)
        .map_err(|error| format!("failed to discover Python files: {error}"))?;

    let normalization_options = loaded.settings.normalization_options();

    let mut prepared = Vec::with_capacity(discovered.len());

    for path in discovered {
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

        let file = prepare_file(path, source, normalization_options)
            .map_err(|error| format!("failed to prepare Python source: {error}"))?;

        prepared.push(file);
    }

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

fn scan_paths(cli: &Cli) -> Vec<PathBuf> {
    if cli.paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        cli.paths.clone()
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
                std::env::temp_dir().join(format!("arid-mvp-test-{}-{id}", std::process::id()));

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

    #[test]
    fn end_to_end_scan_reports_real_duplicate() {
        let temp = TempDir::new();
        write_test_config(&temp);

        temp.write("a.py", "alpha = 1\nbeta = 2\n");

        temp.write("b.py", "alpha = 1\nbeta = 2\n");

        let result = run(&Cli {
            paths: vec![temp.path().to_path_buf()],
            min_lines: None,
            json: false,
            show_source: false,
        })
        .unwrap();

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

        let result = run(&Cli {
            paths: vec![temp.path().to_path_buf()],
            min_lines: Some(3),
            json: false,
            show_source: false,
        })
        .unwrap();

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

        let result = run(&Cli {
            paths: vec![temp.path().to_path_buf()],
            min_lines: None,
            json: false,
            show_source: false,
        })
        .unwrap();

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

        let result = run(&Cli {
            paths: vec![temp.path().to_path_buf()],
            min_lines: None,
            json: true,
            show_source: false,
        })
        .unwrap();

        assert_eq!(result.exit_status, ExitStatus::Findings);

        let value: serde_json::Value = serde_json::from_str(&result.output).unwrap();

        assert_eq!(value["version"], 3);
        assert_eq!(value["duplicate_groups"], 1);
        assert_eq!(value["findings"][0]["code"], "DUP001");
    }
}
