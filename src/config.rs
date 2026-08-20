use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

use crate::detect::DetectionOptions;
use crate::model::NormalizationOptions;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub min_lines: u32,
    pub ignore_comments: bool,
    pub ignore_docstrings: bool,
    pub ignore_imports: bool,
    pub ignore_signatures: bool,
    pub same_file: bool,
    pub hidden: bool,
    pub exclude: Vec<String>,
    pub baseline: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            min_lines: 4,
            ignore_comments: true,
            ignore_docstrings: true,
            ignore_imports: true,
            ignore_signatures: true,
            same_file: true,
            hidden: false,
            exclude: Vec::new(),
            baseline: None,
        }
    }
}

impl Settings {
    #[must_use]
    pub fn normalization_options(&self) -> NormalizationOptions {
        NormalizationOptions {
            ignore_comments: self.ignore_comments,
            ignore_docstrings: self.ignore_docstrings,
            ignore_imports: self.ignore_imports,
            ignore_signatures: self.ignore_signatures,
        }
    }

    #[must_use]
    pub fn detection_options(&self) -> DetectionOptions {
        DetectionOptions {
            min_lines: self.min_lines,
            same_file: self.same_file,
        }
    }
}

/// Optional higher-precedence settings.
///
/// Populated by higher-precedence CLI arguments.
/// `Some` replaces the corresponding project setting; `None` leaves it
/// unchanged.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SettingsOverrides {
    pub min_lines: Option<u32>,
    pub ignore_comments: Option<bool>,
    pub ignore_docstrings: Option<bool>,
    pub ignore_imports: Option<bool>,
    pub ignore_signatures: Option<bool>,
    pub same_file: Option<bool>,
    pub hidden: Option<bool>,
    pub exclude: Option<Vec<String>>,
}

/// Explicit project/configuration selection layered over the compatibility path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectOptions {
    pub config: Option<PathBuf>,
    pub no_config: bool,
    pub project_root: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedSettings {
    pub settings: Settings,

    /// Selected `pyproject.toml`, if Arid configuration was discovered or an
    /// exact configuration file was explicitly selected.
    pub config_path: Option<PathBuf>,

    /// Root used for project-relative settings such as `exclude`.
    pub project_root: PathBuf,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to determine current directory: {0}")]
    CurrentDirectory(#[source] io::Error),

    #[error("configuration search path does not exist: {0:?}")]
    MissingSearchPath(PathBuf),

    #[error("explicit configuration and no-config mode cannot be combined")]
    ConflictingConfigSelection,

    #[error("configuration file does not exist: {0:?}")]
    MissingConfig(PathBuf),

    #[error("configuration path is not a regular file: {0:?}")]
    ConfigNotFile(PathBuf),

    #[error("configuration file must be named pyproject.toml: {0:?}")]
    InvalidConfigName(PathBuf),

    #[error("project root does not exist: {0:?}")]
    MissingProjectRoot(PathBuf),

    #[error("project root is not a directory: {0:?}")]
    ProjectRootNotDirectory(PathBuf),

    #[error("configuration {config:?} is outside explicit project root {project_root:?}")]
    ContradictoryConfigRoot {
        config: PathBuf,
        project_root: PathBuf,
    },

    #[error("failed to read configuration {path:?}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to parse configuration {path:?}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("min-lines must be at least 1")]
    InvalidMinLines,
}

/// Resolves Arid settings using the 1.x compatibility path:
///
/// CLI-style overrides
///     ↓
/// nearest `[tool.arid]`
///     ↓
/// built-in defaults
///
/// `start` may be either a directory or file path. Configuration search
/// begins at its containing directory and walks upward.
pub fn load_settings(
    start: &Path,
    overrides: SettingsOverrides,
) -> Result<LoadedSettings, ConfigError> {
    load_settings_with_options(start, overrides, ProjectOptions::default())
}

/// Resolves Arid settings with explicit project/configuration selection.
pub fn load_settings_with_options(
    start: &Path,
    overrides: SettingsOverrides,
    options: ProjectOptions,
) -> Result<LoadedSettings, ConfigError> {
    if options.config.is_some() && options.no_config {
        return Err(ConfigError::ConflictingConfigSelection);
    }

    let start = absolute_path(start)?;
    let search_dir = configuration_search_dir(&start)?;
    let project_root = options
        .project_root
        .as_deref()
        .map(resolve_project_root)
        .transpose()?;
    let config_path = options
        .config
        .as_deref()
        .map(resolve_exact_config)
        .transpose()?;

    if let (Some(config), Some(root)) = (&config_path, &project_root)
        && config.parent() != Some(root.as_path())
    {
        return Err(ConfigError::ContradictoryConfigRoot {
            config: config.clone(),
            project_root: root.clone(),
        });
    }

    let (mut settings, config_path, project_root) = if let Some(path) = config_path {
        let mut settings = Settings::default();
        let project = read_project(&path)?;
        if let Some(config) = project.tool.arid {
            apply_project_config(&mut settings, config);
        }

        let root = project_root.unwrap_or_else(|| {
            path.parent()
                .expect("pyproject.toml must have a parent directory")
                .to_path_buf()
        });

        (settings, Some(path), root)
    } else if options.no_config {
        (
            Settings::default(),
            None,
            project_root.unwrap_or(search_dir),
        )
    } else if let Some(root) = project_root {
        let path = root.join("pyproject.toml");
        let config = read_optional_arid_config(&path)?;
        let mut settings = Settings::default();
        let config_path = if let Some(config) = config {
            apply_project_config(&mut settings, config);
            Some(path)
        } else {
            None
        };

        (settings, config_path, root)
    } else if let Some((path, config)) = find_arid_config(&search_dir)? {
        let mut settings = Settings::default();
        apply_project_config(&mut settings, config);

        let root = path
            .parent()
            .expect("pyproject.toml must have a parent directory")
            .to_path_buf();

        (settings, Some(path), root)
    } else {
        (Settings::default(), None, search_dir)
    };

    apply_overrides(&mut settings, overrides);
    resolve_project_paths(&mut settings, &project_root);
    validate_settings(&settings)?;

    Ok(LoadedSettings {
        settings,
        config_path,
        project_root,
    })
}

fn absolute_path(path: &Path) -> Result<PathBuf, ConfigError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }

    let current = std::env::current_dir().map_err(ConfigError::CurrentDirectory)?;

    Ok(current.join(path))
}

