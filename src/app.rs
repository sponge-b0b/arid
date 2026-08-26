use std::ffi::OsStr;
use std::path::PathBuf;

use crate::baseline::{build_baseline, read_baseline, serialize_baseline, write_baseline};
use crate::baseline_filter::{compare_baseline, filter_active_groups};
use crate::baseline_status::{render_baseline_status_json, render_baseline_status_text};
use crate::capabilities::render_capabilities_json;
use crate::cli::{Cli, ColorWhen, OutputFormat};
use crate::config::{
    LoadedSettings, ProjectOptions, SettingsOverrides, load_settings_with_options,
};
use crate::corpus::build_corpus;
use crate::detect::detect_duplicates;
use crate::error::{ErrorKind, OperationalError, render_error_json};
use crate::exit_policy::{apply_fail_on_stale, apply_no_fail_on_findings};
use crate::files::{DiscoveryPolicy, discover_python_files};
use crate::introspection::{
    discovered_file_names, render_config_json, render_config_text, render_file_list_json,
    render_file_list_text,
};
use crate::markdown::render_markdown;
use crate::outcome::{ExitStatus, RunResult};
use crate::output::{resolve_report_targets, write_report_targets};
use crate::report::{AnalysisMetadata, ReportOptions, build_report, render_json};
use crate::sarif::render_sarif;
use crate::source::focus::resolve_focus;
use crate::source::{
    build_source_inputs, prepare_sources, prepare_sources_with_policy, resolve_virtual_source,
};
use crate::text::render_text;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ColorEnvironment {
    no_color: bool,
    clicolor_force: bool,
    clicolor_disabled: bool,
}

impl ColorEnvironment {
    #[must_use]
    pub const fn new(no_color: bool, clicolor_force: bool, clicolor_disabled: bool) -> Self {
        Self {
            no_color,
            clicolor_force,
            clicolor_disabled,
        }
    }

