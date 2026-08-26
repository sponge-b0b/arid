use std::fs;
use std::path::{Path, PathBuf};

use rayon::prelude::*;

use crate::cli::{Cli, OutputFormat};
use crate::config::{ProjectOptions, SettingsOverrides, load_settings_with_options};
use crate::error::{ErrorKind, OperationalError, render_error_json};
use crate::exit_policy::apply_fail_on_stale;
use crate::files::{DiscoveryPolicy, discover_python_files};
use crate::model::NormalizationOptions;
use crate::normalize::{SuppressionMode, prepare_file_with_mode};
use crate::outcome::{ExitStatus, RunResult};
use crate::output::{resolve_administrative_json_targets, write_atomic_output};
use crate::source::{SourceInput, build_source_inputs};
use crate::suppression::{
    AuditPreparedFile, build_suppression_status, render_suppression_status_json,
    render_suppression_status_text,
};

pub(crate) fn run(cli: &Cli) -> RunResult {
    let output_format = cli.output_format();

    match execute(cli) {
        Ok(result) => result,
        Err(error) if output_format == OutputFormat::Json => match render_error_json(error) {
            Ok(output) => RunResult::new(output, "", ExitStatus::Error),
            Err(render_error) => RunResult::failure(format!(
                "failed to render JSON operational error: {render_error}"
            )),
        },
        Err(error) => RunResult::failure(error.to_string()),
    }
}

fn execute(cli: &Cli) -> Result<RunResult, OperationalError> {
    validate_options(cli)?;

    let paths = scan_paths(cli);
    let loaded =
        load_settings_with_options(&paths[0], settings_overrides(cli), project_options(cli))
            .map_err(|error| {
                OperationalError::new(
                    ErrorKind::Configuration,
                    format!("failed to load configuration: {error}"),
                )
            })?;
    let discovery_policy = DiscoveryPolicy::new(!cli.no_ignore_files);

    let discovered = discover_python_files(
        &paths,
        &loaded.settings,
        &loaded.project_root,
        discovery_policy,
    )
    .map_err(|error| {
        OperationalError::new(
            ErrorKind::Discovery,
            format!("failed to discover Python files: {error}"),
        )
    })?;
    let report_targets = resolve_administrative_json_targets(
        &cli.report,
        &discovered,
        "--suppression-status",
    )?;
    let inputs = build_source_inputs(discovered, None);
    let prepared = prepare_sources_for_audit(
        inputs,
        loaded.settings.normalization_options(),
        cli.workers,
        &loaded.project_root,
    )?;
    let status = build_suppression_status(
        prepared,
        &loaded.settings,
        &loaded.project_root,
        discovery_policy.ignore_files(),
    )
    .map_err(|error| {
        OperationalError::new(
            ErrorKind::Internal,
            format!("failed to audit suppressions: {error}"),
        )
    })?;

    let exit_status = apply_fail_on_stale(
        ExitStatus::Success,
        status.has_stale(),
        cli.fail_on_stale,
    );
    let json_output = if cli.output_format() == OutputFormat::Json || !report_targets.is_empty() {
        Some(render_suppression_status_json(&status).map_err(|error| {
            OperationalError::new(
                ErrorKind::Output,
                format!("failed to render suppression status JSON: {error}"),
            )
        })?)
    } else {
        None
    };

    if let Some(json) = json_output.as_deref() {
        for target in &report_targets {
            write_atomic_output(&target.path, json, &loaded.project_root)?;
        }
    }

    let output = match cli.output_format() {
        OutputFormat::Text => render_suppression_status_text(&status),
        OutputFormat::Json => json_output.expect("JSON output is rendered for JSON stdout"),
        OutputFormat::Markdown | OutputFormat::Sarif => {
            unreachable!("suppression status output is validated before execution")
        }
    };

    Ok(RunResult::new(output, "", exit_status))
}

