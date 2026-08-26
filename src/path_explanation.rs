use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::cli::{Cli, OutputFormat};
use crate::config::{ProjectOptions, Settings, SettingsOverrides, load_settings_with_options};
use crate::error::{ErrorKind, OperationalError, render_error_json};
use crate::files::{DiscoveryPolicy, configured_walk_builder, is_excluded_path, is_python_file};
use crate::outcome::{ExitStatus, RunResult};
use crate::project_path::project_relative_path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum PathKind {
    File,
    Directory,
}

impl PathKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Directory => "directory",
        }
    }

    const fn is_dir(self) -> bool {
        matches!(self, Self::Directory)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum PathDecision {
    Include,
    Exclude,
}

impl PathDecision {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Include => "include",
            Self::Exclude => "exclude",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum PathReason {
    ExplicitFile,
    ExplicitDirectory,
    Discovered,
    OutsideScanRoots,
    AridExclude,
    IgnoreFile,
    Hidden,
    UnsupportedSourceType,
    SymlinkDirectory,
    SymlinkTraversal,
}

impl PathReason {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ExplicitFile => "explicit-file",
            Self::ExplicitDirectory => "explicit-directory",
            Self::Discovered => "discovered",
            Self::OutsideScanRoots => "outside-scan-roots",
            Self::AridExclude => "arid-exclude",
            Self::IgnoreFile => "ignore-file",
            Self::Hidden => "hidden",
            Self::UnsupportedSourceType => "unsupported-source-type",
            Self::SymlinkDirectory => "symlink-directory",
            Self::SymlinkTraversal => "symlink-traversal",
        }
    }

    const fn priority(self) -> u8 {
        match self {
            Self::OutsideScanRoots => 0,
            Self::SymlinkDirectory | Self::SymlinkTraversal => 1,
            Self::AridExclude => 2,
            Self::Hidden => 3,
            Self::IgnoreFile => 4,
            Self::UnsupportedSourceType => 5,
            Self::ExplicitFile | Self::ExplicitDirectory | Self::Discovered => 6,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct PathExplanation {
    path: String,
    kind: PathKind,
    symlink: bool,
    explicit: bool,
    decision: PathDecision,
    reasons: Vec<PathReason>,
}

#[derive(Serialize)]
struct PathExplanationDocument<'a> {
    schema_version: u32,
    tool_version: &'static str,
    #[serde(flatten)]
    explanation: &'a PathExplanation,
}

#[derive(Debug)]
struct InspectedPath {
    path: PathBuf,
    kind: Option<PathKind>,
    symlink: bool,
}

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

    let target = cli
        .explain_path
        .as_deref()
        .expect("path explanation routing requires --explain-path");
    let paths = scan_paths(cli);
    let loaded =
        load_settings_with_options(&paths[0], settings_overrides(cli), project_options(cli))
            .map_err(|error| {
                OperationalError::new(
                    ErrorKind::Configuration,
                    format!("failed to load configuration: {error}"),
                )
            })?;
    let policy = DiscoveryPolicy::new(!cli.no_ignore_files);
    let explanation = explain_path(
        &paths,
        !cli.paths.is_empty(),
        target,
        &loaded.settings,
        &loaded.project_root,
        policy,
    )?;

    let output = match cli.output_format() {
        OutputFormat::Text => render_path_explanation_text(&explanation),
        OutputFormat::Json => render_path_explanation_json(&explanation).map_err(|error| {
            OperationalError::new(
                ErrorKind::Output,
                format!("failed to render path explanation JSON: {error}"),
            )
        })?,
        OutputFormat::Markdown | OutputFormat::Sarif => {
            unreachable!("path explanation output is validated before execution")
        }
    };

    Ok(RunResult::new(output, "", ExitStatus::Success))
}

fn validate_options(cli: &Cli) -> Result<(), OperationalError> {
    let invalid = [
        (cli.capabilities, "--capabilities"),
        (cli.show_config, "--show-config"),
        (cli.list_files, "--list-files"),
        (cli.suppression_status, "--suppression-status"),
        (cli.fail_on_stale, "--fail-on-stale"),
        (cli.stdin_path.is_some(), "--stdin-path"),
        (cli.keep_going, "--keep-going"),
        (!cli.focus.is_empty(), "--focus"),
        (cli.no_fail_on_findings, "--no-fail-on-findings"),
        (cli.baseline.is_some(), "--baseline"),
        (cli.baseline_status.is_some(), "--baseline-status"),
        (cli.prune_baseline.is_some(), "--prune-baseline"),
        (cli.write_baseline.is_some(), "--write-baseline"),
        (!cli.report.is_empty(), "--report"),
        (cli.show_source, "--show-source"),
        (cli.color.is_some(), "--color"),
    ];

    if let Some((_, option)) = invalid.into_iter().find(|(present, _)| *present) {
        return Err(OperationalError::new(
            ErrorKind::Configuration,
            format!("{option} is not valid with --explain-path"),
        ));
    }

    if matches!(
        cli.output_format(),
        OutputFormat::Markdown | OutputFormat::Sarif
    ) {
        return Err(OperationalError::new(
            ErrorKind::Configuration,
            "--explain-path supports only text or JSON output",
        ));
    }

    Ok(())
}

fn explain_path(
    scan_paths: &[PathBuf],
    roots_are_explicit: bool,
    target: &Path,
    settings: &Settings,
    project_root: &Path,
    policy: DiscoveryPolicy,
) -> Result<PathExplanation, OperationalError> {
    let target = inspect_path(target, true)?;
    let kind = target
        .kind
        .expect("target inspection requires a file or directory kind");
    let display_path = display_path(&target.path, project_root);

    let roots = scan_paths
        .iter()
        .map(|path| inspect_path(path, false))
        .collect::<Result<Vec<_>, _>>()?;

    if let Some(root) = roots.iter().find(|root| root.path == target.path) {
        return explain_root(
            root,
            roots_are_explicit,
            &target,
            kind,
            display_path,
            settings,
            project_root,
        );
    }

    let candidate_roots = roots
        .iter()
        .filter(|root| {
            root.kind == Some(PathKind::Directory) && target.path.starts_with(&root.path)
        })
        .collect::<Vec<_>>();

    if candidate_roots.is_empty() {
        return Ok(PathExplanation {
            path: display_path,
            kind,
            symlink: target.symlink,
            explicit: false,
            decision: PathDecision::Exclude,
            reasons: vec![PathReason::OutsideScanRoots],
        });
    }

    let mut exclusion_reasons = Vec::new();

    for root in candidate_roots {
        let reasons = traversal_exclusion_reasons(
            &root.path,
            &target,
            kind,
            settings,
            project_root,
            policy,
        )?;

        if reasons.is_empty() {
            return Ok(PathExplanation {
                path: display_path,
                kind,
                symlink: target.symlink,
                explicit: false,
                decision: PathDecision::Include,
                reasons: vec![PathReason::Discovered],
            });
        }

        for reason in reasons {
            if !exclusion_reasons.contains(&reason) {
                exclusion_reasons.push(reason);
            }
        }
    }

    exclusion_reasons.sort_by_key(|reason| reason.priority());

    Ok(PathExplanation {
        path: display_path,
        kind,
        symlink: target.symlink,
        explicit: false,
        decision: PathDecision::Exclude,
        reasons: exclusion_reasons,
    })
}

fn explain_root(
    root: &InspectedPath,
    explicit: bool,
    target: &InspectedPath,
    kind: PathKind,
    display_path: String,
    settings: &Settings,
    project_root: &Path,
) -> Result<PathExplanation, OperationalError> {
    match kind {
        PathKind::Directory => {
            if target.symlink {
                return Ok(PathExplanation {
                    path: display_path,
                    kind,
                    symlink: true,
                    explicit,
                    decision: PathDecision::Exclude,
                    reasons: vec![PathReason::SymlinkDirectory],
                });
            }

            Ok(PathExplanation {
                path: display_path,
                kind,
                symlink: false,
                explicit,
                decision: PathDecision::Include,
                reasons: vec![if explicit {
                    PathReason::ExplicitDirectory
                } else {
                    PathReason::Discovered
                }],
            })
        }
        PathKind::File => {
            let mut reasons = Vec::new();

            if is_excluded_path(&root.path, settings, project_root).map_err(|error| {
                OperationalError::new(
                    ErrorKind::Discovery,
                    format!("failed to evaluate Arid excludes: {error}"),
                )
            })? {
                reasons.push(PathReason::AridExclude);
            }

            if !is_python_file(&root.path) {
                reasons.push(PathReason::UnsupportedSourceType);
            }

            if reasons.is_empty() {
                Ok(PathExplanation {
                    path: display_path,
                    kind,
                    symlink: target.symlink,
                    explicit,
                    decision: PathDecision::Include,
                    reasons: vec![if explicit {
                        PathReason::ExplicitFile
                    } else {
                        PathReason::Discovered
                    }],
                })
            } else {
                reasons.sort_by_key(|reason| reason.priority());
                Ok(PathExplanation {
                    path: display_path,
                    kind,
                    symlink: target.symlink,
                    explicit,
                    decision: PathDecision::Exclude,
                    reasons,
                })
            }
        }
    }
}

fn traversal_exclusion_reasons(
    root: &Path,
    target: &InspectedPath,
    kind: PathKind,
    settings: &Settings,
    project_root: &Path,
    policy: DiscoveryPolicy,
) -> Result<Vec<PathReason>, OperationalError> {
    if let Some(reason) = traversal_symlink_reason(root, &target.path, kind)? {
        return Ok(vec![reason]);
    }

    let mut reasons = Vec::new();

    if is_excluded_path(&target.path, settings, project_root).map_err(|error| {
        OperationalError::new(
            ErrorKind::Discovery,
            format!("failed to evaluate Arid excludes: {error}"),
        )
    })? {
        reasons.push(PathReason::AridExclude);
    }

    let actual_ignored = incremental_ignored(root, &target.path, kind.is_dir(), settings.hidden, policy)?;

    if actual_ignored {
        let hidden_ignored = incremental_ignored(
            root,
            &target.path,
            kind.is_dir(),
            settings.hidden,
            DiscoveryPolicy::new(false),
        )?;
        let ignore_file_ignored = policy.ignore_files()
            && incremental_ignored(root, &target.path, kind.is_dir(), true, policy)?;

        if hidden_ignored {
            reasons.push(PathReason::Hidden);
        }
        if ignore_file_ignored {
            reasons.push(PathReason::IgnoreFile);
        }

        if !hidden_ignored && !ignore_file_ignored {
            return Err(OperationalError::new(
                ErrorKind::Internal,
                "targeted discovery matcher excluded a path without a supported reason",
            ));
        }
    }

    if kind == PathKind::File && !is_python_file(&target.path) {
        reasons.push(PathReason::UnsupportedSourceType);
    }

    reasons.sort_by_key(|reason| reason.priority());
    reasons.dedup();
    Ok(reasons)
}

fn traversal_symlink_reason(
    root: &Path,
    target: &Path,
    target_kind: PathKind,
) -> Result<Option<PathReason>, OperationalError> {
    let root_metadata = fs::symlink_metadata(root).map_err(|error| metadata_error(root, error))?;
    if root_metadata.file_type().is_symlink() {
        return Ok(Some(PathReason::SymlinkTraversal));
    }

    let relative = target.strip_prefix(root).map_err(|_| {
        OperationalError::new(
            ErrorKind::Internal,
            "targeted discovery root does not contain target",
        )
    })?;
    let mut current = root.to_path_buf();

    for component in relative.components() {
        current.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&current).map_err(|error| metadata_error(&current, error))?;
        if metadata.file_type().is_symlink() {
            return Ok(Some(if current == target && target_kind == PathKind::Directory {
                PathReason::SymlinkDirectory
            } else {
                PathReason::SymlinkTraversal
            }));
        }
    }

    Ok(None)
}