    #[must_use]
    pub fn from_process() -> Self {
        Self::new(
            std::env::var_os("NO_COLOR").is_some_and(|value| !value.as_os_str().is_empty()),
            std::env::var_os("CLICOLOR_FORCE")
                .is_some_and(|value| value.as_os_str() != OsStr::new("0")),
            std::env::var_os("CLICOLOR").is_some_and(|value| value.as_os_str() == OsStr::new("0")),
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunContext {
    text_color_capable: bool,
    color_environment: ColorEnvironment,
    stdin_source: Option<String>,
}

impl RunContext {
    #[must_use]
    pub const fn non_terminal() -> Self {
        Self::new(false, ColorEnvironment::new(false, false, false))
    }

    #[must_use]
    pub const fn new(text_color_capable: bool, color_environment: ColorEnvironment) -> Self {
        Self {
            text_color_capable,
            color_environment,
            stdin_source: None,
        }
    }

    #[must_use]
    pub fn with_stdin_source(mut self, source: String) -> Self {
        self.stdin_source = Some(source);
        self
    }
}

/// Runs one complete Arid scan with deterministic non-terminal defaults.
#[must_use]
pub fn run(cli: &Cli) -> RunResult {
    run_with_context(cli, RunContext::non_terminal())
}

/// Runs one complete Arid scan with explicit output-environment context.
///
/// Application pipeline:
///
/// discover → source input → normalize → corpus → detect → baseline → focus → report
#[must_use]
pub fn run_with_context(cli: &Cli, context: RunContext) -> RunResult {
    let output_format = cli.output_format();

    match execute(cli, context) {
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

fn execute(cli: &Cli, context: RunContext) -> Result<RunResult, OperationalError> {
    if cli.capabilities {
        let output = render_capabilities_json().map_err(|error| {
            OperationalError::new(
                ErrorKind::Internal,
                format!("failed to render capabilities JSON: {error}"),
            )
        })?;
        return Ok(RunResult::new(output, "", ExitStatus::Success));
    }

    let output_format = cli.output_format();

    validate_output_options(cli, output_format)?;
    let text_color = resolve_text_color(cli.color, &context);

    let paths = scan_paths(cli);

    let loaded =
        load_settings_with_options(&paths[0], settings_overrides(cli), project_options(cli))
            .map_err(|error| {
                OperationalError::new(
                    ErrorKind::Configuration,
                    format!("failed to load configuration: {error}"),
                )
            })?;
    let baseline_path = selected_baseline_path(cli, &loaded);

    if cli.show_config {
        let output = match output_format {
            OutputFormat::Text => {
                render_config_text(&loaded, cli.no_config, baseline_path.as_deref())
            }
            OutputFormat::Json => {
                render_config_json(&loaded, cli.no_config, baseline_path.as_deref()).map_err(
                    |error| {
                        OperationalError::new(
                            ErrorKind::Output,
                            format!("failed to render configuration JSON: {error}"),
                        )
                    },
                )?
            }
            OutputFormat::Markdown | OutputFormat::Sarif => {
                unreachable!("configuration introspection output is validated before execution")
            }
        };

        return Ok(RunResult::new(output, "", ExitStatus::Success));
    }

    let normalization = loaded.settings.normalization_options();
    let virtual_source = resolve_virtual_source(
        cli.stdin_path.as_deref(),
        context.stdin_source.as_deref(),
        &loaded.settings,
        &loaded.project_root,
    )?;
    let virtual_project_path = virtual_source
        .as_ref()
        .map(|source| source.project_path().to_owned());
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

    if cli.list_files {
        let files = discovered_file_names(&discovered, &loaded.project_root);
        let output = match output_format {
            OutputFormat::Text => render_file_list_text(&files),
            OutputFormat::Json => render_file_list_json(&files).map_err(|error| {
                OperationalError::new(
                    ErrorKind::Output,
                    format!("failed to render discovered file list JSON: {error}"),
                )
            })?,
            OutputFormat::Markdown | OutputFormat::Sarif => {
                unreachable!("file-list introspection output is validated before execution")
            }
        };

        return Ok(RunResult::new(output, "", ExitStatus::Success));
    }

    let source_inputs = build_source_inputs(discovered, virtual_source);
    let report_targets =
        resolve_report_targets(&cli.report, &source_inputs, baseline_path.as_deref())?;
    let focus = resolve_focus(&cli.focus, &source_inputs, &loaded.project_root)?;
    let analysis = analysis_metadata(
        &loaded,
        baseline_path.is_some(),
        focus.selectors().to_vec(),
        virtual_project_path,
        cli.keep_going,
    );
    let (prepared, preparation_errors) = if cli.keep_going {
        let prepared = prepare_sources_with_policy(
            source_inputs,
            normalization,
            cli.workers,
            &loaded.project_root,
            true,
        )?;
        (prepared.files, prepared.errors)
    } else {
        (
            prepare_sources(
                source_inputs,
                normalization,
                cli.workers,
                &loaded.project_root,
            )?,
            Vec::new(),
        )
    };
    let complete = preparation_errors.is_empty();

    let corpus = build_corpus(prepared).map_err(|error| {
        OperationalError::new(
            ErrorKind::Internal,
            format!("failed to build source corpus: {error}"),
        )
    })?;

    let groups =
        detect_duplicates(&corpus, loaded.settings.detection_options()).map_err(|error| {
            OperationalError::new(
                ErrorKind::Internal,
                format!("failed to detect duplicates: {error}"),
            )
        })?;

    if let Some(path) = &cli.write_baseline {
        let baseline = build_baseline(&corpus, &groups, normalization, &loaded.project_root)
            .map_err(|error| {
                OperationalError::new(
                    ErrorKind::Baseline,
                    format!("failed to build baseline: {error}"),
                )
            })?;

        write_baseline(path, &baseline).map_err(|error| {
            OperationalError::new(
                ErrorKind::Baseline,
                format!("failed to write baseline: {error}"),
            )
            .with_project_path(path, &loaded.project_root)
        })?;

        return Ok(RunResult::new("", "", ExitStatus::Success));
    }

    let baseline_administration = cli
        .baseline_status
        .as_ref()
        .map(|path| (path, false))
        .or_else(|| cli.prune_baseline.as_ref().map(|path| (path, true)));

    if let Some((path, prune)) = baseline_administration {
        let baseline = read_baseline(path, normalization).map_err(|error| {
            OperationalError::new(
                ErrorKind::Baseline,
                format!("failed to load baseline: {error}"),
            )
            .with_project_path(path, &loaded.project_root)
        })?;
        let comparison = compare_baseline(
            &corpus,
            groups,
            &baseline,
            normalization,
            &loaded.project_root,
        )
        .map_err(|error| {
            OperationalError::new(
                ErrorKind::Baseline,
                format!("failed to compare baseline: {error}"),
            )
            .with_project_path(path, &loaded.project_root)
        })?;

        if prune {
            let current = serialize_baseline(&baseline).map_err(|error| {
                OperationalError::new(
                    ErrorKind::Baseline,
                    format!("failed to serialize baseline before pruning: {error}"),
                )
                .with_project_path(path, &loaded.project_root)
            })?;
            let pruned = serialize_baseline(&comparison.pruned).map_err(|error| {
                OperationalError::new(
                    ErrorKind::Baseline,
                    format!("failed to serialize pruned baseline: {error}"),
                )
                .with_project_path(path, &loaded.project_root)
            })?;

            if current != pruned {
                write_baseline(path, &comparison.pruned).map_err(|error| {
                    OperationalError::new(
                        ErrorKind::Baseline,
                        format!("failed to prune baseline: {error}"),
                    )
                    .with_project_path(path, &loaded.project_root)
                })?;
            }
        }

        let exit_status = if comparison.status.has_active() {
            ExitStatus::Findings
        } else {
            ExitStatus::Success
        };
        let exit_status = apply_fail_on_stale(
            exit_status,
            comparison.status.has_stale(),
            cli.fail_on_stale,
        );
        let output = match output_format {
            OutputFormat::Text => render_baseline_status_text(&comparison.status),
            OutputFormat::Json => {
                render_baseline_status_json(&comparison.status).map_err(|error| {
                    OperationalError::new(
                        ErrorKind::Output,
                        format!("failed to render baseline status JSON: {error}"),
                    )
                })?
            }
            OutputFormat::Markdown | OutputFormat::Sarif => {
                unreachable!("baseline administrative output is validated before execution")
            }
        };

        return Ok(RunResult::new(output, "", exit_status));
    }

    let groups = if let Some(path) = baseline_path {
        let baseline = read_baseline(&path, normalization).map_err(|error| {
            OperationalError::new(
                ErrorKind::Baseline,
                format!("failed to load baseline: {error}"),
            )
            .with_project_path(&path, &loaded.project_root)
        })?;

        filter_active_groups(
            &corpus,
            groups,
            &baseline,
            normalization,
            &loaded.project_root,
        )
        .map_err(|error| {
            OperationalError::new(
                ErrorKind::Baseline,
                format!("failed to apply baseline: {error}"),
            )
            .with_project_path(&path, &loaded.project_root)
        })?
    } else {
        groups
    };
    let groups = focus.filter_groups(&corpus, groups, &loaded.project_root);

    let report = build_report(
        &corpus,
        &groups,
        &ReportOptions {
            show_source: cli.show_source,
            path_root: Some(loaded.project_root.clone()),
            analysis,
            complete,
            errors: preparation_errors,
        },
    )
    .map_err(|error| {
        OperationalError::new(
            ErrorKind::Internal,
            format!("failed to build report: {error}"),
        )
    })?;

    let output = match output_format {
        OutputFormat::Text => render_text(&report, text_color),
        OutputFormat::Json => render_json(&report).map_err(|error| {
            OperationalError::new(
                ErrorKind::Output,
                format!("failed to render JSON report: {error}"),
            )
        })?,
        OutputFormat::Markdown => render_markdown(&report),
        OutputFormat::Sarif => render_sarif(&report).map_err(|error| {
            OperationalError::new(
                ErrorKind::Output,
                format!("failed to render SARIF report: {error}"),
            )
        })?,
    };

    write_report_targets(&report_targets, &report, &loaded.project_root)?;

    let exit_status = apply_no_fail_on_findings(report.exit_status(), cli.no_fail_on_findings);
    Ok(RunResult::new(output, "", exit_status))
}

fn selected_baseline_path(cli: &Cli, loaded: &LoadedSettings) -> Option<PathBuf> {
    cli.baseline
        .clone()
        .or_else(|| loaded.settings.baseline.clone())
}

fn analysis_metadata(
    loaded: &LoadedSettings,
    baseline_enabled: bool,
    focus: Vec<String>,
    virtual_source: Option<String>,
    keep_going: bool,
) -> AnalysisMetadata {
    let settings = &loaded.settings;

    AnalysisMetadata {
        min_lines: settings.min_lines,
        ignore_comments: settings.ignore_comments,
        ignore_docstrings: settings.ignore_docstrings,
        ignore_imports: settings.ignore_imports,
        ignore_signatures: settings.ignore_signatures,
        same_file: settings.same_file,
        hidden: settings.hidden,
        exclude: settings.exclude.clone(),
        baseline_enabled,
        focus,
        virtual_source,
        keep_going,
    }
}

fn validate_output_options(cli: &Cli, output_format: OutputFormat) -> Result<(), OperationalError> {
    let administrative_mode = if cli.baseline_status.is_some() {
        Some("--baseline-status")
    } else if cli.prune_baseline.is_some() {
        Some("--prune-baseline")
    } else if cli.show_config {
        Some("--show-config")
    } else if cli.list_files {
        Some("--list-files")
    } else {
        None
    };

    let non_scan_mode =
        administrative_mode.or_else(|| cli.write_baseline.as_ref().map(|_| "--write-baseline"));

    if cli.fail_on_stale && cli.baseline_status.is_none() {
        return Err(OperationalError::new(
            ErrorKind::Configuration,
            "--fail-on-stale requires --suppression-status or --baseline-status",
        ));
    }

    if cli.stdin_path.is_some()
        && let Some(mode) = non_scan_mode
    {
        return Err(OperationalError::new(
            ErrorKind::Configuration,
            format!("--stdin-path is not valid with {mode}"),
        ));
    }

    if cli.keep_going
        && let Some(mode) = non_scan_mode
    {
        return Err(OperationalError::new(
            ErrorKind::Configuration,
            format!("--keep-going is not valid with {mode}"),
        ));
    }

    if !cli.focus.is_empty()
        && let Some(mode) = non_scan_mode
    {
        return Err(OperationalError::new(
            ErrorKind::Configuration,
            format!("--focus is not valid with {mode}"),
        ));
    }

    if cli.no_fail_on_findings
        && let Some(mode) = non_scan_mode
    {
        return Err(OperationalError::new(
            ErrorKind::Configuration,
            format!("--no-fail-on-findings is not valid with {mode}"),
        ));
    }

    if !cli.report.is_empty()
        && let Some(mode) = non_scan_mode
    {
        return Err(OperationalError::new(
            ErrorKind::Configuration,
            format!("--report is not valid with {mode}"),
        ));
    }

    if let Some(mode) = administrative_mode {
        if cli.show_source {
            return Err(OperationalError::new(
                ErrorKind::Configuration,
                format!("--show-source is not valid with {mode}"),
            ));
        }

        if cli.color.is_some() {
            return Err(OperationalError::new(
                ErrorKind::Configuration,
                format!("--color is not valid with {mode}"),
            ));
        }

        if matches!(output_format, OutputFormat::Markdown | OutputFormat::Sarif) {
            return Err(OperationalError::new(
                ErrorKind::Configuration,
                format!("{mode} supports only text or JSON output"),
            ));
        }
    }

    if cli.color.is_some() && output_format != OutputFormat::Text {
        return Err(OperationalError::new(
            ErrorKind::Configuration,
            "--color is only valid with text output",
        ));
    }

    Ok(())
}

fn resolve_text_color(explicit: Option<ColorWhen>, context: &RunContext) -> bool {
    if let Some(choice) = explicit {
        return match choice {
            ColorWhen::Auto => context.text_color_capable,
            ColorWhen::Always => true,
            ColorWhen::Never => false,
        };
    }

    let environment = context.color_environment;

    if environment.no_color {
        false
    } else if environment.clicolor_force {
        true
    } else if environment.clicolor_disabled {
        false
    } else {
        context.text_color_capable
    }
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
    use std::fs;
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
                std::env::temp_dir().join(format!("arid-run-test-{}-{id}", std::process::id()));
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

    fn test_cli(paths: Vec<PathBuf>) -> Cli {
        Cli {
            paths,
            config: None,
            no_config: false,
            project_root: None,
            capabilities: false,
            show_config: false,
            list_files: false,
            suppression_status: false,
            explain_path: None,
            fail_on_stale: false,
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
            no_ignore_files: false,
            exclude: Vec::new(),
            workers: 1,
            format: None,
            report: Vec::new(),
            color: None,
            json: false,
            show_source: false,
            baseline: None,
            baseline_status: None,
            prune_baseline: None,
            write_baseline: None,
        }
    }

    fn duplicate_fixture() -> (TempDir, Cli) {
        let temp = TempDir::new();
        write_test_config(&temp);
        temp.write("a.py", "alpha = 1\nbeta = 2\n");
        temp.write("b.py", "alpha = 1\nbeta = 2\n");
        let cli = test_cli(vec![temp.path().to_path_buf()]);
        (temp, cli)
    }

    #[test]
    fn capabilities_do_not_require_project_discovery() {
        let mut cli = test_cli(vec![PathBuf::from("definitely-missing-project")]);
        cli.capabilities = true;

        let first = run(&cli);
        let second = run(&cli);

        assert_eq!(first.exit_status(), ExitStatus::Success);
        assert!(first.stderr().is_empty());
        assert_eq!(first.stdout(), second.stdout());
        let value: serde_json::Value = serde_json::from_str(first.stdout()).unwrap();
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["tool_version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(value["report_schema_versions"], serde_json::json!([4]));
        assert_eq!(value["baseline_schema_versions"], serde_json::json!([1]));
        assert_eq!(value["error_schema_versions"], serde_json::json!([1]));
        assert_eq!(
            value["finding_fingerprint_versions"],
            serde_json::json!([1])
        );
        assert_eq!(
            value["formats"],
            serde_json::json!(["json", "markdown", "sarif", "text"])
        );
        assert!(
            value["features"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("multi-report"))
        );
    }

    #[test]
    fn end_to_end_scan_reports_real_duplicate() {
        let (_temp, cli) = duplicate_fixture();
        let result = run(&cli);
        assert_eq!(result.exit_status(), ExitStatus::Findings);
        assert!(result.stderr().is_empty());
        assert!(result.stdout().contains("DUP001"));
        assert!(result.stdout().contains("a.py:1-2"));
        assert!(result.stdout().contains("b.py:1-2"));
        assert!(result.stdout().contains("Found 1 duplicate group."));
        assert!(!result.stdout().contains('\u{1b}'));
    }

    #[test]
    fn virtual_source_replaces_disk_file_end_to_end() {
        let (temp, mut cli) = duplicate_fixture();
        cli.stdin_path = Some(PathBuf::from("b.py"));

        let result = run_with_context(
            &cli,
            RunContext::non_terminal().with_stdin_source("gamma = 3\ndelta = 4\n".to_owned()),
        );

        assert_eq!(result.exit_status(), ExitStatus::Success);
        assert!(result.stdout().contains("No duplicate code found."));
        assert_eq!(
            fs::read_to_string(temp.path().join("b.py")).unwrap(),
            "alpha = 1\nbeta = 2\n"
        );
    }

    #[test]
    fn virtual_source_is_added_and_ignores_gitignore() {
        let temp = TempDir::new();
        write_test_config(&temp);
        temp.write("a.py", "alpha = 1\nbeta = 2\n");
        temp.write(".gitignore", "proposed.py\n");

        let mut cli = test_cli(vec![temp.path().to_path_buf()]);
        cli.stdin_path = Some(PathBuf::from("proposed.py"));
        cli.json = true;

        let result = run_with_context(
            &cli,
            RunContext::non_terminal().with_stdin_source("alpha = 1\nbeta = 2\n".to_owned()),
        );

        assert_eq!(result.exit_status(), ExitStatus::Findings);
        assert!(!temp.path().join("proposed.py").exists());
        let value: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
        assert_eq!(value["analysis"]["virtual_source"], "proposed.py");
        assert_eq!(value["findings"][0]["locations"][1]["path"], "proposed.py");
    }

    #[test]
    fn no_config_disables_project_configuration_end_to_end() {
        let (temp, configured_cli) = duplicate_fixture();
        assert_eq!(run(&configured_cli).exit_status(), ExitStatus::Findings);

        let mut no_config_cli = test_cli(vec![temp.path().to_path_buf()]);
        no_config_cli.no_config = true;
        let result = run(&no_config_cli);

        assert_eq!(result.exit_status(), ExitStatus::Success);
        assert!(result.stdout().contains("No duplicate code found."));
    }

    #[test]
    fn show_config_reports_effective_settings() {
        let temp = TempDir::new();
        temp.write(
            "pyproject.toml",
            r#"
[tool.arid]
min-lines = 7
hidden = true
exclude = ["generated/**"]
baseline = "debt.json"
"#,
        );
        temp.write("broken.py", "def broken(:\n");

        let mut cli = test_cli(vec![temp.path().to_path_buf()]);
        cli.show_config = true;
        cli.json = true;
        cli.min_lines = Some(5);

        let result = run(&cli);
        assert_eq!(result.exit_status(), ExitStatus::Success);
        assert!(result.stderr().is_empty());

        let value: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["configuration"]["state"], "file");
        assert_eq!(value["settings"]["min_lines"], 5);
        assert_eq!(value["settings"]["hidden"], true);
        assert_eq!(value["settings"]["exclude"][0], "generated/**");
        assert_eq!(
            value["settings"]["baseline"],
            temp.path().join("debt.json").to_string_lossy().as_ref()
        );
    }

    #[test]
    fn list_files_stops_before_source_preparation() {
        let temp = TempDir::new();
        write_test_config(&temp);
        temp.write("a.py", "alpha = 1\nbeta = 2\n");
        temp.write("broken.py", "def broken(:\n");
        temp.write("notes.txt", "not Python\n");

        let mut cli = test_cli(vec![temp.path().to_path_buf()]);
        cli.list_files = true;

        let listed = run(&cli);
        assert_eq!(listed.exit_status(), ExitStatus::Success);
        assert_eq!(listed.stdout(), "a.py\nbroken.py\n");

        cli.list_files = false;
        let scanned = run(&cli);
        assert_eq!(scanned.exit_status(), ExitStatus::Error);
        assert!(scanned.stderr().contains("broken.py"));
    }

    #[test]
    fn no_ignore_files_changes_list_files_discovery() {
        let temp = TempDir::new();
        write_test_config(&temp);
        fs::create_dir_all(temp.path().join(".git")).unwrap();
        temp.write("a.py", "alpha = 1\n");
        temp.write("generated/ignored.py", "beta = 2\n");
        temp.write(".gitignore", "generated/\n");

        let mut cli = test_cli(vec![temp.path().to_path_buf()]);
        cli.list_files = true;

        let normal = run(&cli);
        assert_eq!(normal.exit_status(), ExitStatus::Success);
        assert_eq!(normal.stdout(), "a.py\n");

        cli.no_ignore_files = true;
        let overridden = run(&cli);
        assert_eq!(overridden.exit_status(), ExitStatus::Success);
        assert_eq!(overridden.stdout(), "a.py\ngenerated/ignored.py\n");
    }

    #[test]
    fn introspection_rejects_non_administrative_output_options() {
        let (_temp, mut cli) = duplicate_fixture();
        cli.show_config = true;
        cli.format = Some(OutputFormat::Markdown);
        let error = run(&cli);
        assert_eq!(error.exit_status(), ExitStatus::Error);
        assert!(error.stderr().contains("supports only text or JSON"));

        cli.show_config = false;
        cli.list_files = true;
        cli.format = Some(OutputFormat::Sarif);
        let error = run(&cli);
        assert_eq!(error.exit_status(), ExitStatus::Error);
        assert!(error.stderr().contains("supports only text or JSON"));
    }

    #[test]
    fn baseline_write_and_enforcement_are_end_to_end() {
        let (temp, mut write_cli) = duplicate_fixture();
        let baseline_path = temp.path().join("arid-baseline.json");
        write_cli.write_baseline = Some(baseline_path.clone());
        let written = run(&write_cli);
        assert_eq!(written.exit_status(), ExitStatus::Success);
        assert!(written.stdout().is_empty());
        assert!(written.stderr().is_empty());
        assert!(baseline_path.is_file());

        let mut enforce_cli = test_cli(vec![temp.path().to_path_buf()]);
        enforce_cli.baseline = Some(baseline_path.clone());
        let accepted = run(&enforce_cli);
        assert_eq!(accepted.exit_status(), ExitStatus::Success);
        assert!(accepted.stdout().contains("No duplicate code found."));

        temp.write("c.py", "alpha = 1\nbeta = 2\n");
        let active = run(&enforce_cli);
        assert_eq!(active.exit_status(), ExitStatus::Findings);
        assert!(active.stdout().contains("a.py:1-2"));
        assert!(active.stdout().contains("b.py:1-2"));
        assert!(active.stdout().contains("c.py:1-2"));
    }

    #[test]
    fn baseline_status_reports_debt_without_enforcing_it() {
        let (temp, mut write_cli) = duplicate_fixture();
        let baseline_path = temp.path().join("arid-baseline.json");
        write_cli.write_baseline = Some(baseline_path.clone());
        assert_eq!(run(&write_cli).exit_status(), ExitStatus::Success);

        let mut status_cli = test_cli(vec![temp.path().to_path_buf()]);
        status_cli.baseline_status = Some(baseline_path.clone());

        let exact = run(&status_cli);
        assert_eq!(exact.exit_status(), ExitStatus::Success);
        assert!(exact.stdout().contains("Accepted occurrences: 2"));
        assert!(exact.stdout().contains("Active occurrences: 0"));
        assert!(exact.stdout().contains("Stale occurrences: 0"));

        temp.write("c.py", "alpha = 1\nbeta = 2\n");
        let active = run(&status_cli);
        assert_eq!(active.exit_status(), ExitStatus::Findings);
        assert!(active.stdout().contains("Accepted occurrences: 2"));
        assert!(active.stdout().contains("Active occurrences: 1"));

        status_cli.json = true;
        let json = run(&status_cli);
        assert_eq!(json.exit_status(), ExitStatus::Findings);
        let value: serde_json::Value = serde_json::from_str(json.stdout()).unwrap();
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["summary"]["accepted"], 2);
        assert_eq!(value["summary"]["active"], 1);
        assert_eq!(value["summary"]["stale"], 0);
        assert_eq!(value["groups"][0]["paths"][2]["path"], "c.py");
        assert_eq!(value["groups"][0]["paths"][2]["active"], 1);

        fs::remove_file(temp.path().join("b.py")).unwrap();
        fs::remove_file(temp.path().join("c.py")).unwrap();
        status_cli.json = false;
        let stale = run(&status_cli);
        assert_eq!(stale.exit_status(), ExitStatus::Success);
        assert!(stale.stdout().contains("Accepted occurrences: 0"));
        assert!(stale.stdout().contains("Active occurrences: 0"));
        assert!(stale.stdout().contains("Stale occurrences: 2"));

        status_cli.fail_on_stale = true;
        let enforced = run(&status_cli);
        assert_eq!(enforced.exit_status(), ExitStatus::Findings);
        assert!(enforced.stdout().contains("Stale occurrences: 2"));
    }

    #[test]
    fn fail_on_stale_requires_status_mode() {
        let (_temp, mut cli) = duplicate_fixture();
        cli.fail_on_stale = true;

        let error = run(&cli);

        assert_eq!(error.exit_status(), ExitStatus::Error);
        assert!(error.stdout().is_empty());
        assert!(
            error
                .stderr()
                .contains("--fail-on-stale requires --suppression-status or --baseline-status")
        );
    }

    #[test]
    fn prune_baseline_removes_only_stale_acceptance() {
        let (temp, mut write_cli) = duplicate_fixture();
        let baseline_path = temp.path().join("arid-baseline.json");
        write_cli.write_baseline = Some(baseline_path.clone());
        assert_eq!(run(&write_cli).exit_status(), ExitStatus::Success);

        fs::rename(temp.path().join("b.py"), temp.path().join("c.py")).unwrap();

        let mut prune_cli = test_cli(vec![temp.path().to_path_buf()]);
        prune_cli.prune_baseline = Some(baseline_path.clone());
        let pruned = run(&prune_cli);
        assert_eq!(pruned.exit_status(), ExitStatus::Findings);
        assert!(pruned.stdout().contains("Accepted occurrences: 1"));
        assert!(pruned.stdout().contains("Active occurrences: 1"));
        assert!(pruned.stdout().contains("Stale occurrences: 1"));

        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&baseline_path).unwrap()).unwrap();
        assert_eq!(value["version"], 1);
        assert_eq!(value["groups"].as_array().unwrap().len(), 1);
        assert_eq!(
            value["groups"][0]["occurrences"].as_array().unwrap().len(),
            1
        );
        assert_eq!(value["groups"][0]["occurrences"][0]["path"], "a.py");
        assert_eq!(value["groups"][0]["occurrences"][0]["count"], 1);

        let baseline_bytes = fs::read(&baseline_path).unwrap();
        prune_cli.json = true;
        let unchanged = run(&prune_cli);
        assert_eq!(unchanged.exit_status(), ExitStatus::Findings);
        assert_eq!(fs::read(&baseline_path).unwrap(), baseline_bytes);
        let value: serde_json::Value = serde_json::from_str(unchanged.stdout()).unwrap();
        assert_eq!(value["summary"]["accepted"], 1);
        assert_eq!(value["summary"]["active"], 1);
        assert_eq!(value["summary"]["stale"], 0);

        fs::remove_file(temp.path().join("c.py")).unwrap();
        prune_cli.json = false;
        let stale = run(&prune_cli);
        assert_eq!(stale.exit_status(), ExitStatus::Success);
        assert!(stale.stdout().contains("Accepted occurrences: 0"));
        assert!(stale.stdout().contains("Active occurrences: 0"));
        assert!(stale.stdout().contains("Stale occurrences: 1"));

        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&baseline_path).unwrap()).unwrap();
        assert!(value["groups"].as_array().unwrap().is_empty());
    }

