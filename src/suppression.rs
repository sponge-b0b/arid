use crate::python::{SuppressionEvent, SuppressionKind};

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

pub(crate) fn derive_suppression_regions(
    source: &str,
    events: &[SuppressionEvent],
) -> Vec<SuppressionRegion> {
    debug_assert!(events.windows(2).all(|pair| pair[0].offset <= pair[1].offset));

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::NormalizationOptions;
    use crate::python;

    fn regions(source: &str) -> Vec<SuppressionRegion> {
        let analysis = python::analyze(source, NormalizationOptions::default()).unwrap();
        derive_suppression_regions(source, &analysis.suppressions)
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
}
