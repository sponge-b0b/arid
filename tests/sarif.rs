use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use arid::{Cli, ExitStatus};
use clap::Parser;

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "arid-sarif-integration-test-{}-{id}",
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
    temp.write("space dir/a file.py", "alpha = 1\nbeta = 2\n");
    temp.write("unicode/naïve.py", "alpha = 1\nbeta = 2\n");
    temp
}

fn cli(temp: &TempDir, options: impl IntoIterator<Item = OsString>) -> Cli {
    let mut args = vec![OsString::from("arid")];
    args.extend(options);
    args.push(temp.path().as_os_str().to_owned());

    Cli::try_parse_from(args).unwrap()
}

#[test]
fn real_sarif_scan_is_deterministic_and_uri_safe() {
    let temp = duplicate_project();
    let sarif_cli = cli(
        &temp,
        [
            OsString::from("--format"),
            OsString::from("sarif"),
            OsString::from("--show-source"),
        ],
    );

    let first = arid::run(&sarif_cli);
    let second = arid::run(&sarif_cli);

    assert_eq!(first.exit_status(), ExitStatus::Findings);
    assert_eq!(first, second);
    assert!(!first.stdout().contains('\u{1b}'));

    let value: serde_json::Value = serde_json::from_str(first.stdout()).unwrap();
    let result = &value["runs"][0]["results"][0];

    assert_eq!(value["version"], "2.1.0");
    assert_eq!(value["runs"][0]["results"].as_array().unwrap().len(), 1);
    assert!(result.get("level").is_none());
    assert_eq!(result["locations"].as_array().unwrap().len(), 1);
    assert_eq!(result["relatedLocations"].as_array().unwrap().len(), 1);

    let json_cli = cli(&temp, [OsString::from("--format"), OsString::from("json")]);
    let json = arid::run(&json_cli);
    assert_eq!(json.exit_status(), ExitStatus::Findings);
    let json_value: serde_json::Value = serde_json::from_str(json.stdout()).unwrap();

    assert_eq!(
        result["partialFingerprints"]["aridFindingFingerprint/v1"],
        json_value["findings"][0]["fingerprint"]
    );
    assert!(
        result["partialFingerprints"]
            .get("primaryLocationLineHash")
            .is_none()
    );

    let primary = result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
        .as_str()
        .unwrap();
    let related = result["relatedLocations"][0]["physicalLocation"]["artifactLocation"]["uri"]
        .as_str()
        .unwrap();

    assert_eq!(primary, "space%20dir/a%20file.py");
    assert_eq!(related, "unicode/na%C3%AFve.py");
    assert_eq!(
        result["locations"][0]["physicalLocation"]["region"]["snippet"]["text"],
        "alpha = 1\nbeta = 2"
    );
}

#[test]
fn baseline_filtering_applies_to_sarif_output() {
    let temp = duplicate_project();
    let baseline_path = temp.path().join("arid-baseline.json");

    let write_cli = cli(
        &temp,
        [
            OsString::from("--write-baseline"),
            baseline_path.as_os_str().to_owned(),
        ],
    );
    let written = arid::run(&write_cli);
    assert_eq!(written.exit_status(), ExitStatus::Success);

    let sarif_cli = cli(
        &temp,
        [
            OsString::from("--format"),
            OsString::from("sarif"),
            OsString::from("--baseline"),
            baseline_path.as_os_str().to_owned(),
        ],
    );

    let accepted = arid::run(&sarif_cli);
    assert_eq!(accepted.exit_status(), ExitStatus::Success);
    let accepted_value: serde_json::Value = serde_json::from_str(accepted.stdout()).unwrap();
    assert!(
        accepted_value["runs"][0]["results"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    temp.write("new.py", "alpha = 1\nbeta = 2\n");

    let active = arid::run(&sarif_cli);
    assert_eq!(active.exit_status(), ExitStatus::Findings);
    let active_value: serde_json::Value = serde_json::from_str(active.stdout()).unwrap();
    let result = &active_value["runs"][0]["results"][0];

    assert_eq!(
        active_value["runs"][0]["results"].as_array().unwrap().len(),
        1
    );
    assert_eq!(result["locations"].as_array().unwrap().len(), 1);
    assert_eq!(result["relatedLocations"].as_array().unwrap().len(), 2);
    assert_eq!(result["properties"]["occurrences"], 3);
    assert_eq!(result["properties"]["files"], 3);
}