    #[test]
    fn baseline_status_rejects_non_administrative_output_options() {
        let (_temp, mut cli) = duplicate_fixture();
        cli.baseline_status = Some(PathBuf::from("baseline.json"));
        cli.format = Some(OutputFormat::Markdown);
        let error = run(&cli);
        assert_eq!(error.exit_status(), ExitStatus::Error);
        assert!(error.stderr().contains("supports only text or JSON"));
    }

    #[test]
    fn prune_baseline_rejects_non_administrative_output_options() {
        let (_temp, mut cli) = duplicate_fixture();
        cli.prune_baseline = Some(PathBuf::from("baseline.json"));
        cli.format = Some(OutputFormat::Sarif);
        let error = run(&cli);
        assert_eq!(error.exit_status(), ExitStatus::Error);
        assert!(error.stderr().contains("supports only text or JSON"));
    }

    #[test]
    fn keep_going_rejects_administrative_modes_through_rust_api() {
        let (_temp, mut cli) = duplicate_fixture();
        cli.keep_going = true;
        cli.write_baseline = Some(PathBuf::from("baseline.json"));

        let error = run(&cli);

        assert_eq!(error.exit_status(), ExitStatus::Error);
        assert!(
            error
                .stderr()
                .contains("--keep-going is not valid with --write-baseline")
        );
    }

