use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use arid::cli::Cli;
use arid::outcome::ExitStatus;
use arid::{RunContext, run, run_with_context};

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "arid-focus-integration-test-{}-{id}",
            std::process::id()
        ));
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

fn write_config(temp: &TempDir) {
    temp.write(
        "pyproject.toml",
        r#"
[tool.arid]
min-lines = 2
"#,
    );
}

fn test_cli(root: &Path) -> Cli {
    Cli {
        paths: vec![root.to_path_buf()],
        config: None,
        no_config: false,
        project_root: None,
        capabilities: false,
        show_config: false,
        list_files: false,
        stdin_path: None,
        keep_going: false,
        focus: Vec::new(),
        no_fail_on_findings: false,
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
        report: Vec::new(),
        color: None,
        json: true,
        show_source: false,
        baseline: None,
        baseline_status: None,
        prune_baseline: None,
        write_baseline: None,
    }
}

fn json(result: &arid::outcome::RunResult) -> serde_json::Value {
    serde_json::from_str(result.stdout()).unwrap()
}

#[test]
fn focus_uses_whole_project_detection_and_keeps_complete_group_context() {
    let temp = TempDir::new();
    write_config(&temp);
    for path in ["a.py", "b.py", "c.py"] {
        temp.write(path, "alpha = 1\nbeta = 2\n");
    }

    let mut cli = test_cli(temp.path());
    cli.focus = vec![PathBuf::from("b.py")];

    let result = run(&cli);

    assert_eq!(result.exit_status(), ExitStatus::Findings);
    let report = json(&result);
    assert_eq!(report["analysis"]["focus"], serde_json::json!(["b.py"]));
    assert_eq!(report["files"], 3);
    assert_eq!(report["duplicate_groups"], 1);
    assert_eq!(report["findings"][0]["files"], 3);
    let locations = report["findings"][0]["locations"].as_array().unwrap();
    assert_eq!(locations.len(), 3);
    assert_eq!(locations[0]["path"], "a.py");
    assert_eq!(locations[1]["path"], "b.py");
    assert_eq!(locations[2]["path"], "c.py");
}

#[test]
fn focus_metadata_is_canonical_sorted_and_deduplicated() {
    let temp = TempDir::new();
    write_config(&temp);
    temp.write("src/a.py", "alpha = 1\nbeta = 2\n");
    temp.write("tests/test_a.py", "alpha = 1\nbeta = 2\n");

    let mut cli = test_cli(temp.path());
    cli.focus = vec![
        PathBuf::from("tests"),
        PathBuf::from("src/a.py"),
        PathBuf::from("tests"),
    ];

    let result = run(&cli);

    assert_eq!(result.exit_status(), ExitStatus::Findings);
    let report = json(&result);
    assert_eq!(
        report["analysis"]["focus"],
        serde_json::json!(["src/a.py", "tests"])
    );
}

#[test]
fn unmatched_focus_is_a_configuration_error() {
    let temp = TempDir::new();
    write_config(&temp);
    temp.write("a.py", "alpha = 1\nbeta = 2\n");

    let mut cli = test_cli(temp.path());
    cli.focus = vec![PathBuf::from("missing")];

    let result = run(&cli);

    assert_eq!(result.exit_status(), ExitStatus::Error);
    assert!(result.stderr().is_empty());
    let error = json(&result);
    assert_eq!(error["error"]["kind"], "configuration");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("--focus does not match any Python source: missing")
    );
}

#[test]
fn focus_matches_virtual_source_before_preparation() {
    let temp = TempDir::new();
    write_config(&temp);
    temp.write("a.py", "alpha = 1\nbeta = 2\n");

    let mut cli = test_cli(temp.path());
    cli.stdin_path = Some(PathBuf::from("proposed.py"));
    cli.focus = vec![PathBuf::from("proposed.py")];

    let result = run_with_context(
        &cli,
        RunContext::non_terminal().with_stdin_source("alpha = 1\nbeta = 2\n".to_owned()),
    );

    assert_eq!(result.exit_status(), ExitStatus::Findings);
    let report = json(&result);
    assert_eq!(
        report["analysis"]["focus"],
        serde_json::json!(["proposed.py"])
    );
    assert_eq!(report["analysis"]["virtual_source"], "proposed.py");
    let locations = report["findings"][0]["locations"].as_array().unwrap();
    assert_eq!(locations.len(), 2);
    assert_eq!(locations[0]["path"], "a.py");
    assert_eq!(locations[1]["path"], "proposed.py");
    assert!(!temp.path().join("proposed.py").exists());
}

#[test]
fn baseline_enforcement_happens_before_focus_filtering() {
    let temp = TempDir::new();
    write_config(&temp);
    temp.write("a.py", "alpha = 1\nbeta = 2\n");
    temp.write("b.py", "alpha = 1\nbeta = 2\n");

    let baseline_path = temp.path().join("baseline.json");
    let mut write_cli = test_cli(temp.path());
    write_cli.json = false;
    write_cli.write_baseline = Some(baseline_path.clone());
    assert_eq!(run(&write_cli).exit_status(), ExitStatus::Success);

    temp.write("c.py", "alpha = 1\nbeta = 2\n");

    let mut cli = test_cli(temp.path());
    cli.baseline = Some(baseline_path);
    cli.focus = vec![PathBuf::from("a.py")];

    let result = run(&cli);

    assert_eq!(result.exit_status(), ExitStatus::Findings);
    let report = json(&result);
    assert_eq!(report["analysis"]["baseline_enabled"], true);
    assert_eq!(report["analysis"]["focus"], serde_json::json!(["a.py"]));
    assert_eq!(report["duplicate_groups"], 1);
    let locations = report["findings"][0]["locations"].as_array().unwrap();
    assert_eq!(locations.len(), 3);
    assert_eq!(locations[0]["path"], "a.py");
    assert_eq!(locations[1]["path"], "b.py");
    assert_eq!(locations[2]["path"], "c.py");
}
