use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use thiserror::Error;

use crate::config::Settings;

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("failed to determine current directory: {0}")]
    CurrentDirectory(#[source] io::Error),

    #[error("input path does not exist: {0:?}")]
    MissingPath(PathBuf),

    #[error("failed to inspect path {path:?}: {source}")]
    Metadata {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed while discovering files: {0}")]
    Walk(#[from] ignore::Error),

    #[error("invalid exclude pattern {pattern:?}: {source}")]
    InvalidExclude {
        pattern: String,
        #[source]
        source: ignore::Error,
    },

    #[error("failed to build exclude matcher: {0}")]
    ExcludeMatcher(#[source] ignore::Error),
}

/// Discovers Python source files from explicit files and directory roots.
///
/// Directory traversal respects the normal ignore rules provided by the
/// `ignore` crate, including `.gitignore`.
///
/// Explicit file arguments bypass those normal discovery ignore rules because
/// naming a file directly is an explicit request to scan it. Configured Arid
/// `exclude` patterns still apply.
///
/// Returned paths are absolute, deduplicated, and sorted deterministically.
pub fn discover_python_files(
    paths: &[PathBuf],
    settings: &Settings,
    project_root: &Path,
) -> Result<Vec<PathBuf>, DiscoveryError> {
    let project_root = absolute_path(project_root)?;
    let excludes = build_exclude_matcher(&project_root, &settings.exclude)?;

    let mut discovered = BTreeSet::new();

    for path in paths {
        let path = absolute_path(path)?;

        let symlink_metadata = fs::symlink_metadata(&path).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                DiscoveryError::MissingPath(path.clone())
            } else {
                DiscoveryError::Metadata {
                    path: path.clone(),
                    source: error,
                }
            }
        })?;

        if symlink_metadata.file_type().is_symlink() {
            let target = fs::metadata(&path).map_err(|source| DiscoveryError::Metadata {
                path: path.clone(),
                source,
            })?;

            if target.is_dir() {
                // Directory symlinks are intentionally not followed.
                continue;
            }

            if target.is_file()
                && is_python_file(&path)
                && !is_excluded(&excludes, &project_root, &path, false)
            {
                discovered.insert(path);
            }

            continue;
        }

        if symlink_metadata.is_file() {
            if is_python_file(&path) && !is_excluded(&excludes, &project_root, &path, false) {
                discovered.insert(path);
            }

            continue;
        }

        if symlink_metadata.is_dir() {
            discover_directory(
                &path,
                &project_root,
                &excludes,
                settings.hidden,
                &mut discovered,
            )?;
        }
    }

    Ok(discovered.into_iter().collect())
}

pub(crate) fn is_excluded_path(
    path: &Path,
    settings: &Settings,
    project_root: &Path,
) -> Result<bool, DiscoveryError> {
    let project_root = absolute_path(project_root)?;
    let path = absolute_path(path)?;
    let excludes = build_exclude_matcher(&project_root, &settings.exclude)?;

    Ok(is_excluded(&excludes, &project_root, &path, false))
}

fn discover_directory(
    root: &Path,
    project_root: &Path,
    excludes: &Gitignore,
    include_hidden: bool,
    discovered: &mut BTreeSet<PathBuf>,
) -> Result<(), DiscoveryError> {
    let exclude_matcher = excludes.clone();
    let exclude_root = project_root.to_path_buf();
    let walk_root = root.to_path_buf();

    let mut builder = WalkBuilder::new(root);

    builder.follow_links(false);
    builder.hidden(!include_hidden);

    builder.filter_entry(move |entry| {
        let path = entry.path();

        // Always allow the walk root itself. Exclusion applies to its
        // descendants.
        if path == walk_root {
            return true;
        }

        let is_dir = entry
            .file_type()
            .is_some_and(|file_type| file_type.is_dir());

        !is_excluded(&exclude_matcher, &exclude_root, path, is_dir)
    });

    for entry in builder.build() {
        let entry = entry?;

        let Some(file_type) = entry.file_type() else {
            continue;
        };

        if !file_type.is_file() {
            continue;
        }

        let path = entry.into_path();

        if is_python_file(&path) {
            discovered.insert(path);
        }
    }

    Ok(())
}

fn build_exclude_matcher(
    project_root: &Path,
    patterns: &[String],
) -> Result<Gitignore, DiscoveryError> {
    let mut builder = GitignoreBuilder::new(project_root);

    for pattern in patterns {
        builder
            .add_line(None, pattern)
            .map_err(|source| DiscoveryError::InvalidExclude {
                pattern: pattern.clone(),
                source,
            })?;
    }

    builder.build().map_err(DiscoveryError::ExcludeMatcher)
}

fn is_excluded(matcher: &Gitignore, project_root: &Path, path: &Path, is_dir: bool) -> bool {
    if !path.starts_with(project_root) {
        return false;
    }

    matcher
        .matched_path_or_any_parents(path, is_dir)
        .is_ignore()
}

fn absolute_path(path: &Path) -> Result<PathBuf, DiscoveryError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }

    let current = std::env::current_dir().map_err(DiscoveryError::CurrentDirectory)?;

    Ok(current.join(path))
}