    #[test]
    fn configured_baseline_is_used_and_cli_baseline_wins() {
        let (temp, mut write_cli) = duplicate_fixture();
        let configured_path = temp.path().join("configured.json");
        write_cli.write_baseline = Some(configured_path.clone());
        assert_eq!(run(&write_cli).exit_status(), ExitStatus::Success);
        temp.write(
            "pyproject.toml",
            r#"
[tool.arid]
min-lines = 2
baseline = "configured.json"
"#,
        );
        let configured = run(&test_cli(vec![temp.path().to_path_buf()]));
        assert_eq!(configured.exit_status(), ExitStatus::Success);
        let mut cli = test_cli(vec![temp.path().to_path_buf()]);
        cli.baseline = Some(temp.path().join("missing.json"));
        let error = run(&cli);
        assert_eq!(error.exit_status(), ExitStatus::Error);
        assert!(error.stdout().is_empty());
        assert!(error.stderr().contains("missing.json"));
    }

    #[test]
    fn baseline_normalization_mismatch_is_rejected() {
        let (temp, mut write_cli) = duplicate_fixture();
        let baseline_path = temp.path().join("arid-baseline.json");
        write_cli.write_baseline = Some(baseline_path.clone());
        assert_eq!(run(&write_cli).exit_status(), ExitStatus::Success);
        let mut cli = test_cli(vec![temp.path().to_path_buf()]);
        cli.baseline = Some(baseline_path);
        cli.no_ignore_comments = true;
        let error = run(&cli);
        assert_eq!(error.exit_status(), ExitStatus::Error);
        assert!(
            error
                .stderr()
                .contains("normalization settings do not match")
        );
    }

