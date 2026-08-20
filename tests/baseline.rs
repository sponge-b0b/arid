use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use arid::cli::Cli;
use arid::outcome::ExitStatus;
use clap::Parser;

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "arid-baseline-integration-test-{}-{id}",
            std::process::id()
        ));

        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("pyproject.toml"), "[tool.arid]\nmin-lines = 2\n").unwrap();

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

fn duplicate_project() -> TempDir {
    let temp = TempDir::new();
    temp.write("a.py", "alpha = 1\nbeta = 2\n");
    temp.write("b.py", "alpha = 1\nbeta = 2\n");
    temp
}

fn cli(temp: &TempDir, options: impl IntoIterator<Item = OsString>) -> Cli {
    let mut args = vec![OsString::from("arid")];
    args.extend(options);
    args.push(temp.path().as_os_str().to_owned());

    Cli::try_parse_from(args).unwrap()
}

fn write_baseline(temp: &TempDir) -> PathBuf {
    let path = temp.path().join("arid-baseline.json");
    let result = arid::run(&cli(
        temp,
        [
            OsString::from("--write-baseline"),
            path.as_os_str().to_owned(),
        ],
    ));

    assert_eq!(result.exit_status(), ExitStatus::Success);
    assert!(result.stdout().is_empty());
    assert!(result.stderr().is_empty());
    assert!(path.is_file());

    path
}

fn enforce(temp: &TempDir, baseline: &Path) -> arid::RunResult {
    arid::run(&cli(
        temp,
        [
            OsString::from("--baseline"),
            baseline.as_os_str().to_owned(),
        ],
    ))
}

#[test]
fn unchanged_baseline_has_no_active_finding() {
    let temp = duplicate_project();
    let baseline = write_baseline(&temp);

    let result = enforce(&temp, &baseline);

    assert_eq!(result.exit_status(), ExitStatus::Success);
    assert!(result.stdout().contains("No duplicate code found."));
}

#[test]
fn unrelated_lines_inserted_above_remain_accepted() {
    let temp = duplicate_project();
    let baseline = write_baseline(&temp);

    temp.write("a.py", "prefix = 0\nalpha = 1\nbeta = 2\n");

    let result = enforce(&temp, &baseline);

    assert_eq!(result.exit_status(), ExitStatus::Success);
    assert!(result.stdout().contains("No duplicate code found."));
}

#[test]
fn new_occurrence_in_existing_file_is_active() {
    let temp = duplicate_project();
    let baseline = write_baseline(&temp);

    temp.write("a.py", "alpha = 1\nbeta = 2\nalpha = 1\nbeta = 2\n");

    let result = enforce(&temp, &baseline);

    assert_eq!(result.exit_status(), ExitStatus::Findings);
    assert!(result.stdout().contains("DUP001"));
}

#[test]
fn occurrence_in_new_file_is_active() {
    let temp = duplicate_project();
    let baseline = write_baseline(&temp);

    temp.write("c.py", "alpha = 1\nbeta = 2\n");

    let result = enforce(&temp, &baseline);

    assert_eq!(result.exit_status(), ExitStatus::Findings);
    assert!(result.stdout().contains("c.py:1-2"));
}

#[test]
fn renamed_file_is_conservatively_active() {
    let temp = duplicate_project();
    let baseline = write_baseline(&temp);

    fs::rename(temp.path().join("b.py"), temp.path().join("renamed.py")).unwrap();

    let result = enforce(&temp, &baseline);

    assert_eq!(result.exit_status(), ExitStatus::Findings);
    assert!(result.stdout().contains("renamed.py:1-2"));
}

#[test]
fn changed_normalized_duplicate_is_active() {
    let temp = duplicate_project();
    let baseline = write_baseline(&temp);

    temp.write("a.py", "gamma = 3\ndelta = 4\n");
    temp.write("b.py", "gamma = 3\ndelta = 4\n");

    let result = enforce(&temp, &baseline);

    assert_eq!(result.exit_status(), ExitStatus::Findings);
    assert!(result.stdout().contains("DUP001"));
}

#[test]
fn removed_accepted_duplicate_does_not_leave_stale_finding() {
    let temp = duplicate_project();
    let baseline = write_baseline(&temp);

    temp.write("b.py", "gamma = 3\ndelta = 4\n");

    let result = enforce(&temp, &baseline);

    assert_eq!(result.exit_status(), ExitStatus::Success);
    assert!(result.stdout().contains("No duplicate code found."));
}

#[test]
fn normalization_mismatch_is_an_error() {
    let temp = duplicate_project();
    let baseline = write_baseline(&temp);

    let error = arid::run(&cli(
        &temp,
        [
            OsString::from("--baseline"),
            baseline.as_os_str().to_owned(),
            OsString::from("--no-ignore-comments"),
        ],
    ));

    assert_eq!(error.exit_status(), ExitStatus::Error);
    assert!(error.stdout().is_empty());
    assert!(
        error
            .stderr()
            .contains("normalization settings do not match")
    );
}

#[test]
fn malformed_baseline_returns_process_exit_status_two() {
    let temp = duplicate_project();
    let baseline = temp.path().join("malformed.json");
    fs::write(&baseline, "{not-json").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_arid"))
        .arg("--baseline")
        .arg(&baseline)
        .arg(temp.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("failed to load baseline"));
}

#[test]
fn baseline_creation_returns_process_exit_status_zero_with_duplicates() {
    let temp = duplicate_project();
    let baseline = temp.path().join("created.json");

    let output = Command::new(env!("CARGO_BIN_EXE_arid"))
        .arg("--write-baseline")
        .arg(&baseline)
        .arg(temp.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
    assert!(baseline.is_file());
}
