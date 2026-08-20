use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::config::LoadedSettings;
use crate::project_path::project_relative_path;

pub(crate) fn render_config_text(
    loaded: &LoadedSettings,
    no_config: bool,
    baseline_path: Option<&Path>,
) -> String {
    let settings = &loaded.settings;
    let mut output = String::new();

    writeln!(output, "Project root: {}", loaded.project_root.display()).unwrap();

    if no_config {
        writeln!(output, "Configuration: disabled (--no-config)").unwrap();
    } else if let Some(path) = &loaded.config_path {
        writeln!(output, "Configuration: {}", path.display()).unwrap();
    } else {
        writeln!(output, "Configuration: none").unwrap();
    }

    writeln!(output, "Min lines: {}", settings.min_lines).unwrap();
    writeln!(output, "Ignore comments: {}", settings.ignore_comments).unwrap();
    writeln!(output, "Ignore docstrings: {}", settings.ignore_docstrings).unwrap();
    writeln!(output, "Ignore imports: {}", settings.ignore_imports).unwrap();
    writeln!(output, "Ignore signatures: {}", settings.ignore_signatures).unwrap();
    writeln!(output, "Same-file detection: {}", settings.same_file).unwrap();
    writeln!(output, "Hidden files: {}", settings.hidden).unwrap();

    if settings.exclude.is_empty() {
        writeln!(output, "Exclude: none").unwrap();
    } else {
        writeln!(output, "Exclude:").unwrap();
        for pattern in &settings.exclude {
            writeln!(output, "  {pattern}").unwrap();
        }
    }

    if let Some(path) = baseline_path {
        writeln!(output, "Baseline: {}", path.display()).unwrap();
    } else {
        writeln!(output, "Baseline: disabled").unwrap();
    }

    output
}

pub(crate) fn render_config_json(
    loaded: &LoadedSettings,
    no_config: bool,
    baseline_path: Option<&Path>,
) -> Result<String, serde_json::Error> {
    let settings = &loaded.settings;
    let configuration_state = if no_config {
        "disabled"
    } else if loaded.config_path.is_some() {
        "file"
    } else {
        "none"
    };
    let configuration_path = loaded
        .config_path
        .as_ref()
        .map(|path| path.to_string_lossy().into_owned());
    let baseline_path = baseline_path.map(|path| path.to_string_lossy().into_owned());

    serde_json::to_string_pretty(&serde_json::json!({
        "schema_version": 1,
        "project_root": loaded.project_root.to_string_lossy(),
        "configuration": {
            "state": configuration_state,
            "path": configuration_path,
        },
        "settings": {
            "min_lines": settings.min_lines,
            "ignore_comments": settings.ignore_comments,
            "ignore_docstrings": settings.ignore_docstrings,
            "ignore_imports": settings.ignore_imports,
            "ignore_signatures": settings.ignore_signatures,
            "same_file": settings.same_file,
            "hidden": settings.hidden,
            "exclude": &settings.exclude,
            "baseline": baseline_path,
        },
    }))
}

pub(crate) fn discovered_file_names(paths: &[PathBuf], project_root: &Path) -> Vec<String> {
    let mut files = paths
        .iter()
        .map(|path| {
            project_relative_path(path, project_root)
                .unwrap_or_else(|_| path.to_string_lossy().into_owned())
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}

pub(crate) fn render_file_list_text(files: &[String]) -> String {
    if files.is_empty() {
        String::new()
    } else {
        format!("{}\n", files.join("\n"))
    }
}

pub(crate) fn render_file_list_json(files: &[String]) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(files)
}