pub(crate) fn is_python_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("py" | "pyi")
    )
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
                std::env::temp_dir().join(format!("arid-files-test-{}-{id}", std::process::id()));

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
    fn discovers_py_and_pyi_files_only() {
        let temp = TempDir::new();

        let py = temp.write("src/a.py", "");
        let pyi = temp.write("src/b.pyi", "");
        temp.write("src/readme.txt", "");
        temp.write("src/data.json", "");

        let files = discover_python_files(
            &[temp.path().to_path_buf()],
            &Settings::default(),
            temp.path(),
        )
        .unwrap();

        assert_eq!(files, vec![py, pyi]);
    }

    #[test]
    fn respects_gitignore_during_directory_discovery() {
        let temp = TempDir::new();

        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let included = temp.write("src/included.py", "");
        temp.write("generated/ignored.py", "");

        temp.write(".gitignore", "generated/\n");

        let files = discover_python_files(
            &[temp.path().to_path_buf()],
            &Settings::default(),
            temp.path(),
        )
        .unwrap();

        assert_eq!(files, vec![included]);
    }

    #[test]
    fn skips_hidden_paths_by_default() {
        let temp = TempDir::new();

        let visible = temp.write("src/visible.py", "");
        temp.write(".github/actions/hidden.py", "");

        let files = discover_python_files(
            &[temp.path().to_path_buf()],
            &Settings::default(),
            temp.path(),
        )
        .unwrap();

        assert_eq!(files, vec![visible]);
    }

    #[test]
    fn can_include_hidden_paths() {
        let temp = TempDir::new();

        let visible = temp.write("src/visible.py", "");
        let hidden = temp.write(".github/actions/hidden.py", "");

        let settings = Settings {
            hidden: true,
            ..Settings::default()
        };

        let files =
            discover_python_files(&[temp.path().to_path_buf()], &settings, temp.path()).unwrap();

        assert_eq!(files, vec![hidden, visible]);
    }

    #[test]
    fn explicit_file_bypasses_gitignore() {
        let temp = TempDir::new();

        let ignored = temp.write("generated/ignored.py", "");

        temp.write(".gitignore", "generated/\n");

        let files = discover_python_files(
            std::slice::from_ref(&ignored),
            &Settings::default(),
            temp.path(),
        )
        .unwrap();

        assert_eq!(files, vec![ignored]);
    }

    #[test]
    fn configured_exclude_applies_to_directory_discovery() {
        let temp = TempDir::new();

        let included = temp.write("src/included.py", "");
        temp.write("generated/ignored.py", "");

        let settings = Settings {
            exclude: vec!["generated/**".to_owned()],
            ..Settings::default()
        };

        let files =
            discover_python_files(&[temp.path().to_path_buf()], &settings, temp.path()).unwrap();

        assert_eq!(files, vec![included]);
    }

    #[test]
    fn configured_exclude_applies_to_explicit_file() {
        let temp = TempDir::new();

        let excluded = temp.write("generated/ignored.py", "");

        let settings = Settings {
            exclude: vec!["generated/**".to_owned()],
            ..Settings::default()
        };

        let files =
            discover_python_files(std::slice::from_ref(&excluded), &settings, temp.path()).unwrap();

        assert!(files.is_empty());
    }

    #[test]
    fn exclude_check_does_not_require_path_to_exist() {
        let temp = TempDir::new();
        let settings = Settings {
            exclude: vec!["generated/**".to_owned()],
            ..Settings::default()
        };

        assert!(
            is_excluded_path(&temp.path().join("generated/proposed.py"), &settings, temp.path())
                .unwrap()
        );
        assert!(
            !is_excluded_path(&temp.path().join("src/proposed.py"), &settings, temp.path()).unwrap()
        );
    }

    #[test]
    fn deduplicates_overlapping_input_roots() {
        let temp = TempDir::new();

        let file = temp.write("src/example.py", "");

        let files = discover_python_files(
            &[temp.path().to_path_buf(), temp.path().join("src")],
            &Settings::default(),
            temp.path(),
        )
        .unwrap();

        assert_eq!(files, vec![file]);
    }

    #[test]
    fn returns_paths_in_deterministic_order() {
        let temp = TempDir::new();

        let a = temp.write("a.py", "");
        let b = temp.write("b.py", "");
        let c = temp.write("c.py", "");

        let files = discover_python_files(
            &[temp.path().to_path_buf()],
            &Settings::default(),
            temp.path(),
        )
        .unwrap();

        assert_eq!(files, vec![a, b, c]);
    }

    #[test]
    fn ignores_non_python_explicit_files() {
        let temp = TempDir::new();

        let text = temp.write("notes.txt", "");

        let files = discover_python_files(&[text], &Settings::default(), temp.path()).unwrap();

        assert!(files.is_empty());
    }

    #[test]
    fn rejects_missing_input_path() {
        let temp = TempDir::new();
        let missing = temp.path().join("missing");

        let error = discover_python_files(
            std::slice::from_ref(&missing),
            &Settings::default(),
            temp.path(),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            DiscoveryError::MissingPath(path)
                if path == missing
        ));
    }

    #[cfg(unix)]
    #[test]
    fn does_not_follow_directory_symlinks() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new();

        let target = temp.path().join("target");
        fs::create_dir_all(&target).unwrap();

        temp.write("target/example.py", "");

        let link = temp.path().join("linked");
        symlink(&target, &link).unwrap();

        let files = discover_python_files(&[link], &Settings::default(), temp.path()).unwrap();

        assert!(files.is_empty());
    }
}