fn configuration_search_dir(start: &Path) -> Result<PathBuf, ConfigError> {
    let metadata = fs::metadata(start).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            ConfigError::MissingSearchPath(start.to_path_buf())
        } else {
            ConfigError::Read {
                path: start.to_path_buf(),
                source: error,
            }
        }
    })?;

    if metadata.is_dir() {
        return Ok(start.to_path_buf());
    }

    Ok(start
        .parent()
        .expect("a filesystem path must have a parent")
        .to_path_buf())
}

fn resolve_exact_config(path: &Path) -> Result<PathBuf, ConfigError> {
    let path = absolute_path(path)?;
    let metadata = fs::metadata(&path).map_err(|error| match error.kind() {
        io::ErrorKind::NotFound => ConfigError::MissingConfig(path.clone()),
        _ => ConfigError::Read {
            path: path.clone(),
            source: error,
        },
    })?;

    if !metadata.is_file() {
        return Err(ConfigError::ConfigNotFile(path));
    }

    if path.file_name() != Some(OsStr::new("pyproject.toml")) {
        return Err(ConfigError::InvalidConfigName(path));
    }

    fs::canonicalize(&path).map_err(|source| ConfigError::Read { path, source })
}

fn resolve_project_root(path: &Path) -> Result<PathBuf, ConfigError> {
    let path = absolute_path(path)?;
    let metadata = fs::metadata(&path).map_err(|error| match error.kind() {
        io::ErrorKind::NotFound => ConfigError::MissingProjectRoot(path.clone()),
        _ => ConfigError::Read {
            path: path.clone(),
            source: error,
        },
    })?;

    if !metadata.is_dir() {
        return Err(ConfigError::ProjectRootNotDirectory(path));
    }

    fs::canonicalize(&path).map_err(|source| ConfigError::Read { path, source })
}

fn find_arid_config(start: &Path) -> Result<Option<(PathBuf, AridConfig)>, ConfigError> {
    for directory in start.ancestors() {
        let path = directory.join("pyproject.toml");

        if let Some(config) = read_optional_arid_config(&path)? {
            return Ok(Some((path, config)));
        }
    }

    Ok(None)
}

fn read_optional_arid_config(path: &Path) -> Result<Option<AridConfig>, ConfigError> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(ConfigError::Read {
                path: path.to_path_buf(),
                source,
            });
        }
    };

    let project = parse_project(path, &contents)?;
    Ok(project.tool.arid)
}

fn read_project(path: &Path) -> Result<PyProject, ConfigError> {
    let contents = fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;

    parse_project(path, &contents)
}

