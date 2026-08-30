use std::fmt::Write as _;
use std::path::Path;

use serde::Serialize;
use thiserror::Error;

use crate::config::Settings;
use crate::corpus::{Corpus, CorpusError, build_corpus};
use crate::detect::{DetectError, detect_duplicates};
use crate::model::{Occurrence, PreparedFile};
use crate::project_path::project_relative_path;
use crate::python::{SuppressionEvent, SuppressionKind};

pub(crate) const SUPPRESSION_STATUS_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SuppressionEnd {
    Enable { line: u32 },
    Eof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SuppressionRegion {
    pub(crate) disable_line: u32,
    pub(crate) end: SuppressionEnd,
}

#[derive(Debug)]
pub(crate) struct AuditPreparedFile {
    pub(crate) file: PreparedFile,
    pub(crate) suppressions: Vec<SuppressionRegion>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SuppressionTermination {
    Enable,
    Eof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SuppressionStatusKind {
    Active,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct SuppressionAnalysis {
    min_lines: u32,
    ignore_comments: bool,
    ignore_docstrings: bool,
    ignore_imports: bool,
    ignore_signatures: bool,
    same_file: bool,
    hidden: bool,
    exclude: Vec<String>,
    ignore_files: bool,
}

impl SuppressionAnalysis {
    fn from_settings(settings: &Settings, ignore_files: bool) -> Self {
        Self {
            min_lines: settings.min_lines,
            ignore_comments: settings.ignore_comments,
            ignore_docstrings: settings.ignore_docstrings,
            ignore_imports: settings.ignore_imports,
            ignore_signatures: settings.ignore_signatures,
            same_file: settings.same_file,
            hidden: settings.hidden,
            exclude: settings.exclude.clone(),
            ignore_files,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub(crate) struct SuppressionSummary {
    total: u64,
    active: u64,
    stale: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct SuppressionStatusRegion {
    path: String,
    start_line: u32,
    end_line: Option<u32>,
    termination: SuppressionTermination,
    status: SuppressionStatusKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct SuppressionStatus {
    schema_version: u8,
    tool_version: String,
    files: u64,
    analysis: SuppressionAnalysis,
    summary: SuppressionSummary,
    regions: Vec<SuppressionStatusRegion>,
}

impl SuppressionStatus {
    pub(crate) fn has_stale(&self) -> bool {
        self.summary.stale > 0
    }
}

#[derive(Debug, Error)]
pub(crate) enum SuppressionAuditError {
    #[error(transparent)]
    Corpus(#[from] CorpusError),

    #[error(transparent)]
    Detect(#[from] DetectError),

    #[error(
        "duplicate occurrence for file {file} at normalized range {start}..{end} \
         cannot be mapped to source lines"
    )]
    InvalidOccurrence { file: u32, start: u32, end: u32 },
}

pub(crate) fn derive_suppression_regions(
    source: &str,
    events: &[SuppressionEvent],
) -> Vec<SuppressionRegion> {
    debug_assert!(
        events
            .windows(2)
            .all(|pair| pair[0].offset <= pair[1].offset)
    );

    let mut regions = Vec::new();
    let mut open_disable = None;
    let mut event_index = 0_usize;
    let mut line_start = 0_usize;

    for (line_index, raw_line) in source.split_inclusive('\n').enumerate() {
        let line_end = line_start + raw_line.len();
        let line = u32::try_from(line_index + 1)
            .expect("source cannot contain more than u32::MAX physical lines");

        while let Some(event) = events.get(event_index) {
            if event.offset >= line_end {
                break;
            }

            match (open_disable, event.kind) {
                (None, SuppressionKind::Disable) => {
                    open_disable = Some(line);
                }
                (Some(disable_line), SuppressionKind::Enable) => {
                    regions.push(SuppressionRegion {
                        disable_line,
                        end: SuppressionEnd::Enable { line },
                    });
                    open_disable = None;
                }
                _ => {}
            }

            event_index += 1;
        }

        line_start = line_end;
    }

    debug_assert_eq!(event_index, events.len());

    if let Some(disable_line) = open_disable {
        regions.push(SuppressionRegion {
            disable_line,
            end: SuppressionEnd::Eof,
        });
    }

    regions
}

pub(crate) fn build_suppression_status(
    mut prepared: Vec<AuditPreparedFile>,
    settings: &Settings,
    project_root: &Path,
    ignore_files: bool,
) -> Result<SuppressionStatus, SuppressionAuditError> {
    prepared.sort_by(|left, right| left.file.path.cmp(&right.file.path));

    let suppression_sets = prepared
        .iter()
        .map(|prepared| prepared.suppressions.clone())
        .collect::<Vec<_>>();
    let files = prepared
        .into_iter()
        .map(|prepared| prepared.file)
        .collect::<Vec<_>>();

    let corpus = build_corpus(files)?;
    let groups = detect_duplicates(&corpus, settings.detection_options())?;
    let mut active = suppression_sets
        .iter()
        .map(|regions| vec![false; regions.len()])
        .collect::<Vec<_>>();

    for group in &groups {
        for occurrence in &group.occurrences {
            mark_active_regions(&corpus, *occurrence, &suppression_sets, &mut active)?;
        }
    }

    let mut regions = Vec::new();

    for (file_index, source_regions) in suppression_sets.iter().enumerate() {
        let file = &corpus.files[file_index];
        let path = project_relative_path(&file.path, project_root)
            .unwrap_or_else(|_| file.path.to_string_lossy().into_owned());

        for (region_index, region) in source_regions.iter().enumerate() {
            let (end_line, termination) = match region.end {
                SuppressionEnd::Enable { line } => (Some(line), SuppressionTermination::Enable),
                SuppressionEnd::Eof => (None, SuppressionTermination::Eof),
            };

            regions.push(SuppressionStatusRegion {
                path: path.clone(),
                start_line: region.disable_line,
                end_line,
                termination,
                status: if active[file_index][region_index] {
                    SuppressionStatusKind::Active
                } else {
                    SuppressionStatusKind::Stale
                },
            });
        }
    }

    regions.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.start_line.cmp(&right.start_line))
            .then_with(|| {
                left.end_line
                    .unwrap_or(u32::MAX)
                    .cmp(&right.end_line.unwrap_or(u32::MAX))
            })
    });

    let summary = regions
        .iter()
        .fold(SuppressionSummary::default(), |mut summary, region| {
            summary.total += 1;
            match region.status {
                SuppressionStatusKind::Active => summary.active += 1,
                SuppressionStatusKind::Stale => summary.stale += 1,
            }
            summary
        });

    Ok(SuppressionStatus {
        schema_version: SUPPRESSION_STATUS_SCHEMA_VERSION,
        tool_version: env!("CARGO_PKG_VERSION").to_owned(),
        files: corpus.files.len() as u64,
        analysis: SuppressionAnalysis::from_settings(settings, ignore_files),
        summary,
        regions,
    })
}

fn mark_active_regions(
    corpus: &Corpus,
    occurrence: Occurrence,
    suppression_sets: &[Vec<SuppressionRegion>],
    active: &mut [Vec<bool>],
) -> Result<(), SuppressionAuditError> {
    let file_index = occurrence.file as usize;
    let Some(file) = corpus.files.get(file_index) else {
        return Err(invalid_occurrence(occurrence));
    };
    let Some(regions) = suppression_sets.get(file_index) else {
        return Err(invalid_occurrence(occurrence));
    };
    let Some(active_regions) = active.get_mut(file_index) else {
        return Err(invalid_occurrence(occurrence));
    };

    let start = occurrence.normalized_start as usize;
    let Some(end) = start.checked_add(occurrence.normalized_len as usize) else {
        return Err(invalid_occurrence(occurrence));
    };
    let Some(lines) = file.lines.get(start..end) else {
        return Err(invalid_occurrence(occurrence));
    };
    let Some(first) = lines.first() else {
        return Err(invalid_occurrence(occurrence));
    };
    let Some(last) = lines.last() else {
        return Err(invalid_occurrence(occurrence));
    };

    let physical_start = first
        .source_line
        .checked_add(1)
        .ok_or_else(|| invalid_occurrence(occurrence))?;
    let physical_end = last
        .source_line
        .checked_add(1)
        .ok_or_else(|| invalid_occurrence(occurrence))?;

    for (region, is_active) in regions.iter().zip(active_regions) {
        if occurrence_is_affected(physical_start, physical_end, *region) {
            *is_active = true;
        }
    }

    Ok(())
}

fn invalid_occurrence(occurrence: Occurrence) -> SuppressionAuditError {
    SuppressionAuditError::InvalidOccurrence {
        file: occurrence.file,
        start: occurrence.normalized_start,
        end: occurrence
            .normalized_start
            .saturating_add(occurrence.normalized_len),
    }
}

fn occurrence_is_affected(start_line: u32, end_line: u32, region: SuppressionRegion) -> bool {
    if end_line <= region.disable_line {
        return false;
    }

    match region.end {
        SuppressionEnd::Enable { line } => start_line <= line,
        SuppressionEnd::Eof => true,
    }
}

pub(crate) fn render_suppression_status_text(status: &SuppressionStatus) -> String {
    let mut output = String::new();

    output.push_str("Suppression status\n");
    writeln!(&mut output, "Files: {}", status.files).expect("writing to String cannot fail");
    writeln!(&mut output, "Total suppressions: {}", status.summary.total)
        .expect("writing to String cannot fail");
    writeln!(
        &mut output,
        "Active suppressions: {}",
        status.summary.active
    )
    .expect("writing to String cannot fail");
    writeln!(&mut output, "Stale suppressions: {}", status.summary.stale)
        .expect("writing to String cannot fail");

    for region in &status.regions {
        let state = match region.status {
            SuppressionStatusKind::Active => "active",
            SuppressionStatusKind::Stale => "stale",
        };
        let location = match region.end_line {
            Some(end_line) => format!("{}-{}", region.start_line, end_line),
            None => format!("{}-EOF", region.start_line),
        };
        let termination = match region.termination {
            SuppressionTermination::Enable => "enable",
            SuppressionTermination::Eof => "eof",
        };

        writeln!(
            &mut output,
            "{state}: {}:{location} ({termination})",
            region.path
        )
        .expect("writing to String cannot fail");
    }

    output
}

pub(crate) fn render_suppression_status_json(
    status: &SuppressionStatus,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(status).map(|mut output| {
        output.push('\n');
        output
    })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::model::NormalizationOptions;
    use crate::normalize::{SuppressionMode, prepare_file_with_mode};
    use crate::python;

    fn regions(source: &str) -> Vec<SuppressionRegion> {
        let analysis = python::analyze(source, NormalizationOptions::default()).unwrap();
        derive_suppression_regions(source, &analysis.suppressions)
    }

    fn audit_file(path: &str, source: &str) -> AuditPreparedFile {
        let (file, suppressions) = prepare_file_with_mode(
            PathBuf::from(path),
            source.to_owned(),
            NormalizationOptions::default(),
            SuppressionMode::Audit,
        )
        .unwrap();

        AuditPreparedFile { file, suppressions }
    }

    fn settings() -> Settings {
        Settings {
            min_lines: 2,
            ..Settings::default()
        }
    }

    #[test]
    fn derives_enable_terminated_region() {
        let source = "before()\n# arid: disable\nhidden()\n# arid: enable\nafter()\n";

        assert_eq!(
            regions(source),
            vec![SuppressionRegion {
                disable_line: 2,
                end: SuppressionEnd::Enable { line: 4 },
            }]
        );
    }

    #[test]
    fn derives_eof_terminated_region() {
        let source = "before()\n# arid: disable\nhidden()\n";

        assert_eq!(
            regions(source),
            vec![SuppressionRegion {
                disable_line: 2,
                end: SuppressionEnd::Eof,
            }]
        );
    }

    #[test]
    fn repeated_same_state_directives_are_noops() {
        let source = concat!(
            "# arid: enable\n",
            "# arid: disable\n",
            "# arid: disable\n",
            "# arid: enable\n",
            "# arid: enable\n",
        );

        assert_eq!(
            regions(source),
            vec![SuppressionRegion {
                disable_line: 2,
                end: SuppressionEnd::Enable { line: 4 },
            }]
        );
    }

    #[test]
    fn derives_multiple_effective_regions() {
        let source = concat!(
            "# arid: disable\n",
            "first()\n",
            "# arid: enable\n",
            "middle()\n",
            "# arid: disable\n",
            "second()\n",
        );

        assert_eq!(
            regions(source),
            vec![
                SuppressionRegion {
                    disable_line: 1,
                    end: SuppressionEnd::Enable { line: 3 },
                },
                SuppressionRegion {
                    disable_line: 5,
                    end: SuppressionEnd::Eof,
                },
            ]
        );
    }

    #[test]
    fn inline_directive_uses_its_physical_line() {
        let source = "kept()  # arid: disable\nhidden()\n# arid: enable\n";

        assert_eq!(
            regions(source),
            vec![SuppressionRegion {
                disable_line: 1,
                end: SuppressionEnd::Enable { line: 3 },
            }]
        );
    }

    #[test]
    fn classifies_active_and_stale_regions() {
        let status = build_suppression_status(
            vec![
                audit_file("project/a.py", "alpha = 1\nbeta = 2\n"),
                audit_file(
                    "project/b.py",
                    "# arid: disable\nalpha = 1\nbeta = 2\n# arid: enable\n",
                ),
                audit_file(
                    "project/c.py",
                    "# arid: disable\ngamma = 3\ndelta = 4\n# arid: enable\n",
                ),
            ],
            &settings(),
            Path::new("project"),
            true,
        )
        .unwrap();

        assert_eq!(status.summary.total, 2);
        assert_eq!(status.summary.active, 1);
        assert_eq!(status.summary.stale, 1);
        assert!(status.has_stale());
        assert_eq!(status.regions[0].path, "b.py");
        assert_eq!(status.regions[0].status, SuppressionStatusKind::Active);
        assert_eq!(status.regions[1].path, "c.py");
        assert_eq!(status.regions[1].status, SuppressionStatusKind::Stale);
    }

    #[test]
    fn suppressed_regions_can_activate_one_another() {
        let source = "# arid: disable\nalpha = 1\nbeta = 2\n# arid: enable\n";
        let status = build_suppression_status(
            vec![
                audit_file("project/a.py", source),
                audit_file("project/b.py", source),
            ],
            &settings(),
            Path::new("project"),
            true,
        )
        .unwrap();

        assert_eq!(status.summary.active, 2);
        assert_eq!(status.summary.stale, 0);
        assert!(!status.has_stale());
    }

    #[test]
    fn empty_region_is_active_when_its_boundary_breaks_a_duplicate() {
        let status = build_suppression_status(
            vec![
                audit_file("project/a.py", "alpha = 1\nbeta = 2\n"),
                audit_file(
                    "project/b.py",
                    "alpha = 1\n# arid: disable\n# arid: enable\nbeta = 2\n",
                ),
            ],
            &settings(),
            Path::new("project"),
            true,
        )
        .unwrap();

        assert_eq!(status.summary.active, 1);
        assert_eq!(status.regions[0].status, SuppressionStatusKind::Active);
    }

    #[test]
    fn eof_region_is_a_normal_stale_record() {
        let status = build_suppression_status(
            vec![audit_file(
                "project/a.py",
                "before = 1\n# arid: disable\nunique = 2\n",
            )],
            &settings(),
            Path::new("project"),
            true,
        )
        .unwrap();

        assert_eq!(status.summary.stale, 1);
        assert_eq!(status.regions[0].end_line, None);
        assert_eq!(status.regions[0].termination, SuppressionTermination::Eof);
    }

    #[test]
    fn no_effective_regions_is_a_successful_empty_status() {
        let status = build_suppression_status(
            vec![audit_file("project/a.py", "alpha = 1\n")],
            &settings(),
            Path::new("project"),
            true,
        )
        .unwrap();

        assert_eq!(status.summary, SuppressionSummary::default());
        assert!(status.regions.is_empty());
        assert!(!status.has_stale());
    }

    #[test]
    fn text_lists_summary_and_regions() {
        let status = build_suppression_status(
            vec![audit_file(
                "project/a.py",
                "# arid: disable\nunique = 1\n# arid: enable\n",
            )],
            &settings(),
            Path::new("project"),
            true,
        )
        .unwrap();
        let rendered = render_suppression_status_text(&status);

        assert!(rendered.starts_with("Suppression status\n"));
        assert!(rendered.contains("Files: 1"));
        assert!(rendered.contains("Total suppressions: 1"));
        assert!(rendered.contains("Stale suppressions: 1"));
        assert!(rendered.contains("stale: a.py:1-3 (enable)"));
    }

    #[test]
    fn json_has_versioned_deterministic_shape() {
        let status = build_suppression_status(
            vec![audit_file("project/a.py", "# arid: disable\nunique = 1\n")],
            &settings(),
            Path::new("project"),
            true,
        )
        .unwrap();
        let rendered = render_suppression_status_json(&status).unwrap();
        assert!(rendered.ends_with('\n'));
        let value: serde_json::Value = serde_json::from_str(&rendered).unwrap();

        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["tool_version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(value["files"], 1);
        assert_eq!(value["analysis"]["min_lines"], 2);
        assert_eq!(value["analysis"]["ignore_files"], true);
        assert_eq!(value["summary"]["total"], 1);
        assert_eq!(value["summary"]["stale"], 1);
        assert_eq!(value["regions"][0]["path"], "a.py");
        assert_eq!(value["regions"][0]["end_line"], serde_json::Value::Null);
        assert_eq!(value["regions"][0]["termination"], "eof");
        assert_eq!(value["regions"][0]["status"], "stale");
    }
}
