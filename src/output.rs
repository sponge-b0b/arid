use std::collections::BTreeSet;
use std::fs;
use std::io::Write as _;
use std::path::{Component, Path, PathBuf};

use atomic_write_file::AtomicWriteFile;

use crate::cli::OutputFormat;
use crate::error::{ErrorKind, OperationalError};
use crate::markdown::render_markdown;
use crate::report::{Report, render_json, render_text_plain};
use crate::sarif::render_sarif;
use crate::source::SourceInput;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReportTarget {
    pub(crate) format: OutputFormat,
    pub(crate) path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RenderedTarget {
    path: PathBuf,
    contents: String,
}

pub(crate) fn resolve_report_targets(
    specs: &[String],
    source_inputs: &[SourceInput],
    baseline_path: Option<&Path>,
) -> Result<Vec<ReportTarget>, OperationalError> {
    let mut targets = Vec::with_capacity(specs.len());
    let mut destinations = BTreeSet::new();
    let source_paths = source_inputs
        .iter()
        .map(|input| comparison_path(input.path()))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let baseline_path = baseline_path.map(comparison_path).transpose()?;

    for spec in specs {
        let target = parse_report_target(spec)?;
        let destination = comparison_path(&target.path)?;

        if !destinations.insert(destination.clone()) {
            return Err(OperationalError::new(
                ErrorKind::Configuration,
                format!("duplicate --report destination: {}", target.path.display()),
            ));
        }

        if source_paths.contains(&destination) {
            return Err(OperationalError::new(
                ErrorKind::Configuration,
                format!(
                    "--report destination overlaps a source file: {}",
                    target.path.display()
                ),
            ));
        }

        if baseline_path.as_ref() == Some(&destination) {
            return Err(OperationalError::new(
                ErrorKind::Configuration,
                format!(
                    "--report destination overlaps the active baseline: {}",
                    target.path.display()
                ),
            ));
        }

        targets.push(target);
    }

    Ok(targets)
}

pub(crate) fn resolve_administrative_json_targets(
    specs: &[String],
    protected_paths: &[PathBuf],
    operation: &str,
) -> Result<Vec<ReportTarget>, OperationalError> {
    let mut targets = Vec::with_capacity(specs.len());
    let mut destinations = BTreeSet::new();
    let protected_paths = protected_paths
        .iter()
        .map(|path| comparison_path(path))
        .collect::<Result<BTreeSet<_>, _>>()?;

    for spec in specs {
        let target = parse_report_target(spec)?;

        if target.format != OutputFormat::Json {
            return Err(OperationalError::new(
                ErrorKind::Configuration,
                format!("{operation} supports only json=PATH supplemental reports"),
            ));
        }

        let destination = comparison_path(&target.path)?;

        if !destinations.insert(destination.clone()) {
            return Err(OperationalError::new(
                ErrorKind::Configuration,
                format!("duplicate --report destination: {}", target.path.display()),
            ));
        }

        if protected_paths.contains(&destination) {
            return Err(OperationalError::new(
                ErrorKind::Configuration,
                format!(
                    "--report destination overlaps an input path: {}",
                    target.path.display()
                ),
            ));
        }

        targets.push(target);
    }

    Ok(targets)
}

pub(crate) fn write_report_targets(
    targets: &[ReportTarget],
    report: &Report,
    project_root: &Path,
) -> Result<(), OperationalError> {
    let rendered = render_report_targets(targets, report)?;

    for target in rendered {
        write_atomic_output(&target.path, &target.contents, project_root)?;
    }

    Ok(())
}

pub(crate) fn write_atomic_output(
    path: &Path,
    contents: &str,
    project_root: &Path,
) -> Result<(), OperationalError> {
    let mut file = AtomicWriteFile::open(path).map_err(|source| {
        OperationalError::new(
            ErrorKind::Output,
            format!(
                "failed to open report destination {}: {source}",
                path.display()
            ),
        )
        .with_project_path(path, project_root)
    })?;

    file.write_all(contents.as_bytes()).map_err(|source| {
        OperationalError::new(
            ErrorKind::Output,
            format!(
                "failed to write report destination {}: {source}",
                path.display()
            ),
        )
        .with_project_path(path, project_root)
    })?;

    file.commit().map_err(|source| {
        OperationalError::new(
            ErrorKind::Output,
            format!(
                "failed to replace report destination {}: {source}",
                path.display()
            ),
        )
        .with_project_path(path, project_root)
    })?;

    Ok(())
}

fn parse_report_target(spec: &str) -> Result<ReportTarget, OperationalError> {
    let (format, path) = spec.split_once('=').ok_or_else(|| {
        OperationalError::new(
            ErrorKind::Configuration,
            format!("invalid --report {spec:?}; expected FORMAT=PATH"),
        )
    })?;

    let format = match format {
        "text" => OutputFormat::Text,
        "json" => OutputFormat::Json,
        "markdown" => OutputFormat::Markdown,
        "sarif" => OutputFormat::Sarif,
        _ => {
            return Err(OperationalError::new(
                ErrorKind::Configuration,
                format!(
                    "invalid --report format {format:?}; expected text, json, markdown, or sarif"
                ),
            ));
        }
    };

    if path.is_empty() {
        return Err(OperationalError::new(
            ErrorKind::Configuration,
            "--report destination path must not be empty",
        ));
    }

    Ok(ReportTarget {
        format,
        path: PathBuf::from(path),
    })
}

fn render_report_targets(
    targets: &[ReportTarget],
    report: &Report,
) -> Result<Vec<RenderedTarget>, OperationalError> {
    let mut rendered = Vec::with_capacity(targets.len());

    for target in targets {
        if target.format == OutputFormat::Sarif && !report.complete {
            continue;
        }

        let contents = match target.format {
            OutputFormat::Text => render_text_plain(report),
            OutputFormat::Json => render_json(report).map_err(|error| {
                OperationalError::new(
                    ErrorKind::Output,
                    format!("failed to render supplemental JSON report: {error}"),
                )
            })?,
            OutputFormat::Markdown => render_markdown(report),
            OutputFormat::Sarif => render_sarif(report).map_err(|error| {
                OperationalError::new(
                    ErrorKind::Output,
                    format!("failed to render supplemental SARIF report: {error}"),
                )
            })?,
        };

        rendered.push(RenderedTarget {
            path: target.path.clone(),
            contents,
        });
    }

    Ok(rendered)
}

fn comparison_path(path: &Path) -> Result<PathBuf, OperationalError> {
    if path.exists() {
        return fs::canonicalize(path).map_err(|source| {
            OperationalError::new(
                ErrorKind::Configuration,
                format!("failed to resolve path {}: {source}", path.display()),
            )
        });
    }

    if let (Some(parent), Some(name)) = (path.parent(), path.file_name())
        && !parent.as_os_str().is_empty()
        && parent.exists()
    {
        let parent = fs::canonicalize(parent).map_err(|source| {
            OperationalError::new(
                ErrorKind::Configuration,
                format!("failed to resolve path {}: {source}", parent.display()),
            )
        })?;
        return Ok(parent.join(name));
    }

    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|source| {
                OperationalError::new(
                    ErrorKind::Configuration,
                    format!("failed to resolve current directory: {source}"),
                )
            })?
            .join(path)
    };

    Ok(normalize_lexically(&absolute))
}

fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }

    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_report_targets() {
        for (spec, format, path) in [
            ("text=out.txt", OutputFormat::Text, "out.txt"),
            ("json=out.json", OutputFormat::Json, "out.json"),
            ("markdown=out.md", OutputFormat::Markdown, "out.md"),
            ("sarif=out.sarif", OutputFormat::Sarif, "out.sarif"),
        ] {
            let target = parse_report_target(spec).unwrap();
            assert_eq!(target.format, format);
            assert_eq!(target.path, PathBuf::from(path));
        }
    }

    #[test]
    fn rejects_invalid_report_target_syntax() {
        assert!(parse_report_target("json").is_err());
        assert!(parse_report_target("xml=out.xml").is_err());
        assert!(parse_report_target("json=").is_err());
    }

    #[test]
    fn administrative_targets_are_json_only() {
        let error = resolve_administrative_json_targets(
            &["text=out.txt".to_owned()],
            &[],
            "--suppression-status",
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("supports only json=PATH supplemental reports")
        );
    }
}