    #[test]
    fn cli_min_lines_overrides_project_config() {
        let temp = TempDir::new();
        write_test_config(&temp);
        temp.write("a.py", "alpha = 1\nbeta = 2\n");
        temp.write("b.py", "alpha = 1\nbeta = 2\n");
        let mut cli = test_cli(vec![temp.path().to_path_buf()]);
        cli.min_lines = Some(3);
        let result = run(&cli);
        assert_eq!(result.exit_status(), ExitStatus::Success);
        assert_eq!(
            result.stdout(),
            concat!("No duplicate code found.\n", "0 duplicate lines (0.00%).\n")
        );
    }

    #[test]
    fn end_to_end_scan_succeeds_without_duplicates() {
        let temp = TempDir::new();
        write_test_config(&temp);
        temp.write("a.py", "alpha = 1\nbeta = 2\n");
        temp.write("b.py", "gamma = 3\ndelta = 4\n");
        let result = run(&test_cli(vec![temp.path().to_path_buf()]));
        assert_eq!(result.exit_status(), ExitStatus::Success);
        assert_eq!(
            result.stdout(),
            concat!("No duplicate code found.\n", "0 duplicate lines (0.00%).\n")
        );
    }

    #[test]
    fn end_to_end_scan_can_emit_json() {
        let (_temp, mut cli) = duplicate_fixture();
        cli.json = true;
        let result = run(&cli);
        assert_eq!(result.exit_status(), ExitStatus::Findings);
        let value: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
        assert_eq!(value["schema_version"], 4);
        assert!(value.get("version").is_none());
        assert_eq!(value["tool_version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(value["analysis"]["min_lines"], 2);
        assert_eq!(value["duplicate_groups"], 1);
        assert_eq!(value["findings"][0]["code"], "DUP001");
        assert!(
            value["findings"][0]["fingerprint"]
                .as_str()
                .unwrap()
                .starts_with("arid-finding-v1:sha256:")
        );
    }

    #[test]
    fn keep_going_reports_partial_json_with_valid_findings() {
        let temp = TempDir::new();
        write_test_config(&temp);
        temp.write("a.py", "alpha = 1\nbeta = 2\n");
        temp.write("b.py", "alpha = 1\nbeta = 2\n");
        temp.write("broken.py", "def broken(:\n");

        let mut cli = test_cli(vec![temp.path().to_path_buf()]);
        cli.keep_going = true;
        cli.json = true;

        let result = run(&cli);

        assert_eq!(result.exit_status(), ExitStatus::Error);
        assert!(result.stderr().is_empty());
        let value: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
        assert_eq!(value["complete"], false);
        assert_eq!(value["analysis"]["keep_going"], true);
        assert_eq!(value["files"], 2);
        assert_eq!(value["duplicate_groups"], 1);
        assert_eq!(value["errors"].as_array().unwrap().len(), 1);
        assert_eq!(value["errors"][0]["kind"], "parse");
        assert_eq!(value["errors"][0]["path"], "broken.py");
        assert_eq!(value["findings"][0]["code"], "DUP001");
    }

    #[test]
    fn keep_going_without_source_errors_produces_complete_report() {
        let (_temp, mut cli) = duplicate_fixture();
        cli.keep_going = true;
        cli.json = true;

        let result = run(&cli);

        assert_eq!(result.exit_status(), ExitStatus::Findings);
        let value: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
        assert_eq!(value["complete"], true);
        assert_eq!(value["analysis"]["keep_going"], true);
        assert!(value["errors"].as_array().unwrap().is_empty());
        assert_eq!(value["duplicate_groups"], 1);
    }

    #[test]
    fn format_json_matches_json_flag() {
        let (_temp, cli) = duplicate_fixture();
        let mut via_flag = test_cli(cli.paths.clone());
        via_flag.json = true;
        let mut via_format = test_cli(cli.paths);
        via_format.format = Some(OutputFormat::Json);
        assert_eq!(run(&via_format), run(&via_flag));
    }

    #[test]
    fn end_to_end_scan_can_emit_markdown() {
        let (_temp, mut cli) = duplicate_fixture();
        cli.format = Some(OutputFormat::Markdown);
        cli.show_source = true;
        let result = run(&cli);
        assert_eq!(result.exit_status(), ExitStatus::Findings);
        assert!(
            result
                .stdout()
                .starts_with("# Arid duplicate-code report\n\n")
        );
        assert!(result.stdout().contains("## `DUP001` — 2 duplicated lines"));
        assert!(result.stdout().contains("### `a.py:1-2`"));
        assert!(
            result
                .stdout()
                .contains("```python\nalpha = 1\nbeta = 2\n```")
        );
        assert!(result.stdout().contains("- **Duplicate groups:** 1"));
        assert!(!result.stdout().contains('\u{1b}'));
    }

    #[test]
    fn end_to_end_scan_can_emit_sarif() {
        let (_temp, mut cli) = duplicate_fixture();
        cli.format = Some(OutputFormat::Sarif);
        cli.show_source = true;
        let result = run(&cli);
        assert_eq!(result.exit_status(), ExitStatus::Findings);
        assert!(!result.stdout().contains('\u{1b}'));
        let value: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
        assert_eq!(value["version"], "2.1.0");
        assert_eq!(value["runs"][0]["tool"]["driver"]["name"], "Arid");
        assert_eq!(value["runs"][0]["results"][0]["ruleId"], "DUP001");
        assert_eq!(
            value["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]
                ["uri"],
            "a.py"
        );
    }

    #[test]
    fn explicit_color_overrides_environment() {
        let (_temp, mut cli) = duplicate_fixture();
        let hostile_environment = ColorEnvironment {
            no_color: true,
            clicolor_force: true,
            clicolor_disabled: true,
        };
        cli.color = Some(ColorWhen::Always);
        let result = run_with_context(&cli, RunContext::new(false, hostile_environment));
        assert!(result.stdout().contains('\u{1b}'));
        cli.color = Some(ColorWhen::Never);
        let result = run_with_context(&cli, RunContext::new(true, hostile_environment));
        assert!(!result.stdout().contains('\u{1b}'));
        cli.color = Some(ColorWhen::Auto);
        let result = run_with_context(&cli, RunContext::new(false, hostile_environment));
        assert!(!result.stdout().contains('\u{1b}'));
    }

    #[test]
    fn color_environment_precedence_is_deterministic() {
        let terminal = |environment| RunContext::new(true, environment);
        let redirected = |environment| RunContext::new(false, environment);
        assert!(!resolve_text_color(
            None,
            &terminal(ColorEnvironment {
                no_color: true,
                clicolor_force: true,
                clicolor_disabled: false
            })
        ));
        assert!(resolve_text_color(
            None,
            &redirected(ColorEnvironment {
                no_color: false,
                clicolor_force: true,
                clicolor_disabled: true
            })
        ));
        assert!(!resolve_text_color(
            None,
            &terminal(ColorEnvironment {
                no_color: false,
                clicolor_force: false,
                clicolor_disabled: true
            })
        ));
        assert!(resolve_text_color(
            None,
            &terminal(ColorEnvironment::default())
        ));
        assert!(!resolve_text_color(
            None,
            &redirected(ColorEnvironment::default())
        ));
    }

    #[test]
    fn json_never_contains_terminal_color() {
        let (_temp, mut cli) = duplicate_fixture();
        cli.json = true;
        let result = run_with_context(
            &cli,
            RunContext::new(
                true,
                ColorEnvironment {
                    no_color: false,
                    clicolor_force: true,
                    clicolor_disabled: false,
                },
            ),
        );
        assert!(!result.stdout().contains('\u{1b}'));
        serde_json::from_str::<serde_json::Value>(result.stdout()).unwrap();
    }

    #[test]
    fn color_is_rejected_for_non_text_output() {
        let (_temp, mut cli) = duplicate_fixture();
        cli.json = true;
        cli.color = Some(ColorWhen::Always);
        let result = run(&cli);
        assert_eq!(result.exit_status(), ExitStatus::Error);
        assert!(result.stderr().is_empty());
        let value: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["error"]["kind"], "configuration");
        assert_eq!(
            value["error"]["message"],
            "--color is only valid with text output"
        );
    }

    #[test]
    fn parse_failure_aborts_entire_scan() {
        let temp = TempDir::new();
        write_test_config(&temp);
        temp.write("a.py", "alpha = 1\nbeta = 2\n");
        temp.write("b.py", "alpha = 1\nbeta = 2\n");
        temp.write("broken.py", "def broken(:\n");
        for workers in [1, 4] {
            let mut cli = test_cli(vec![temp.path().to_path_buf()]);
            cli.workers = workers;
            let error = run(&cli);
            assert_eq!(error.exit_status(), ExitStatus::Error);
            assert!(error.stderr().contains("broken.py"));
            assert!(error.stderr().contains("invalid Python syntax"));
        }
    }

    #[test]
    fn worker_counts_produce_identical_json() {
        let temp = TempDir::new();
        write_test_config(&temp);
        for index in 0..8 {
            temp.write(&format!("file_{index}.py"), "alpha = 1\nbeta = 2\n");
        }
        let mut serial = test_cli(vec![temp.path().to_path_buf()]);
        serial.json = true;
        let expected = run(&serial);
        for workers in [2, 4, 8] {
            let mut parallel = test_cli(vec![temp.path().to_path_buf()]);
            parallel.json = true;
            parallel.workers = workers;
            let actual = run(&parallel);
            assert_eq!(actual.exit_status(), expected.exit_status());
            assert_eq!(actual.stdout(), expected.stdout());
            assert_eq!(actual.stderr(), expected.stderr());
        }
    }

    #[test]
    fn run_rejects_zero_workers() {
        let temp = TempDir::new();
        write_test_config(&temp);
        temp.write("example.py", "alpha = 1\nbeta = 2\n");
        let mut cli = test_cli(vec![temp.path().to_path_buf()]);
        cli.workers = 0;
        let error = run(&cli);
        assert_eq!(error.exit_status(), ExitStatus::Error);
        assert_eq!(error.stderr(), "error: worker count must be at least 1\n");
    }

    #[test]
    fn cli_boolean_flags_produce_settings_overrides() {
        let mut cli = test_cli(Vec::new());
        cli.ignore_comments = true;
        cli.no_ignore_docstrings = true;
        cli.ignore_imports = true;
        cli.no_ignore_signatures = true;
        cli.no_same_file = true;
        cli.hidden = true;
        let overrides = settings_overrides(&cli);
        assert_eq!(overrides.ignore_comments, Some(true));
        assert_eq!(overrides.ignore_docstrings, Some(false));
        assert_eq!(overrides.ignore_imports, Some(true));
        assert_eq!(overrides.ignore_signatures, Some(false));
        assert_eq!(overrides.same_file, Some(false));
        assert_eq!(overrides.hidden, Some(true));
    }

    #[test]
    fn absent_cli_flags_do_not_override_project_settings() {
        let cli = test_cli(Vec::new());
        assert_eq!(settings_overrides(&cli), SettingsOverrides::default());
    }

    #[test]
    fn cli_excludes_replace_project_excludes() {
        let mut cli = test_cli(Vec::new());
        cli.exclude = vec!["generated/**".to_owned(), "vendor/**".to_owned()];
        assert_eq!(
            settings_overrides(&cli).exclude,
            Some(vec!["generated/**".to_owned(), "vendor/**".to_owned()])
        );
    }
}
