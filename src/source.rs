use std::fs;
use std::path::{Path, PathBuf};

use rayon::prelude::*;

use crate::error::{ErrorKind, OperationalError};
use crate::model::{NormalizationOptions, PreparedFile};
use crate::normalize::prepare_file;

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

pub(crate) fn build_source_inputs(
    disk_paths: Vec<PathBuf>,
    virtual_source: Option<(PathBuf, String)>,
) -> Vec<SourceInput> {
    let virtual_path = virtual_source.as_ref().map(|(path, _)| path);

    let mut inputs = disk_paths
        .into_iter()
        .filter(|path| virtual_path != Some(path))
        .map(SourceInput::Disk)
        .collect::<Vec<_>>();

    if let Some((path, source)) = virtual_source {
        inputs.push(SourceInput::Virtual { path, source });
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
    if workers == 0 {
        return Err(OperationalError::new(
            ErrorKind::Configuration,
            "worker count must be at least 1",
        ));
    }

    if workers == 1 || inputs.len() < 2 {
        return inputs
            .into_iter()
            .map(|input| prepare_source(input, options, project_root))
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
            .map(|input| prepare_source(input, options, project_root))
            .collect::<Vec<_>>()
    });

    results.into_iter().collect()
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
            Some((a.clone(), "alpha = 1\n".to_owned())),
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
            Some((virtual_path.clone(), "alpha = 1\n".to_owned())),
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
            Some((virtual_path.clone(), "alpha = 1\nbeta = 2\n".to_owned())),
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
}
