use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use arid::cli::{Cli, ColorWhen};
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
            "arid-multi-output-test-{}-{id}",
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

fn duplicate_fixture() -> (TempDir, Cli) {
    let temp = TempDir::new();
    write_config(&temp);
    temp.write("a.py", "alpha = 1\nbeta = 2\n");
    temp.write("b.py", "alpha = 1\nbeta = 2\n");
    let cli = test_cli(temp.path());
    (temp, cli)
}

#[test]
fn writes_all_concrete_formats_from_the_same_report() {
    let (temp, mut cli) = duplicate_fixture();
    let artifacts = temp.path().join("artifacts");
    fs::create_dir_all(&artifacts).unwrap();
    let json_path = artifacts.join("arid.json");
    let markdown_path = artifacts.join("arid.md");
    let sarif_path = artifacts.join("arid.sarif");
    let text_path = artifacts.join("arid.txt");
    cli.report = vec![
        format!("json={}", json_path.display()),
        format!("markdown={}", markdown_path.display()),
        format!("sarif={}", sarif_path.display()),
        format!("text={}", text_path.display()),
    ];

    let result = run(&cli);

    assert_eq!(result.exit_status(), ExitStatus::Findings);
    assert_eq!(fs::read_to_string(&json_path).unwrap(), result.stdout());

    let report: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
    assert_eq!(report["schema_version"], 4);
    assert_eq!(report["duplicate_groups"], 1);

    let markdown = fs::read_to_string(markdown_path).unwrap();
    assert!(markdown.starts_with("# Arid duplicate-code report\n\n"));
    assert!(markdown.contains("DUP001"));

    let sarif: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(sarif_path).unwrap()).unwrap();
    assert_eq!(sarif["version"], "2.1.0");
    assert_eq!(sarif["runs"][0]["results"][0]["ruleId"], "DUP001");

    let text = fs::read_to_string(text_path).unwrap();
    assert!(text.contains("DUP001"));
    assert!(!text.contains('\u{1b}'));
}

#[test]
fn supplemental_text_is_plain_when_stdout_is_colored() {
    let (temp, mut cli) = duplicate_fixture();
    let text_path = temp.path().join("report.txt");
    cli.json = false;
    cli.color = Some(ColorWhen::Always);
    cli.report = vec![format!("text={}", text_path.display())];

    let result = run(&cli);

    assert_eq!(result.exit_status(), ExitStatus::Findings);
    assert!(result.stdout().contains('\u{1b}'));
    let text = fs::read_to_string(text_path).unwrap();
    assert!(text.contains("DUP001"));
    assert!(!text.contains('\u{1b}'));
}

#[test]
fn rejects_duplicate_report_destinations() {
    let (temp, mut cli) = duplicate_fixture();
    let path = temp.path().join("report.json");
    cli.report = vec![
        format!("json={}", path.display()),
        format!("text={}", path.display()),
    ];

    let result = run(&cli);

    assert_eq!(result.exit_status(), ExitStatus::Error);
    let error: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
    assert_eq!(error["error"]["kind"], "configuration");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("duplicate --report destination")
    );
}

#[test]
fn rejects_report_destination_overlapping_disk_source() {
    let (temp, mut cli) = duplicate_fixture();
    let source = temp.path().join("a.py");
    let original = fs::read_to_string(&source).unwrap();
    cli.report = vec![format!("json={}", source.display())];

    let result = run(&cli);

    assert_eq!(result.exit_status(), ExitStatus::Error);
    assert_eq!(fs::read_to_string(source).unwrap(), original);
    let error: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
    assert_eq!(error["error"]["kind"], "configuration");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("overlaps a source file")
    );
}

#[test]
fn rejects_report_destination_overlapping_virtual_source() {
    let (temp, mut cli) = duplicate_fixture();
    let virtual_path = temp.path().join("proposed.py");
    cli.stdin_path = Some(PathBuf::from("proposed.py"));
    cli.report = vec![format!("json={}", virtual_path.display())];

    let result = run_with_context(
        &cli,
        RunContext::non_terminal().with_stdin_source("alpha = 1\nbeta = 2\n".to_owned()),
    );

    assert_eq!(result.exit_status(), ExitStatus::Error);
    assert!(!virtual_path.exists());
    let error: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
    assert_eq!(error["error"]["kind"], "configuration");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("overlaps a source file")
    );
}

#[test]
fn rejects_report_destination_overlapping_active_baseline() {
    let (temp, mut cli) = duplicate_fixture();
    let baseline = temp.path().join("baseline.json");
    cli.baseline = Some(baseline.clone());
    cli.report = vec![format!("json={}", baseline.display())];

    let result = run(&cli);

    assert_eq!(result.exit_status(), ExitStatus::Error);
    let error: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
    assert_eq!(error["error"]["kind"], "configuration");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("overlaps the active baseline")
    );
}

#[test]
fn incomplete_scan_writes_partial_json_but_suppresses_sarif() {
    let (temp, mut cli) = duplicate_fixture();
    temp.write("broken.py", "def broken(:\n");
    let artifacts = temp.path().join("artifacts");
    fs::create_dir_all(&artifacts).unwrap();
    let json_path = artifacts.join("partial.json");
    let sarif_path = artifacts.join("partial.sarif");
    cli.keep_going = true;
    cli.report = vec![
        format!("json={}", json_path.display()),
        format!("sarif={}", sarif_path.display()),
    ];

    let result = run(&cli);

    assert_eq!(result.exit_status(), ExitStatus::Error);
    assert_eq!(fs::read_to_string(json_path).unwrap(), result.stdout());
    assert!(!sarif_path.exists());
    let report: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
    assert_eq!(report["complete"], false);
    assert_eq!(report["duplicate_groups"], 1);
}

#[test]
fn report_write_failure_returns_operational_error() {
    let (temp, mut cli) = duplicate_fixture();
    let path = temp.path().join("missing").join("report.json");
    cli.report = vec![format!("json={}", path.display())];

    let result = run(&cli);

    assert_eq!(result.exit_status(), ExitStatus::Error);
    let error: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
    assert_eq!(error["error"]["kind"], "output");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("failed to open report destination")
    );
}

#[test]
fn report_rejects_administrative_modes_through_rust_api() {
    let (temp, mut cli) = duplicate_fixture();
    cli.json = false;
    cli.report = vec![format!(
        "json={}",
        temp.path().join("report.json").display()
    )];
    cli.write_baseline = Some(temp.path().join("baseline.json"));

    let result = run(&cli);

    assert_eq!(result.exit_status(), ExitStatus::Error);
    assert!(
        result
            .stderr()
            .contains("--report is not valid with --write-baseline")
    );
}