fn parse_project(path: &Path, contents: &str) -> Result<PyProject, ConfigError> {
    toml::from_str(contents).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

fn apply_project_config(settings: &mut Settings, config: AridConfig) {
    if let Some(value) = config.min_lines {
        settings.min_lines = value;
    }

    if let Some(value) = config.ignore_comments {
        settings.ignore_comments = value;
    }

    if let Some(value) = config.ignore_docstrings {
        settings.ignore_docstrings = value;
    }

    if let Some(value) = config.ignore_imports {
        settings.ignore_imports = value;
    }

    if let Some(value) = config.ignore_signatures {
        settings.ignore_signatures = value;
    }

    if let Some(value) = config.same_file {
        settings.same_file = value;
    }

    if let Some(value) = config.hidden {
        settings.hidden = value;
    }

    if let Some(value) = config.exclude {
        settings.exclude = value;
    }

    if let Some(value) = config.baseline {
        settings.baseline = Some(value);
    }
}

fn apply_overrides(settings: &mut Settings, overrides: SettingsOverrides) {
    if let Some(value) = overrides.min_lines {
        settings.min_lines = value;
    }

    if let Some(value) = overrides.ignore_comments {
        settings.ignore_comments = value;
    }

    if let Some(value) = overrides.ignore_docstrings {
        settings.ignore_docstrings = value;
    }

    if let Some(value) = overrides.ignore_imports {
        settings.ignore_imports = value;
    }

    if let Some(value) = overrides.ignore_signatures {
        settings.ignore_signatures = value;
    }

    if let Some(value) = overrides.same_file {
        settings.same_file = value;
    }

    if let Some(value) = overrides.hidden {
        settings.hidden = value;
    }

    if let Some(value) = overrides.exclude {
        settings.exclude = value;
    }
}

fn resolve_project_paths(settings: &mut Settings, project_root: &Path) {
    let Some(path) = settings.baseline.as_mut() else {
        return;
    };

    if path.is_relative() {
        *path = project_root.join(&*path);
    }
}

fn validate_settings(settings: &Settings) -> Result<(), ConfigError> {
    if settings.min_lines == 0 {
        return Err(ConfigError::InvalidMinLines);
    }

    Ok(())
}

#[derive(Debug, Deserialize, Default)]
struct PyProject {
    #[serde(default)]
    tool: ToolTable,
}

#[derive(Debug, Deserialize, Default)]
struct ToolTable {
    #[serde(default)]
    arid: Option<AridConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct AridConfig {
    min_lines: Option<u32>,
    ignore_comments: Option<bool>,
    ignore_docstrings: Option<bool>,
    ignore_imports: Option<bool>,
    ignore_signatures: Option<bool>,
    same_file: Option<bool>,
    hidden: Option<bool>,
    exclude: Option<Vec<String>>,
    baseline: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
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
                std::env::temp_dir().join(format!("arid-config-test-{}-{id}", std::process::id()));

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

    #[test]
    fn uses_defaults_without_configuration() {
        let temp = TempDir::new();

        let loaded = load_settings(temp.path(), SettingsOverrides::default()).unwrap();

        assert_eq!(loaded.settings, Settings::default());
        assert_eq!(loaded.config_path, None);
        assert_eq!(loaded.project_root, temp.path());
    }

    #[test]
    fn finds_nearest_arid_configuration() {
        let temp = TempDir::new();
        let child = temp.path().join("src").join("nested");

        fs::create_dir_all(&child).unwrap();

        temp.write(
            "pyproject.toml",
            r#"
[tool.arid]
min-lines = 7
ignore-comments = false
same-file = false
hidden = true
exclude = ["generated/**"]
baseline = "config/arid-baseline.json"
"#,
        );

        let loaded = load_settings(&child, SettingsOverrides::default()).unwrap();

        assert_eq!(loaded.settings.min_lines, 7);
        assert!(!loaded.settings.ignore_comments);
        assert!(!loaded.settings.same_file);
        assert!(loaded.settings.hidden);
        assert_eq!(loaded.settings.exclude, vec!["generated/**"]);
        assert_eq!(
            loaded.settings.baseline,
            Some(temp.path().join("config/arid-baseline.json"))
        );
        assert_eq!(loaded.config_path, Some(temp.path().join("pyproject.toml")));
        assert_eq!(loaded.project_root, temp.path());
    }

    #[test]
    fn exact_config_disables_ancestor_search_and_sets_root() {
        let temp = TempDir::new();
        temp.write("pyproject.toml", "[tool.arid]\nmin-lines = 9\n");
        let nested_config = temp.write(
            "nested/pyproject.toml",
            "[tool.arid]\nmin-lines = 6\nbaseline = \"debt.json\"\n",
        );
        let start = temp.path().join("nested/src");
        fs::create_dir_all(&start).unwrap();

        let loaded = load_settings_with_options(
            &start,
            SettingsOverrides::default(),
            ProjectOptions {
                config: Some(nested_config.clone()),
                ..ProjectOptions::default()
            },
        )
        .unwrap();
        let root = nested_config.parent().unwrap();

        assert_eq!(loaded.settings.min_lines, 6);
        assert_eq!(loaded.settings.baseline, Some(root.join("debt.json")));
        assert_eq!(loaded.config_path, Some(nested_config));
        assert_eq!(loaded.project_root, root);
    }

    #[test]
    fn exact_config_without_arid_uses_defaults_and_remains_selected() {
        let temp = TempDir::new();
        let config = temp.write("pyproject.toml", "[tool.ruff]\nline-length = 100\n");

        let loaded = load_settings_with_options(
            temp.path(),
            SettingsOverrides::default(),
            ProjectOptions {
                config: Some(config.clone()),
                ..ProjectOptions::default()
            },
        )
        .unwrap();

        assert_eq!(loaded.settings, Settings::default());
        assert_eq!(loaded.config_path, Some(config));
        assert_eq!(loaded.project_root, temp.path());
    }

    #[test]
    fn no_config_ignores_discovered_project_configuration() {
        let temp = TempDir::new();
        temp.write("pyproject.toml", "[tool.arid]\nmin-lines = 9\n");

        let loaded = load_settings_with_options(
            temp.path(),
            SettingsOverrides {
                min_lines: Some(5),
                ..SettingsOverrides::default()
            },
            ProjectOptions {
                no_config: true,
                ..ProjectOptions::default()
            },
        )
        .unwrap();

        assert_eq!(loaded.settings.min_lines, 5);
        assert_eq!(loaded.config_path, None);
        assert_eq!(loaded.project_root, temp.path());
    }

    #[test]
    fn explicit_project_root_reads_only_root_configuration() {
        let temp = TempDir::new();
        temp.write("pyproject.toml", "[tool.arid]\nmin-lines = 9\n");
        let root = temp.path().join("project");
        let start = root.join("src/nested");
        fs::create_dir_all(&start).unwrap();
        fs::write(
            root.join("pyproject.toml"),
            "[tool.arid]\nmin-lines = 6\n",
        )
        .unwrap();

        let loaded = load_settings_with_options(
            &start,
            SettingsOverrides::default(),
            ProjectOptions {
                project_root: Some(root.clone()),
                ..ProjectOptions::default()
            },
        )
        .unwrap();

        assert_eq!(loaded.settings.min_lines, 6);
        assert_eq!(loaded.config_path, Some(root.join("pyproject.toml")));
        assert_eq!(loaded.project_root, root);
    }

    #[test]
    fn explicit_project_root_never_walks_above_root() {
        let temp = TempDir::new();
        temp.write("pyproject.toml", "[tool.arid]\nmin-lines = 9\n");
        let root = temp.path().join("project");
        let start = root.join("src/nested");
        fs::create_dir_all(&start).unwrap();

        let loaded = load_settings_with_options(
            &start,
            SettingsOverrides::default(),
            ProjectOptions {
                project_root: Some(root.clone()),
                ..ProjectOptions::default()
            },
        )
        .unwrap();

        assert_eq!(loaded.settings, Settings::default());
        assert_eq!(loaded.config_path, None);
        assert_eq!(loaded.project_root, root);
    }

    #[test]
    fn exact_config_and_root_must_identify_same_pyproject() {
        let temp = TempDir::new();
        let config = temp.write("pyproject.toml", "[tool.arid]\nmin-lines = 5\n");
        let other_root = temp.path().join("other");
        fs::create_dir_all(&other_root).unwrap();

        let error = load_settings_with_options(
            temp.path(),
            SettingsOverrides::default(),
            ProjectOptions {
                config: Some(config),
                project_root: Some(other_root),
                ..ProjectOptions::default()
            },
        )
        .unwrap_err();

        assert!(matches!(error, ConfigError::ContradictoryConfigRoot { .. }));
    }

    #[test]
    fn rejects_conflicting_config_selection() {
        let temp = TempDir::new();
        let config = temp.write("pyproject.toml", "[tool.arid]\nmin-lines = 5\n");

        let error = load_settings_with_options(
            temp.path(),
            SettingsOverrides::default(),
            ProjectOptions {
                config: Some(config),
                no_config: true,
                project_root: None,
            },
        )
        .unwrap_err();

        assert!(matches!(error, ConfigError::ConflictingConfigSelection));
    }

    #[test]
    fn rejects_non_pyproject_exact_config() {
        let temp = TempDir::new();
        let config = temp.write("arid.toml", "[tool.arid]\nmin-lines = 5\n");

        let error = load_settings_with_options(
            temp.path(),
            SettingsOverrides::default(),
            ProjectOptions {
                config: Some(config),
                ..ProjectOptions::default()
            },
        )
        .unwrap_err();

        assert!(matches!(error, ConfigError::InvalidConfigName(_)));
    }

    #[test]
    fn rejects_missing_explicit_project_root() {
        let temp = TempDir::new();

        let error = load_settings_with_options(
            temp.path(),
            SettingsOverrides::default(),
            ProjectOptions {
                project_root: Some(temp.path().join("missing")),
                ..ProjectOptions::default()
            },
        )
        .unwrap_err();

        assert!(matches!(error, ConfigError::MissingProjectRoot(_)));
    }

    #[test]
    fn preserves_absolute_configured_baseline_path() {
        let temp = TempDir::new();
        let baseline = temp.path().join("debt.json");

        temp.write(
            "pyproject.toml",
            &format!("[tool.arid]\nbaseline = {:?}\n", baseline.to_string_lossy()),
        );

        let loaded = load_settings(temp.path(), SettingsOverrides::default()).unwrap();

        assert_eq!(loaded.settings.baseline, Some(baseline));
    }

    #[test]
    fn ignores_unrelated_tool_configuration() {
        let temp = TempDir::new();

        temp.write(
            "pyproject.toml",
            r#"
[tool.ruff]
line-length = 100

[tool.maturin]
bindings = "bin"
"#,
        );

        let loaded = load_settings(temp.path(), SettingsOverrides::default()).unwrap();

        assert_eq!(loaded.settings, Settings::default());
        assert_eq!(loaded.config_path, None);
    }

    #[test]
    fn rejects_unknown_arid_keys() {
        let temp = TempDir::new();

        temp.write(
            "pyproject.toml",
            r#"
[tool.arid]
min-line = 4
"#,
        );

        let error = load_settings(temp.path(), SettingsOverrides::default()).unwrap_err();

        assert!(matches!(error, ConfigError::Parse { .. }));
    }

    #[test]
    fn overrides_project_configuration() {
        let temp = TempDir::new();

        temp.write(
            "pyproject.toml",
            r#"
[tool.arid]
min-lines = 8
ignore-comments = true
same-file = true
hidden = true
exclude = ["generated/**"]
"#,
        );

        let loaded = load_settings(
            temp.path(),
            SettingsOverrides {
                min_lines: Some(5),
                ignore_comments: Some(false),
                same_file: Some(false),
                hidden: Some(false),
                exclude: Some(vec!["vendor/**".to_owned()]),
                ..SettingsOverrides::default()
            },
        )
        .unwrap();

        assert_eq!(loaded.settings.min_lines, 5);
        assert!(!loaded.settings.ignore_comments);
        assert!(!loaded.settings.same_file);
        assert!(!loaded.settings.hidden);
        assert_eq!(loaded.settings.exclude, vec!["vendor/**"]);
    }

    #[test]
    fn rejects_zero_min_lines_from_project() {
        let temp = TempDir::new();

        temp.write(
            "pyproject.toml",
            r#"
[tool.arid]
min-lines = 0
"#,
        );

        let error = load_settings(temp.path(), SettingsOverrides::default()).unwrap_err();

        assert!(matches!(error, ConfigError::InvalidMinLines));
    }

    #[test]
    fn converts_settings_to_pipeline_options() {
        let settings = Settings {
            min_lines: 6,
            ignore_comments: false,
            ignore_docstrings: true,
            ignore_imports: false,
            ignore_signatures: true,
            same_file: false,
            hidden: true,
            exclude: Vec::new(),
            baseline: None,
        };

        assert_eq!(
            settings.normalization_options(),
            NormalizationOptions {
                ignore_comments: false,
                ignore_docstrings: true,
                ignore_imports: false,
                ignore_signatures: true,
            }
        );

        assert_eq!(
            settings.detection_options(),
            DetectionOptions {
                min_lines: 6,
                same_file: false,
            }
        );
    }
}
