use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use arid::cli::Cli;
use arid::outcome::ExitStatus;
use arid::run;

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("arid-exit-policy-test-{}-{id}", std::process::id()));
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
fn no_fail_maps_complete_findings_to_success_without_changing_report() {
    let (_temp, cli) = duplicate_fixture();

    let default = run(&cli);
    assert_eq!(default.exit_status(), ExitStatus::Findings);

    let mut no_fail = cli;
    no_fail.no_fail_on_findings = true;
    let successful = run(&no_fail);

    assert_eq!(successful.exit_status(), ExitStatus::Success);
    assert_eq!(successful.stdout(), default.stdout());
    assert_eq!(successful.stderr(), default.stderr());
}

#[test]
fn no_fail_preserves_success_when_there_are_no_findings() {
    let temp = TempDir::new();
    write_config(&temp);
    temp.write("a.py", "alpha = 1\nbeta = 2\n");
    temp.write("b.py", "gamma = 3\ndelta = 4\n");

    let mut cli = test_cli(temp.path());
    cli.no_fail_on_findings = true;

    let result = run(&cli);

    assert_eq!(result.exit_status(), ExitStatus::Success);
}

#[test]
fn no_fail_never_masks_incomplete_keep_going_scan() {
    let (temp, mut cli) = duplicate_fixture();
    temp.write("broken.py", "def broken(:\n");
    cli.keep_going = true;
    cli.no_fail_on_findings = true;

    let result = run(&cli);

    assert_eq!(result.exit_status(), ExitStatus::Error);
    assert!(result.stderr().is_empty());
    let report: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
    assert_eq!(report["complete"], false);
    assert_eq!(report["duplicate_groups"], 1);
}

#[test]
fn no_fail_rejects_administrative_modes_through_rust_api() {
    let (_temp, mut cli) = duplicate_fixture();
    cli.json = false;
    cli.no_fail_on_findings = true;
    cli.write_baseline = Some(PathBuf::from("baseline.json"));

    let result = run(&cli);

    assert_eq!(result.exit_status(), ExitStatus::Error);
    assert!(
        result
            .stderr()
            .contains("--no-fail-on-findings is not valid with --write-baseline")
    );
}