fn incremental_ignored(
    root: &Path,
    target: &Path,
    is_dir: bool,
    include_hidden: bool,
    policy: DiscoveryPolicy,
) -> Result<bool, OperationalError> {
    let builder = configured_walk_builder(root, include_hidden, policy);
    let mut matcher = builder
        .build_matchers()
        .into_iter()
        .next()
        .expect("configured walk builder always has one root");
    let relative = matcher.normalize(target).ok_or_else(|| {
        OperationalError::new(
            ErrorKind::Internal,
            "targeted discovery matcher could not normalize a contained path",
        )
    })?;
    let (matched, error) = matcher.matched_with_errors(relative, is_dir);

    if let Some(error) = error {
        return Err(OperationalError::new(
            ErrorKind::Discovery,
            format!("failed to evaluate ignore files: {error}"),
        ));
    }

    Ok(matched.is_ignore())
}

fn inspect_path(path: &Path, require_kind: bool) -> Result<InspectedPath, OperationalError> {
    let path = std::path::absolute(path).map_err(|error| {
        OperationalError::new(
            ErrorKind::Discovery,
            format!("failed to resolve path {}: {error}", path.display()),
        )
    })?;
    let symlink_metadata = fs::symlink_metadata(&path).map_err(|error| metadata_error(&path, error))?;
    let symlink = symlink_metadata.file_type().is_symlink();
    let metadata = if symlink {
        fs::metadata(&path).map_err(|error| metadata_error(&path, error))?
    } else {
        symlink_metadata
    };
    let kind = if metadata.is_file() {
        Some(PathKind::File)
    } else if metadata.is_dir() {
        Some(PathKind::Directory)
    } else {
        None
    };

    if require_kind && kind.is_none() {
        return Err(OperationalError::new(
            ErrorKind::Discovery,
            format!("path is neither a file nor directory: {}", path.display()),
        ));
    }

    Ok(InspectedPath {
        path,
        kind,
        symlink,
    })
}

