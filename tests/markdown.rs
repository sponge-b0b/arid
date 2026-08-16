use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
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
            "arid-markdown-integration-test-{}-{id}",
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

#[test]
fn repeated_markdown_scans_are_byte_identical() {
    let temp = duplicate_project();
    let cli = cli(
        &temp,
        [
            OsString::from("--format"),
            OsString::from("markdown"),
            OsString::from("--show-source"),
        ],
    );

    let first = arid::run(&cli).unwrap();
    let second = arid::run(&cli).unwrap();

    assert_eq!(first.exit_status, ExitStatus::Findings);
    assert_eq!(first, second);
    assert!(!first.output.contains('\u{1b}'));
}

#[test]
fn baseline_filtering_applies_to_markdown_output() {
    let temp = duplicate_project();
    let baseline_path = temp.path().join("arid-baseline.json");

    let write_cli = cli(
        &temp,
        [
            OsString::from("--write-baseline"),
            baseline_path.as_os_str().to_owned(),
        ],
    );
    let written = arid::run(&write_cli).unwrap();
    assert_eq!(written.exit_status, ExitStatus::Success);

    let markdown_cli = cli(
        &temp,
        [
            OsString::from("--format"),
            OsString::from("markdown"),
            OsString::from("--baseline"),
            baseline_path.as_os_str().to_owned(),
        ],
    );

    let accepted = arid::run(&markdown_cli).unwrap();
    assert_eq!(accepted.exit_status, ExitStatus::Success);
    assert!(accepted.output.contains("No duplicate code found."));
    assert!(accepted.output.contains("- **Duplicate groups:** 0"));
    assert!(!accepted.output.contains("## `DUP001`"));

    temp.write("c.py", "alpha = 1\nbeta = 2\n");

    let active = arid::run(&markdown_cli).unwrap();
    assert_eq!(active.exit_status, ExitStatus::Findings);
    assert!(
        active
            .output
            .contains("**Occurrences:** 3 across 3 files _(cross-file)_")
    );
    assert!(active.output.contains("### `a.py:1-2`"));
    assert!(active.output.contains("### `b.py:1-2`"));
    assert!(active.output.contains("### `c.py:1-2`"));
    assert!(active.output.contains("- **Duplicate groups:** 1"));
}
