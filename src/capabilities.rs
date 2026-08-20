use serde::Serialize;

const CAPABILITIES_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Capabilities {
    schema_version: u8,
    tool_version: &'static str,
    report_schema_versions: Vec<u8>,
    baseline_schema_versions: Vec<u8>,
    error_schema_versions: Vec<u8>,
    finding_fingerprint_versions: Vec<u8>,
    formats: Vec<&'static str>,
    features: Vec<&'static str>,
}

pub(crate) fn build_capabilities() -> Capabilities {
    Capabilities {
        schema_version: CAPABILITIES_SCHEMA_VERSION,
        tool_version: env!("CARGO_PKG_VERSION"),
        report_schema_versions: vec![4],
        baseline_schema_versions: vec![1],
        error_schema_versions: vec![1],
        finding_fingerprint_versions: vec![1],
        formats: vec!["json", "markdown", "sarif", "text"],
        features: vec![
            "baseline-prune",
            "baseline-status",
            "focus",
            "keep-going",
            "no-fail-on-findings",
            "stdin-path",
            "workers-auto",
        ],
    }
}

pub(crate) fn render_capabilities_json() -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&build_capabilities())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_match_v1_contract() {
        let capabilities = build_capabilities();

        assert_eq!(capabilities.schema_version, 1);
        assert_eq!(capabilities.tool_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(capabilities.report_schema_versions, vec![4]);
        assert_eq!(capabilities.baseline_schema_versions, vec![1]);
        assert_eq!(capabilities.error_schema_versions, vec![1]);
        assert_eq!(capabilities.finding_fingerprint_versions, vec![1]);
        assert_eq!(
            capabilities.formats,
            vec!["json", "markdown", "sarif", "text"]
        );
        assert_eq!(
            capabilities.features,
            vec![
                "baseline-prune",
                "baseline-status",
                "focus",
                "keep-going",
                "no-fail-on-findings",
                "stdin-path",
                "workers-auto",
            ]
        );
    }

    #[test]
    fn capability_string_arrays_are_lexically_sorted() {
        let capabilities = build_capabilities();
        let mut formats = capabilities.formats.clone();
        formats.sort_unstable();
        assert_eq!(capabilities.formats, formats);

        let mut features = capabilities.features.clone();
        features.sort_unstable();
        assert_eq!(capabilities.features, features);
    }

    #[test]
    fn capability_json_is_deterministic() {
        assert_eq!(
            render_capabilities_json().unwrap(),
            render_capabilities_json().unwrap()
        );
    }
}