fn metadata_error(path: &Path, error: std::io::Error) -> OperationalError {
    let message = if error.kind() == std::io::ErrorKind::NotFound {
        format!("input path does not exist: {}", path.display())
    } else {
        format!("failed to inspect path {}: {error}", path.display())
    };
    OperationalError::new(ErrorKind::Discovery, message)
}

fn display_path(path: &Path, project_root: &Path) -> String {
    project_relative_path(path, project_root)
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

fn render_path_explanation_text(explanation: &PathExplanation) -> String {
    let mut output = String::new();
    writeln!(output, "Path explanation").unwrap();
    writeln!(output, "Path: {}", explanation.path).unwrap();
    writeln!(output, "Kind: {}", explanation.kind.as_str()).unwrap();
    writeln!(output, "Symlink: {}", explanation.symlink).unwrap();
    writeln!(output, "Explicit: {}", explanation.explicit).unwrap();
    writeln!(output, "Decision: {}", explanation.decision.as_str()).unwrap();
    writeln!(
        output,
        "Reasons: {}",
        explanation
            .reasons
            .iter()
            .map(|reason| reason.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    )
    .unwrap();
    output
}

fn render_path_explanation_json(
    explanation: &PathExplanation,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&PathExplanationDocument {
        schema_version: 1,
        tool_version: env!("CARGO_PKG_VERSION"),
        explanation,
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
                "arid-path-explanation-test-{}-{id}",
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

    fn cli(temp: &TempDir, target: &Path) -> Cli {
        Cli::try_parse_from([
            "arid".into(),
            "--project-root".into(),
            temp.path().as_os_str().to_owned(),
            "--explain-path".into(),
            target.as_os_str().to_owned(),
            temp.path().as_os_str().to_owned(),
        ])
        .unwrap()
    }

    #[test]
    fn ignored_descendant_is_explained_without_parsing_source() {
        let temp = TempDir::new();
        fs::create_dir_all(temp.path().join(".git")).unwrap();
        let target = temp.write("generated/broken.py", "def broken(:\n");
        temp.write(".gitignore", "generated/\n");

        let mut cli = cli(&temp, &target);
        cli.json = true;
        let result = run(&cli);

        assert_eq!(result.exit_status(), ExitStatus::Success);
        let value: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
        assert_eq!(value["path"], "generated/broken.py");
        assert_eq!(value["kind"], "file");
        assert_eq!(value["decision"], "exclude");
        assert_eq!(value["reasons"], serde_json::json!(["ignore-file"]));
    }

    #[test]
    fn no_ignore_files_changes_targeted_decision() {
        let temp = TempDir::new();
        fs::create_dir_all(temp.path().join(".git")).unwrap();
        let target = temp.write("generated/example.py", "pass\n");
        temp.write(".gitignore", "generated/\n");

        let mut cli = cli(&temp, &target);
        cli.no_ignore_files = true;
        cli.json = true;
        let result = run(&cli);

        assert_eq!(result.exit_status(), ExitStatus::Success);
        let value: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
        assert_eq!(value["decision"], "include");
        assert_eq!(value["reasons"], serde_json::json!(["discovered"]));
    }

    #[test]
    fn explicit_file_bypasses_ignore_and_hidden_rules() {
        let temp = TempDir::new();
        fs::create_dir_all(temp.path().join(".git")).unwrap();
        let target = temp.write(".generated/example.py", "pass\n");
        temp.write(".gitignore", ".generated/\n");

        let mut cli = Cli::try_parse_from([
            "arid".into(),
            "--project-root".into(),
            temp.path().as_os_str().to_owned(),
            "--explain-path".into(),
            target.as_os_str().to_owned(),
            target.as_os_str().to_owned(),
        ])
        .unwrap();
        cli.json = true;
        let result = run(&cli);

        let value: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
        assert_eq!(value["explicit"], true);
        assert_eq!(value["decision"], "include");
        assert_eq!(value["reasons"], serde_json::json!(["explicit-file"]));
    }

    #[test]
    fn arid_exclude_remains_distinct_from_ignore_files() {
        let temp = TempDir::new();
        let target = temp.write("generated/example.py", "pass\n");
        temp.write(
            "pyproject.toml",
            "[tool.arid]\nexclude = [\"generated/**\"]\n",
        );

        let mut cli = cli(&temp, &target);
        cli.no_ignore_files = true;
        cli.json = true;
        let result = run(&cli);

        let value: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
        assert_eq!(value["decision"], "exclude");
        assert_eq!(value["reasons"], serde_json::json!(["arid-exclude"]));
    }

    #[test]
    fn hidden_and_unsupported_source_reasons_are_ordered() {
        let temp = TempDir::new();
        let target = temp.write(".hidden/notes.txt", "not python\n");

        let mut cli = cli(&temp, &target);
        cli.json = true;
        let result = run(&cli);

        let value: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
        assert_eq!(value["decision"], "exclude");
        assert_eq!(
            value["reasons"],
            serde_json::json!(["hidden", "unsupported-source-type"])
        );
    }

    #[test]
    fn target_outside_scan_roots_is_excluded() {
        let temp = TempDir::new();
        let scan_root = temp.path().join("src");
        fs::create_dir_all(&scan_root).unwrap();
        let target = temp.write("elsewhere/example.py", "pass\n");

        let mut cli = Cli::try_parse_from([
            "arid".into(),
            "--project-root".into(),
            temp.path().as_os_str().to_owned(),
            "--explain-path".into(),
            target.as_os_str().to_owned(),
            scan_root.as_os_str().to_owned(),
        ])
        .unwrap();
        cli.json = true;
        let result = run(&cli);

        let value: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
        assert_eq!(value["decision"], "exclude");
        assert_eq!(
            value["reasons"],
            serde_json::json!(["outside-scan-roots"])
        );
    }

    #[test]
    fn explicit_directory_root_is_included_without_enumerating_descendants() {
        let temp = TempDir::new();
        let target = temp.path().join("generated");
        fs::create_dir_all(&target).unwrap();
        temp.write("generated/broken.py", "def broken(:\n");
        temp.write(".ignore", "generated/\n");

        let mut cli = Cli::try_parse_from([
            "arid".into(),
            "--project-root".into(),
            temp.path().as_os_str().to_owned(),
            "--explain-path".into(),
            target.as_os_str().to_owned(),
            target.as_os_str().to_owned(),
        ])
        .unwrap();
        cli.json = true;
        let result = run(&cli);

        let value: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
        assert_eq!(value["kind"], "directory");
        assert_eq!(value["explicit"], true);
        assert_eq!(value["decision"], "include");
        assert_eq!(
            value["reasons"],
            serde_json::json!(["explicit-directory"])
        );
    }

    #[test]
    fn missing_target_is_discovery_error() {
        let temp = TempDir::new();
        let target = temp.path().join("missing.py");
        let cli = cli(&temp, &target);
        let result = run(&cli);

        assert_eq!(result.exit_status(), ExitStatus::Error);
        assert!(result.stdout().is_empty());
        assert!(result.stderr().contains("input path does not exist"));
    }

    #[test]
    fn unicode_and_space_paths_are_deterministic() {
        let temp = TempDir::new();
        let target = temp.write("src/π file.py", "pass\n");
        let mut cli = cli(&temp, &target);
        cli.json = true;

        let first = run(&cli);
        let second = run(&cli);

        assert_eq!(first.stdout(), second.stdout());
        let value: serde_json::Value = serde_json::from_str(first.stdout()).unwrap();
        assert_eq!(value["path"], "src/π file.py");
        assert_eq!(value["decision"], "include");
    }

    #[cfg(unix)]
    #[test]
    fn traversal_through_symlink_is_excluded() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new();
        let real = temp.path().join("real");
        fs::create_dir_all(&real).unwrap();
        temp.write("real/example.py", "pass\n");
        let link = temp.path().join("linked");
        symlink(&real, &link).unwrap();
        let target = link.join("example.py");

        let mut cli = cli(&temp, &target);
        cli.json = true;
        let result = run(&cli);

        let value: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
        assert_eq!(value["decision"], "exclude");
        assert_eq!(
            value["reasons"],
            serde_json::json!(["symlink-traversal"])
        );
    }
}
