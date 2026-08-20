use std::fmt;
use std::path::Path;

use serde::Serialize;

use crate::project_path::project_relative_path;

pub(crate) const ERROR_SCHEMA_VERSION: u8 = 1;

/// Stable machine-readable categories for operational failures.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorKind {
    Configuration,
    Discovery,
    Read,
    Parse,
    Normalization,
    Baseline,
    Output,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OperationalError {
    pub(crate) kind: ErrorKind,
    pub(crate) message: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) path: Option<String>,
}

impl OperationalError {
    pub(crate) fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            path: None,
        }
    }

    pub(crate) fn with_project_path(mut self, path: &Path, project_root: &Path) -> Self {
        if let Ok(relative) = project_relative_path(path, project_root) {
            self.path = Some(relative);
        }

        self
    }
}

impl fmt::Display for OperationalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for OperationalError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ErrorDocument {
    schema_version: u8,
    tool_version: &'static str,
    error: OperationalError,
}

pub(crate) fn render_error_json(error: OperationalError) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&ErrorDocument {
        schema_version: ERROR_SCHEMA_VERSION,
        tool_version: env!("CARGO_PKG_VERSION"),
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_stable_error_kind() {
        let error = OperationalError::new(ErrorKind::Configuration, "bad configuration");
        let value = serde_json::to_value(error).unwrap();

        assert_eq!(value["kind"], "configuration");
        assert_eq!(value["message"], "bad configuration");
        assert!(value.get("path").is_none());
    }

    #[test]
    fn adds_safe_project_relative_path() {
        let error = OperationalError::new(ErrorKind::Read, "failed")
            .with_project_path(Path::new("project/src/a.py"), Path::new("project"));

        assert_eq!(error.path.as_deref(), Some("src/a.py"));
    }

    #[test]
    fn omits_path_outside_project_root() {
        let error = OperationalError::new(ErrorKind::Read, "failed")
            .with_project_path(Path::new("elsewhere/a.py"), Path::new("project"));

        assert_eq!(error.path, None);
    }

    #[test]
    fn fatal_error_document_matches_v1_contract() {
        let json = render_error_json(OperationalError::new(
            ErrorKind::Configuration,
            "bad configuration",
        ))
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["tool_version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(value["error"]["kind"], "configuration");
        assert_eq!(value["error"]["message"], "bad configuration");
    }
}
