use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use arid::{Cli, ExitStatus, run};
use clap::Parser;

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("arid-summary-only-{}-{id}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.path.join(relative);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }

        fs::write(path, contents).unwrap();
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn duplicate_fixture() -> TempDir {
    let temp = TempDir::new();
    temp.write("a.py", "alpha = 1\nbeta = 2\n");
    temp.write("b.py", "alpha = 1\nbeta = 2\n");
    temp
}

fn base_args(temp: &TempDir) -> Vec<OsString> {
    vec![
        OsString::from("arid"),
        temp.path().as_os_str().to_owned(),
        OsString::from("--no-config"),
        OsString::from("--project-root"),
        temp.path().as_os_str().to_owned(),
        OsString::from("--min-lines"),
        OsString::from("2"),
    ]
}

fn scan_cli(temp: &TempDir, extra: &[&str]) -> Cli {
    let mut args = base_args(temp);
    args.extend(extra.iter().map(OsString::from));
    Cli::try_parse_from(args).unwrap()
}

#[test]
fn text_suppresses_findings_without_changing_exit_status() {
    let temp = duplicate_fixture();

    let detailed = run(&scan_cli(&temp, &[]));
    let summary = run(&scan_cli(&temp, &["--summary-only"]));

    assert_eq!(detailed.exit_status(), ExitStatus::Findings);
    assert_eq!(summary.exit_status(), detailed.exit_status());

    assert!(detailed.stdout().contains("DUP001"));

    assert!(!summary.stdout().contains("DUP001"));
    assert!(!summary.stdout().contains("a.py:1-2"));
    assert!(summary.stdout().contains("Summary"));
    assert!(summary.stdout().contains("Breakdown"));
    assert!(summary.stdout().contains("Hotspots"));
    assert!(summary.stdout().contains("Total time:"));
}

#[test]
fn json_emits_summary_v1_instead_of_report_v4() {
    let temp = duplicate_fixture();

    let result = run(&scan_cli(&temp, &["--summary-only", "--json"]));

    assert_eq!(result.exit_status(), ExitStatus::Findings);
    assert!(result.stderr().is_empty());
    assert!(!result.stdout().contains("Total time:"));

    let value: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();

    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["complete"], true);
    assert_eq!(value["files"], 2);
    assert_eq!(value["files_with_duplicates"], 2);
    assert_eq!(value["duplicate_groups"], 1);
    assert_eq!(value["occurrences"], 2);
    assert_eq!(value["analysis"]["ignore_files"], true);
    assert!(value.get("findings").is_none());
}

#[test]
fn json_flag_and_json_format_are_byte_identical() {
    let temp = duplicate_fixture();

    let via_flag = run(&scan_cli(&temp, &["--summary-only", "--json"]));
    let via_format = run(&scan_cli(&temp, &["--summary-only", "--format", "json"]));

    assert_eq!(via_flag.exit_status(), via_format.exit_status());
    assert_eq!(via_flag.stdout(), via_format.stdout());
    assert_eq!(via_flag.stderr(), via_format.stderr());
}

#[test]
fn markdown_and_sarif_primary_output_are_rejected() {
    let temp = duplicate_fixture();

    for format in ["markdown", "sarif"] {
        let result = run(&scan_cli(&temp, &["--summary-only", "--format", format]));

        assert_eq!(result.exit_status(), ExitStatus::Error);
        assert!(result.stdout().is_empty());
        assert!(
            result
                .stderr()
                .contains("--summary-only supports only text or JSON primary output")
        );
    }
}

#[test]
fn supplemental_report_remains_full_report_v4() {
    let temp = duplicate_fixture();
    let report_path = temp.path().join("full-report.json");

    let mut args = base_args(&temp);
    args.push(OsString::from("--summary-only"));
    args.push(OsString::from("--report"));
    args.push(OsString::from(format!(
        "json={}",
        report_path.to_string_lossy()
    )));

    let cli = Cli::try_parse_from(args).unwrap();
    let result = run(&cli);

    assert_eq!(result.exit_status(), ExitStatus::Findings);
    assert!(!result.stdout().contains("DUP001"));
    assert!(result.stdout().contains("Summary"));

    let value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(report_path).unwrap()).unwrap();

    assert_eq!(value["schema_version"], 4);
    assert_eq!(value["duplicate_groups"], 1);
    assert_eq!(value["findings"][0]["code"], "DUP001");
}

#[test]
fn keep_going_json_is_partial_summary_without_timing() {
    let temp = duplicate_fixture();
    temp.write("broken.py", "def broken(:\n");

    let result = run(&scan_cli(
        &temp,
        &["--keep-going", "--summary-only", "--json"],
    ));

    assert_eq!(result.exit_status(), ExitStatus::Error);
    assert!(result.stderr().is_empty());
    assert!(!result.stdout().contains("Total time:"));

    let value: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();

    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["complete"], false);
    assert_eq!(value["analysis"]["keep_going"], true);
    assert_eq!(value["duplicate_groups"], 1);
    assert_eq!(value["errors"].as_array().unwrap().len(), 1);
    assert_eq!(value["errors"][0]["kind"], "parse");
    assert!(value.get("findings").is_none());
}

#[test]
fn keep_going_text_marks_incomplete_and_has_no_timing_footer() {
    let temp = duplicate_fixture();
    temp.write("broken.py", "def broken(:\n");

    let result = run(&scan_cli(&temp, &["--keep-going", "--summary-only"]));

    assert_eq!(result.exit_status(), ExitStatus::Error);
    assert!(result.stderr().is_empty());

    assert!(result.stdout().contains("Scan incomplete:"));
    assert!(result.stdout().contains("Summary"));
    assert!(!result.stdout().contains("DUP001"));
    assert!(!result.stdout().contains("Total time:"));
}

#[test]
fn summary_only_conflicts_with_terminal_administrative_modes() {
    assert!(Cli::try_parse_from(["arid", "--summary-only", "--show-config"]).is_err());
    assert!(Cli::try_parse_from(["arid", "--summary-only", "--list-files"]).is_err());
    assert!(Cli::try_parse_from(["arid", "--summary-only", "--suppression-status"]).is_err());

    assert!(
        Cli::try_parse_from(["arid", "--summary-only", "--explain-path", "src/example.py",])
            .is_err()
    );

    assert!(
        Cli::try_parse_from([
            "arid",
            "--summary-only",
            "--baseline-status",
            "baseline.json",
        ])
        .is_err()
    );

    assert!(
        Cli::try_parse_from([
            "arid",
            "--summary-only",
            "--prune-baseline",
            "baseline.json",
        ])
        .is_err()
    );

    assert!(
        Cli::try_parse_from([
            "arid",
            "--summary-only",
            "--write-baseline",
            "baseline.json",
        ])
        .is_err()
    );
}