fn validate_options(cli: &Cli) -> Result<(), OperationalError> {
    let invalid = [
        (cli.capabilities, "--capabilities"),
        (cli.show_config, "--show-config"),
        (cli.list_files, "--list-files"),
        (cli.stdin_path.is_some(), "--stdin-path"),
        (cli.keep_going, "--keep-going"),
        (!cli.focus.is_empty(), "--focus"),
        (cli.no_fail_on_findings, "--no-fail-on-findings"),
        (cli.baseline.is_some(), "--baseline"),
        (cli.baseline_status.is_some(), "--baseline-status"),
        (cli.prune_baseline.is_some(), "--prune-baseline"),
        (cli.write_baseline.is_some(), "--write-baseline"),
        (cli.show_source, "--show-source"),
        (cli.color.is_some(), "--color"),
    ];

    if let Some((_, option)) = invalid.into_iter().find(|(present, _)| *present) {
        return Err(OperationalError::new(
            ErrorKind::Configuration,
            format!("{option} is not valid with --suppression-status"),
        ));
    }

    if matches!(
        cli.output_format(),
        OutputFormat::Markdown | OutputFormat::Sarif
    ) {
        return Err(OperationalError::new(
            ErrorKind::Configuration,
            "--suppression-status supports only text or JSON output",
        ));
    }

    Ok(())
}

fn prepare_sources_for_audit(
    inputs: Vec<SourceInput>,
    options: NormalizationOptions,
    workers: usize,
    project_root: &Path,
) -> Result<Vec<AuditPreparedFile>, OperationalError> {
    if workers == 0 {
        return Err(OperationalError::new(
            ErrorKind::Configuration,
            "worker count must be at least 1",
        ));
    }

    if workers == 1 || inputs.len() < 2 {
        return inputs
            .into_iter()
            .map(|input| prepare_source_for_audit(input, options, project_root))
            .collect();
    }

    let worker_count = workers.min(inputs.len());
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(worker_count)
        .build()
        .map_err(|error| {
            OperationalError::new(
                ErrorKind::Internal,
                format!("failed to create worker pool: {error}"),
            )
        })?;

    let results = pool.install(|| {
        inputs
            .into_par_iter()
            .map(|input| prepare_source_for_audit(input, options, project_root))
            .collect::<Vec<_>>()
    });

    results.into_iter().collect()
}

