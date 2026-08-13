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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedSettings {
    pub settings: Settings,

    /// `pyproject.toml` providing `[tool.arid]`, if one was found.
    pub config_path: Option<PathBuf>,

    /// Root used for project-relative settings such as `exclude`.
    ///
    /// This is the directory containing the selected `pyproject.toml`, or
    /// the configuration search directory when no Arid configuration exists.
    pub project_root: PathBuf,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to determine current directory: {0}")]
    CurrentDirectory(#[source] io::Error),

    #[error("configuration search path does not exist: {0:?}")]
    MissingSearchPath(PathBuf),

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

/// Resolves Arid settings using:
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
    let start = absolute_path(start)?;
    let search_dir = configuration_search_dir(&start)?;

    let config = find_arid_config(&search_dir)?;

    let (mut settings, config_path, project_root) = if let Some((path, config)) = config {
        let mut settings = Settings::default();
        apply_project_config(&mut settings, config);

        let root = path
            .parent()
            .expect("pyproject.toml must have a parent directory")
            .to_path_buf();

        (settings, Some(path), root)
    } else {
        (Settings::default(), None, search_dir.clone())
    };

    apply_overrides(&mut settings, overrides);
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

fn find_arid_config(start: &Path) -> Result<Option<(PathBuf, AridConfig)>, ConfigError> {
    for directory in start.ancestors() {
        let path = directory.join("pyproject.toml");

        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                continue;
            }
            Err(source) => {
                return Err(ConfigError::Read { path, source });
            }
        };

        let project: PyProject =
            toml::from_str(&contents).map_err(|source| ConfigError::Parse {
                path: path.clone(),
                source,
            })?;

        if let Some(config) = project.tool.arid {
            return Ok(Some((path, config)));
        }
    }

    Ok(None)
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

        fs::write(
            temp.path().join("pyproject.toml"),
            r#"
[tool.arid]
min-lines = 7
ignore-comments = false
same-file = false
hidden = true
exclude = ["generated/**"]
"#,
        )
        .unwrap();

        let loaded = load_settings(&child, SettingsOverrides::default()).unwrap();

        assert_eq!(loaded.settings.min_lines, 7);
        assert!(!loaded.settings.ignore_comments);
        assert!(!loaded.settings.same_file);
        assert!(loaded.settings.hidden);
        assert_eq!(loaded.settings.exclude, vec!["generated/**"]);

        assert_eq!(loaded.config_path, Some(temp.path().join("pyproject.toml")));

        assert_eq!(loaded.project_root, temp.path());
    }

    #[test]
    fn ignores_unrelated_tool_configuration() {
        let temp = TempDir::new();

        fs::write(
            temp.path().join("pyproject.toml"),
            r#"
[tool.ruff]
line-length = 100

[tool.maturin]
bindings = "bin"
"#,
        )
        .unwrap();

        let loaded = load_settings(temp.path(), SettingsOverrides::default()).unwrap();

        assert_eq!(loaded.settings, Settings::default());
        assert_eq!(loaded.config_path, None);
    }

    #[test]
    fn rejects_unknown_arid_keys() {
        let temp = TempDir::new();

        fs::write(
            temp.path().join("pyproject.toml"),
            r#"
[tool.arid]
min-line = 4
"#,
        )
        .unwrap();

        let error = load_settings(temp.path(), SettingsOverrides::default()).unwrap_err();

        assert!(matches!(error, ConfigError::Parse { .. }));
    }

    #[test]
    fn overrides_project_configuration() {
        let temp = TempDir::new();

        fs::write(
            temp.path().join("pyproject.toml"),
            r#"
[tool.arid]
min-lines = 8
ignore-comments = true
same-file = true
hidden = true
exclude = ["generated/**"]
"#,
        )
        .unwrap();

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

        fs::write(
            temp.path().join("pyproject.toml"),
            r#"
[tool.arid]
min-lines = 0
"#,
        )
        .unwrap();

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