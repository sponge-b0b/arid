use std::path::{Path, PathBuf};

use arid::cli::Cli;
use arid::outcome::ExitStatus;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("pylint")
        .join(name)
}

fn test_cli(path: PathBuf) -> Cli {
    Cli {
        paths: vec![path],
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
        json: true,
        show_source: false,
        baseline: None,
        write_baseline: None,
    }
}

fn run_fixture(name: &str) -> serde_json::Value {
    let result = arid::run(&test_cli(fixture(name)));

    assert_eq!(result.exit_status(), ExitStatus::Findings);
    assert!(result.stderr().is_empty());

    serde_json::from_str(result.stdout()).unwrap()
}

fn assert_single_cross_file_finding(report: &serde_json::Value) {
    assert_eq!(report["duplicate_groups"], 1);

    let finding = &report["findings"][0];

    assert_eq!(finding["code"], "DUP001");
    assert_eq!(finding["lines"], 4);
    assert_eq!(finding["occurrences"], 2);
    assert_eq!(finding["files"], 2);
    assert_eq!(finding["distribution"], "cross-file");
}

#[test]
fn matches_basic_cross_file_duplicate() {
    assert_single_cross_file_finding(&run_fixture("basic-cross-file"));
}

#[test]
fn matches_when_comments_differ() {
    assert_single_cross_file_finding(&run_fixture("ignored-comments"));
}

#[test]
fn matches_when_docstrings_differ() {
    assert_single_cross_file_finding(&run_fixture("ignored-docstrings"));
}

#[test]
fn matches_when_imports_differ() {
    assert_single_cross_file_finding(&run_fixture("ignored-imports"));
}

#[test]
fn matches_when_function_signatures_differ() {
    assert_single_cross_file_finding(&run_fixture("ignored-signatures"));
}

#[test]
fn preserves_hash_characters_inside_strings() {
    let report = run_fixture("hash-inside-string");

    assert_single_cross_file_finding(&report);

    let locations = report["findings"][0]["locations"].as_array().unwrap();

    assert_eq!(locations[0]["start_line"], 1);
    assert_eq!(locations[0]["end_line"], 4);
    assert_eq!(locations[1]["start_line"], 1);
    assert_eq!(locations[1]["end_line"], 4);
}

#[test]
fn detects_same_file_duplicates() {
    let report = run_fixture("same-file");

    assert_eq!(report["duplicate_groups"], 1);

    let finding = &report["findings"][0];

    assert_eq!(finding["code"], "DUP001");
    assert_eq!(finding["lines"], 4);
    assert_eq!(finding["occurrences"], 2);
    assert_eq!(finding["files"], 1);
    assert_eq!(finding["distribution"], "same-file");
}