fn prepare_source_for_audit(
    input: SourceInput,
    options: NormalizationOptions,
    project_root: &Path,
) -> Result<AuditPreparedFile, OperationalError> {
    let (path, source) = match input {
        SourceInput::Disk(path) => {
            let source = fs::read_to_string(&path).map_err(|error| {
                OperationalError::new(
                    ErrorKind::Read,
                    format!("failed to read {}: {error}", path.display()),
                )
                .with_project_path(&path, project_root)
            })?;
            (path, source)
        }
        SourceInput::Virtual { path, source } => (path, source),
    };

    let (file, suppressions) = prepare_file_with_mode(
        path.clone(),
        source,
        options,
        SuppressionMode::Audit,
    )
    .map_err(|error| {
        OperationalError::new(
            ErrorKind::Parse,
            format!("failed to prepare Python source: {error}"),
        )
        .with_project_path(&path, project_root)
    })?;

    Ok(AuditPreparedFile {
        file,
        suppressions,
    })
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

fn project_options(cli: &Cli) -> ProjectOptions {
    ProjectOptions {
        config: cli.config.clone(),
        no_config: cli.no_config,
        project_root: cli.project_root.clone(),
    }
}

fn boolean_override(enabled: bool, disabled: bool) -> Option<bool> {
    match (enabled, disabled) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        (false, false) => None,
        (true, true) => unreachable!("conflicting CLI flags must be rejected by clap"),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use clap::Parser;

    use super::*;

    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "arid-suppression-status-test-{}-{id}",
                std::process::id()
            ));
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

    fn fixture() -> TempDir {
        let temp = TempDir::new();
        temp.write("a.py", "alpha = 1\nbeta = 2\n");
        temp.write(
            "b.py",
            "# arid: disable\nalpha = 1\nbeta = 2\n# arid: enable\n",
        );
        temp.write(
            "c.py",
            "# arid: disable\ngamma = 3\ndelta = 4\n# arid: enable\n",
        );
        temp
    }

    fn cli(temp: &TempDir, json: bool, workers: usize) -> Cli {
        let mut args = vec![
            "arid".to_owned(),
            "--suppression-status".to_owned(),
            "--min-lines".to_owned(),
            "2".to_owned(),
            "--workers".to_owned(),
            workers.to_string(),
        ];
        if json {
            args.push("--json".to_owned());
        }
        args.push(temp.path().to_string_lossy().into_owned());
        Cli::try_parse_from(args).unwrap()
    }

    #[test]
    fn text_status_is_end_to_end() {
        let temp = fixture();
        let result = run(&cli(&temp, false, 1));

        assert_eq!(result.exit_status(), ExitStatus::Success);
        assert!(result.stderr().is_empty());
        assert!(result.stdout().contains("Active suppressions: 1"));
        assert!(result.stdout().contains("Stale suppressions: 1"));
        assert!(result.stdout().contains("active: b.py:1-4 (enable)"));
        assert!(result.stdout().contains("stale: c.py:1-4 (enable)"));
    }

    #[test]
    fn fail_on_stale_turns_stale_suppression_into_findings_exit() {
        let temp = fixture();
        let mut cli = cli(&temp, false, 1);
        cli.fail_on_stale = true;

        let result = run(&cli);

        assert_eq!(result.exit_status(), ExitStatus::Findings);
        assert!(result.stdout().contains("Stale suppressions: 1"));
        assert!(result.stderr().is_empty());
    }

    #[test]
    fn no_ignore_files_changes_suppression_discovery() {
        let temp = TempDir::new();
        fs::create_dir_all(temp.path().join(".git")).unwrap();
        temp.write("a.py", "alpha = 1\nbeta = 2\n");
        temp.write(
            "generated/ignored.py",
            "# arid: disable\ngamma = 3\ndelta = 4\n# arid: enable\n",
        );
        temp.write(".gitignore", "generated/\n");

        let mut normal_cli = cli(&temp, true, 1);
        let normal = run(&normal_cli);
        let normal_json: serde_json::Value = serde_json::from_str(normal.stdout()).unwrap();
        assert_eq!(normal_json["files"], 1);
        assert_eq!(normal_json["summary"]["total"], 0);
        assert_eq!(normal_json["analysis"]["ignore_files"], true);

        normal_cli.no_ignore_files = true;
        let overridden = run(&normal_cli);
        let overridden_json: serde_json::Value =
            serde_json::from_str(overridden.stdout()).unwrap();
        assert_eq!(overridden_json["files"], 2);
        assert_eq!(overridden_json["summary"]["total"], 1);
        assert_eq!(overridden_json["summary"]["stale"], 1);
        assert_eq!(overridden_json["analysis"]["ignore_files"], false);
    }

    #[test]
    fn json_status_is_deterministic_across_worker_modes() {
        let temp = fixture();
        let serial = run(&cli(&temp, true, 1));
        let parallel = run(&cli(&temp, true, 4));

        assert_eq!(serial.exit_status(), ExitStatus::Success);
        assert_eq!(parallel.exit_status(), ExitStatus::Success);
        assert_eq!(serial.stdout(), parallel.stdout());

        let value: serde_json::Value = serde_json::from_str(serial.stdout()).unwrap();
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["files"], 3);
        assert_eq!(value["summary"]["active"], 1);
        assert_eq!(value["summary"]["stale"], 1);
        assert_eq!(value["regions"][0]["path"], "b.py");
        assert_eq!(value["regions"][1]["path"], "c.py");
    }

    #[test]
    fn supplemental_json_matches_json_stdout() {
        let temp = fixture();
        let report_path = temp.path().join("suppression-status.json");
        let mut text_cli = cli(&temp, false, 1);
        text_cli.report = vec![format!("json={}", report_path.display())];

        let text = run(&text_cli);
        assert_eq!(text.exit_status(), ExitStatus::Success);
        let direct = fs::read_to_string(&report_path).unwrap();

        let json = run(&cli(&temp, true, 1));
        assert_eq!(json.exit_status(), ExitStatus::Success);
        assert_eq!(direct, json.stdout());
    }

    #[test]
    fn rejects_non_json_supplemental_report() {
        let temp = fixture();
        let mut cli = cli(&temp, false, 1);
        cli.report = vec![format!("text={}", temp.path().join("status.txt").display())];

        let result = run(&cli);

        assert_eq!(result.exit_status(), ExitStatus::Error);
        assert!(
            result
                .stderr()
                .contains("supports only json=PATH supplemental reports")
        );
    }

    #[test]
    fn source_failure_does_not_emit_partial_status() {
        let temp = fixture();
        temp.write("broken.py", "def broken(:\n");
        let result = run(&cli(&temp, false, 1));

        assert_eq!(result.exit_status(), ExitStatus::Error);
        assert!(result.stdout().is_empty());
        assert!(result.stderr().contains("broken.py"));
        assert!(!result.stderr().contains("Suppression status"));
    }

    #[test]
    fn json_failure_uses_existing_error_contract() {
        let temp = fixture();
        temp.write("broken.py", "def broken(:\n");
        let result = run(&cli(&temp, true, 1));

        assert_eq!(result.exit_status(), ExitStatus::Error);
        assert!(result.stderr().is_empty());
        let value: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["error"]["kind"], "parse");
    }
}
