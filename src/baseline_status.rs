use std::fmt::Write as _;

use serde::Serialize;

pub(crate) const BASELINE_STATUS_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub(crate) struct DebtCounts {
    pub(crate) accepted: u64,
    pub(crate) active: u64,
    pub(crate) stale: u64,
}

impl DebtCounts {
    pub(crate) fn add(&mut self, other: Self) {
        self.accepted += other.accepted;
        self.active += other.active;
        self.stale += other.stale;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct BaselineStatus {
    schema_version: u8,
    pub(crate) groups: Vec<BaselineStatusGroup>,
    pub(crate) summary: DebtCounts,
}

impl BaselineStatus {
    pub(crate) fn new(groups: Vec<BaselineStatusGroup>) -> Self {
        let summary = groups
            .iter()
            .fold(DebtCounts::default(), |mut total, group| {
                total.add(group.summary);
                total
            });

        Self {
            schema_version: BASELINE_STATUS_SCHEMA_VERSION,
            groups,
            summary,
        }
    }

    pub(crate) fn has_active(&self) -> bool {
        self.summary.active > 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct BaselineStatusGroup {
    pub(crate) fingerprint: String,
    pub(crate) lines: u32,
    pub(crate) paths: Vec<BaselineStatusPath>,
    pub(crate) summary: DebtCounts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct BaselineStatusPath {
    pub(crate) path: String,
    pub(crate) accepted: u64,
    pub(crate) active: u64,
    pub(crate) stale: u64,
}

pub(crate) fn render_baseline_status_text(status: &BaselineStatus) -> String {
    let mut output = String::new();

    output.push_str("Baseline status\n");
    writeln!(
        &mut output,
        "Accepted occurrences: {}",
        status.summary.accepted
    )
    .expect("writing to String cannot fail");
    writeln!(&mut output, "Active occurrences: {}", status.summary.active)
        .expect("writing to String cannot fail");
    writeln!(&mut output, "Stale occurrences: {}", status.summary.stale)
        .expect("writing to String cannot fail");

    for group in &status.groups {
        writeln!(
            &mut output,
            "\n{} ({} lines)",
            group.fingerprint, group.lines
        )
        .expect("writing to String cannot fail");

        for path in &group.paths {
            writeln!(
                &mut output,
                "  {}: accepted {}, active {}, stale {}",
                path.path, path.accepted, path.active, path.stale
            )
            .expect("writing to String cannot fail");
        }
    }

    output
}

pub(crate) fn render_baseline_status_json(
    status: &BaselineStatus,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status() -> BaselineStatus {
        BaselineStatus::new(vec![BaselineStatusGroup {
            fingerprint: format!("sha256:{}", "0".repeat(64)),
            lines: 2,
            paths: vec![
                BaselineStatusPath {
                    path: "a.py".to_owned(),
                    accepted: 1,
                    active: 0,
                    stale: 1,
                },
                BaselineStatusPath {
                    path: "b.py".to_owned(),
                    accepted: 1,
                    active: 1,
                    stale: 0,
                },
            ],
            summary: DebtCounts {
                accepted: 2,
                active: 1,
                stale: 1,
            },
        }])
    }

    #[test]
    fn status_summary_controls_active_state() {
        assert!(status().has_active());

        let empty = BaselineStatus::new(Vec::new());
        assert!(!empty.has_active());
        assert_eq!(empty.summary, DebtCounts::default());
    }

    #[test]
    fn text_lists_totals_and_path_states() {
        let rendered = render_baseline_status_text(&status());

        assert!(rendered.starts_with("Baseline status\n"));
        assert!(rendered.contains("Accepted occurrences: 2"));
        assert!(rendered.contains("Active occurrences: 1"));
        assert!(rendered.contains("Stale occurrences: 1"));
        assert!(rendered.contains("a.py: accepted 1, active 0, stale 1"));
        assert!(rendered.contains("b.py: accepted 1, active 1, stale 0"));
    }

    #[test]
    fn json_has_versioned_deterministic_shape() {
        let value: serde_json::Value =
            serde_json::from_str(&render_baseline_status_json(&status()).unwrap()).unwrap();

        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["summary"]["accepted"], 2);
        assert_eq!(value["summary"]["active"], 1);
        assert_eq!(value["summary"]["stale"], 1);
        assert_eq!(value["groups"][0]["paths"][0]["path"], "a.py");
    }
}
