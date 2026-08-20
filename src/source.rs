use std::fs;
use std::path::{Path, PathBuf};

use rayon::prelude::*;

pub(crate) mod focus;

use crate::config::Settings;
use crate::error::{ErrorKind, OperationalError};
use crate::files::{is_excluded_path, is_python_file};
use crate::model::{NormalizationOptions, PreparedFile};
use crate::normalize::prepare_file;
use crate::project_path::project_relative_path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceInput {
    Disk(PathBuf),
    Virtual { path: PathBuf, source: String },
}

impl SourceInput {
    pub(crate) fn path(&self) -> &Path {
        match self {
            Self::Disk(path) | Self::Virtual { path, .. } => path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VirtualSource {
    path: PathBuf,
    project_path: String,
    source: String,
}

impl VirtualSource {
    pub(crate) fn project_path(&self) -> &str {
        &self.project_path
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PreparationResult {
    pub(crate) files: Vec<PreparedFile>,
    pub(crate) errors: Vec<OperationalError>,
}

pub(crate) fn resolve_virtual_source(
    stdin_path: Option<&Path>,
    stdin_source: Option<&str>,
    settings: &Settings,
    project_root: &Path,
) -> Result<Option<VirtualSource>, OperationalError> {
    let Some(stdin_path) = stdin_path else {
        if stdin_source.is_some() {
            return Err(OperationalError::new(
                ErrorKind::Configuration,
                "virtual stdin source requires --stdin-path",
            ));
        }
        return Ok(None);
    };

    let source = stdin_source.ok_or_else(|| {
        OperationalError::new(
            ErrorKind::Configuration,
            "--stdin-path requires virtual source text in RunContext",
        )
    })?;

    let path = if stdin_path.is_absolute() {
        stdin_path.to_path_buf()
    } else {
        project_root.join(stdin_path)
    };

    let project_path = project_relative_path(&path, project_root).map_err(|error| {
        OperationalError::new(
            ErrorKind::Configuration,
            format!("invalid --stdin-path {}: {error}", stdin_path.display()),
        )
    })?;

    if !is_python_file(&path) {
        return Err(OperationalError::new(
            ErrorKind::Configuration,
            "--stdin-path must identify a .py or .pyi file",
        ));
    }

    let excluded = is_excluded_path(&path, settings, project_root).map_err(|error| {
        OperationalError::new(
            ErrorKind::Discovery,
            format!("failed to evaluate --stdin-path excludes: {error}"),
        )
    })?;

    if excluded {
        return Err(OperationalError::new(
            ErrorKind::Configuration,
            format!("--stdin-path is excluded by Arid configuration: {project_path}"),
        ));
    }

    Ok(Some(VirtualSource {
        path,
        project_path,
        source: source.to_owned(),
    }))
}

pub(crate) fn build_source_inputs(
    disk_paths: Vec<PathBuf>,
    virtual_source: Option<VirtualSource>,
) -> Vec<SourceInput> {
    let virtual_path = virtual_source.as_ref().map(|source| &source.path);

    let mut inputs = disk_paths
        .into_iter()
        .filter(|path| virtual_path != Some(path))
        .map(SourceInput::Disk)
        .collect::<Vec<_>>();

    if let Some(virtual_source) = virtual_source {
        inputs.push(SourceInput::Virtual {
            path: virtual_source.path,
            source: virtual_source.source,
        });
    }

    inputs.sort_by(|left, right| left.path().cmp(right.path()));
    inputs
}

pub(crate) fn prepare_sources(
    inputs: Vec<SourceInput>,
    options: NormalizationOptions,
    workers: usize,
    project_root: &Path,
) -> Result<Vec<PreparedFile>, OperationalError> {
    Ok(prepare_sources_with_policy(inputs, options, workers, project_root, false)?.files)
}

pub(crate) fn prepare_sources_with_policy(
    inputs: Vec<SourceInput>,
    options: NormalizationOptions,
    workers: usize,
    project_root: &Path,
    keep_going: bool,
) -> Result<PreparationResult, OperationalError> {
    if workers == 0 {
        return Err(OperationalError::new(
            ErrorKind::Configuration,
            "worker count must be at least 1",
        ));
    }

    if workers == 1 || inputs.len() < 2 {
        if !keep_going {
            let files = inputs
                .into_iter()
                .map(|input| prepare_source(input, options, project_root))
                .collect::<Result<Vec<_>, _>>()?;

            return Ok(PreparationResult {
                files,
                errors: Vec::new(),
            });
        }

        let results = inputs
            .into_iter()
            .map(|input| prepare_source(input, options, project_root))
            .collect::<Vec<_>>();

        return collect_preparation_results(results, true);
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
            .map(|input| prepare_source(input, options, project_root))
            .collect::<Vec<_>>()
    });

    collect_preparation_results(results, keep_going)
}

fn collect_preparation_results(
    results: Vec<Result<PreparedFile, OperationalError>>,
    keep_going: bool,
) -> Result<PreparationResult, OperationalError> {
    let mut files = Vec::with_capacity(results.len());
    let mut errors = Vec::new();

    for result in results {
        match result {
            Ok(file) => files.push(file),
            Err(error) if keep_going => errors.push(error),
            Err(error) => return Err(error),
        }
    }

    Ok(PreparationResult { files, errors })
}

fn prepare_source(
    input: SourceInput,
    options: NormalizationOptions,
    project_root: &Path,
) -> Result<PreparedFile, OperationalError> {
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

    prepare_file(path.clone(), source, options).map_err(|error| {
        OperationalError::new(
            ErrorKind::Parse,
            format!("failed to prepare Python source: {error}"),
        )
        .with_project_path(&path, project_root)
    })
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
                std::env::temp_dir().join(format!("arid-source-test-{}-{id}", std::process::id()));
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

    fn virtual_source(path: PathBuf, source: &str) -> VirtualSource {
        let project_path = path.file_name().unwrap().to_string_lossy().into_owned();
        VirtualSource {
            path,
            project_path,
            source: source.to_owned(),
        }
    }

    #[test]
    fn resolves_relative_virtual_path_inside_project() {
        let temp = TempDir::new();
        let resolved = resolve_virtual_source(
            Some(Path::new("src/proposed.py")),
            Some("alpha = 1\n"),
            &Settings::default(),
            temp.path(),
        )
        .unwrap()
        .unwrap();

        assert_eq!(resolved.path, temp.path().join("src/proposed.py"));
        assert_eq!(resolved.project_path(), "src/proposed.py");
    }

    #[test]
    fn rejects_virtual_path_outside_project() {
        let temp = TempDir::new();
        let error = resolve_virtual_source(
            Some(Path::new("../outside.py")),
            Some("alpha = 1\n"),
            &Settings::default(),
            temp.path(),
        )
        .unwrap_err();

        assert_eq!(error.kind, ErrorKind::Configuration);
        assert!(error.to_string().contains("invalid --stdin-path"));
    }

    #[test]
    fn rejects_non_python_virtual_path() {
        let temp = TempDir::new();
        let error = resolve_virtual_source(
            Some(Path::new("proposed.txt")),
            Some("alpha = 1\n"),
            &Settings::default(),
            temp.path(),
        )
        .unwrap_err();

        assert!(error.to_string().contains(".py or .pyi"));
    }

    #[test]
    fn rejects_configured_exclude_for_virtual_path() {
        let temp = TempDir::new();
        let settings = Settings {
            exclude: vec!["generated/**".to_owned()],
            ..Settings::default()
        };
        let error = resolve_virtual_source(
            Some(Path::new("generated/proposed.py")),
            Some("alpha = 1\n"),
            &settings,
            temp.path(),
        )
        .unwrap_err();

        assert!(error.to_string().contains("excluded by Arid configuration"));
    }

    #[test]
    fn source_inputs_are_sorted_by_path() {
        let temp = TempDir::new();
        let a = temp.path().join("a.py");
        let b = temp.path().join("b.py");

        let inputs = build_source_inputs(vec![b.clone(), a.clone()], None);

        assert_eq!(inputs[0].path(), a);
        assert_eq!(inputs[1].path(), b);
    }

    #[test]
    fn virtual_source_replaces_matching_disk_identity() {
        let temp = TempDir::new();
        let a = temp.path().join("a.py");
        let b = temp.path().join("b.py");

        let inputs = build_source_inputs(
            vec![a.clone(), b.clone()],
            Some(virtual_source(a.clone(), "alpha = 1\n")),
        );

        assert_eq!(inputs.len(), 2);
        assert!(matches!(
            &inputs[0],
            SourceInput::Virtual { path, source }
                if path == &a && source == "alpha = 1\n"
        ));
        assert!(matches!(&inputs[1], SourceInput::Disk(path) if path == &b));
    }

    #[test]
    fn virtual_source_is_added_without_disk_file() {
        let temp = TempDir::new();
        let virtual_path = temp.path().join("proposed.py");

        let inputs = build_source_inputs(
            Vec::new(),
            Some(virtual_source(virtual_path.clone(), "alpha = 1\n")),
        );

        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].path(), virtual_path);
        assert!(!virtual_path.exists());
    }

    #[test]
    fn virtual_source_uses_normal_preparation_without_disk_io() {
        let temp = TempDir::new();
        let virtual_path = temp.path().join("proposed.py");
        let inputs = build_source_inputs(
            Vec::new(),
            Some(virtual_source(
                virtual_path.clone(),
                "alpha = 1\nbeta = 2\n",
            )),
        );

        let prepared =
            prepare_sources(inputs, NormalizationOptions::default(), 1, temp.path()).unwrap();

        assert_eq!(prepared.len(), 1);
        assert_eq!(prepared[0].path, virtual_path);
        assert_eq!(prepared[0].source, "alpha = 1\nbeta = 2\n");
        assert!(!prepared[0].path.exists());
    }

    #[test]
    fn parallel_preparation_preserves_source_order() {
        let temp = TempDir::new();
        let a = temp.write("a.py", "alpha = 1\n");
        let b = temp.write("b.py", "beta = 2\n");
        let c = temp.write("c.py", "gamma = 3\n");

        let inputs = build_source_inputs(vec![c.clone(), a.clone(), b.clone()], None);
        let prepared =
            prepare_sources(inputs, NormalizationOptions::default(), 3, temp.path()).unwrap();

        assert_eq!(
            prepared
                .into_iter()
                .map(|file| file.path)
                .collect::<Vec<_>>(),
            vec![a, b, c]
        );
    }

    #[test]
    fn keep_going_collects_errors_and_valid_files_in_source_order() {
        let temp = TempDir::new();
        let a = temp.write("a.py", "alpha = 1\n");
        let b = temp.write("b.py", "def broken(:\n");
        let c = temp.path().join("c.py");
        let d = temp.write("d.py", "delta = 4\n");

        let inputs = vec![
            SourceInput::Disk(a.clone()),
            SourceInput::Disk(b),
            SourceInput::Disk(c),
            SourceInput::Disk(d.clone()),
        ];
        let result = prepare_sources_with_policy(
            inputs,
            NormalizationOptions::default(),
            4,
            temp.path(),
            true,
        )
        .unwrap();

        assert_eq!(
            result
                .files
                .into_iter()
                .map(|file| file.path)
                .collect::<Vec<_>>(),
            vec![a, d]
        );
        assert_eq!(result.errors.len(), 2);
        assert_eq!(result.errors[0].kind, ErrorKind::Parse);
        assert_eq!(result.errors[0].path.as_deref(), Some("b.py"));
        assert_eq!(result.errors[1].kind, ErrorKind::Read);
        assert_eq!(result.errors[1].path.as_deref(), Some("c.py"));
    }

    #[test]
    fn fail_fast_parallel_mode_reports_earliest_error_in_source_order() {
        let temp = TempDir::new();
        let a = temp.write("a.py", "alpha = 1\n");
        let b = temp.write("b.py", "def broken_b(:\n");
        let c = temp.write("c.py", "def broken_c(:\n");

        let error = prepare_sources_with_policy(
            vec![
                SourceInput::Disk(a),
                SourceInput::Disk(b),
                SourceInput::Disk(c),
            ],
            NormalizationOptions::default(),
            3,
            temp.path(),
            false,
        )
        .unwrap_err();

        assert_eq!(error.kind, ErrorKind::Parse);
        assert_eq!(error.path.as_deref(), Some("b.py"));
    }
}
