use std::path::{Component, Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum ProjectPathError {
    #[error("path {path:?} is outside project root {root:?}")]
    OutsideRoot { path: PathBuf, root: PathBuf },

    #[error("path {0:?} is not a canonical project-relative path")]
    InvalidPath(PathBuf),

    #[error("path {0:?} is not valid UTF-8")]
    NonUtf8(PathBuf),

    #[error("invalid serialized project-relative path {0:?}")]
    InvalidSerialized(String),
}

/// Converts a path inside `project_root` to Arid's canonical serialized form.
pub(crate) fn project_relative_path(
    path: &Path,
    project_root: &Path,
) -> Result<String, ProjectPathError> {
    let relative = path
        .strip_prefix(project_root)
        .map_err(|_| ProjectPathError::OutsideRoot {
            path: path.to_path_buf(),
            root: project_root.to_path_buf(),
        })?;

    let mut parts = Vec::new();

    for component in relative.components() {
        let Component::Normal(part) = component else {
            return Err(ProjectPathError::InvalidPath(path.to_path_buf()));
        };

        let part = part
            .to_str()
            .ok_or_else(|| ProjectPathError::NonUtf8(path.to_path_buf()))?;

        parts.push(part);
    }

    let serialized = parts.join("/");
    validate_project_relative_path(&serialized)?;

    Ok(serialized)
}

/// Validates the canonical serialized path contract already used by baseline v1.
pub(crate) fn validate_project_relative_path(path: &str) -> Result<(), ProjectPathError> {
    if path.is_empty()
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains('\\')
        || path.contains('\0')
    {
        return Err(ProjectPathError::InvalidSerialized(path.to_owned()));
    }

    let bytes = path.as_bytes();

    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        return Err(ProjectPathError::InvalidSerialized(path.to_owned()));
    }

    if path
        .split('/')
        .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(ProjectPathError::InvalidSerialized(path.to_owned()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_project_relative_path_with_forward_slashes() {
        assert_eq!(
            project_relative_path(Path::new("project/src/a.py"), Path::new("project")).unwrap(),
            "src/a.py"
        );
    }

    #[test]
    fn accepts_unicode_components() {
        assert_eq!(
            project_relative_path(Path::new("project/src/π.py"), Path::new("project")).unwrap(),
            "src/π.py"
        );
    }

    #[test]
    fn rejects_path_outside_root() {
        assert!(matches!(
            project_relative_path(Path::new("elsewhere/a.py"), Path::new("project")),
            Err(ProjectPathError::OutsideRoot { .. })
        ));
    }

    #[test]
    fn rejects_project_root_itself() {
        assert!(matches!(
            project_relative_path(Path::new("project"), Path::new("project")),
            Err(ProjectPathError::InvalidSerialized(path)) if path.is_empty()
        ));
    }

    #[test]
    fn serialized_contract_matches_baseline_v1_rules() {
        for valid in ["a.py", "src/a.py", "src/π.py"] {
            assert!(validate_project_relative_path(valid).is_ok(), "{valid:?}");
        }

        for invalid in [
            "",
            "/a.py",
            "a.py/",
            "src//a.py",
            "./a.py",
            "src/../a.py",
            "src\\a.py",
            "C:/src/a.py",
            "a\0.py",
        ] {
            assert!(
                validate_project_relative_path(invalid).is_err(),
                "{invalid:?}"
            );
        }
    }
}
