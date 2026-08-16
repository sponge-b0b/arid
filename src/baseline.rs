use serde::{Deserialize, Serialize};

use crate::model::NormalizationOptions;

pub const BASELINE_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Baseline {
    pub version: u8,
    pub normalization: BaselineNormalization,
    pub groups: Vec<BaselineGroup>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaselineNormalization {
    pub ignore_comments: bool,
    pub ignore_docstrings: bool,
    pub ignore_imports: bool,
    pub ignore_signatures: bool,
}

impl From<NormalizationOptions> for BaselineNormalization {
    fn from(options: NormalizationOptions) -> Self {
        Self {
            ignore_comments: options.ignore_comments,
            ignore_docstrings: options.ignore_docstrings,
            ignore_imports: options.ignore_imports,
            ignore_signatures: options.ignore_signatures,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BaselineGroup {
    pub fingerprint: String,
    pub lines: u32,
    pub occurrences: Vec<BaselinePathCount>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BaselinePathCount {
    pub path: String,
    pub count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_snapshot_contains_only_identity_settings() {
        let snapshot = BaselineNormalization::from(NormalizationOptions {
            ignore_comments: false,
            ignore_docstrings: true,
            ignore_imports: false,
            ignore_signatures: true,
        });

        assert_eq!(
            snapshot,
            BaselineNormalization {
                ignore_comments: false,
                ignore_docstrings: true,
                ignore_imports: false,
                ignore_signatures: true,
            }
        );
    }

    #[test]
    fn schema_serializes_to_expected_shape() {
        let baseline = Baseline {
            version: BASELINE_SCHEMA_VERSION,
            normalization: BaselineNormalization {
                ignore_comments: true,
                ignore_docstrings: true,
                ignore_imports: true,
                ignore_signatures: true,
            },
            groups: vec![BaselineGroup {
                fingerprint: "sha256:example".to_owned(),
                lines: 9,
                occurrences: vec![
                    BaselinePathCount {
                        path: "src/a.py".to_owned(),
                        count: 1,
                    },
                    BaselinePathCount {
                        path: "src/b.py".to_owned(),
                        count: 2,
                    },
                ],
            }],
        };

        assert_eq!(
            serde_json::to_string_pretty(&baseline).unwrap(),
            concat!(
                "{\n",
                "  \"version\": 1,\n",
                "  \"normalization\": {\n",
                "    \"ignore_comments\": true,\n",
                "    \"ignore_docstrings\": true,\n",
                "    \"ignore_imports\": true,\n",
                "    \"ignore_signatures\": true\n",
                "  },\n",
                "  \"groups\": [\n",
                "    {\n",
                "      \"fingerprint\": \"sha256:example\",\n",
                "      \"lines\": 9,\n",
                "      \"occurrences\": [\n",
                "        {\n",
                "          \"path\": \"src/a.py\",\n",
                "          \"count\": 1\n",
                "        },\n",
                "        {\n",
                "          \"path\": \"src/b.py\",\n",
                "          \"count\": 2\n",
                "        }\n",
                "      ]\n",
                "    }\n",
                "  ]\n",
                "}"
            )
        );
    }
}
