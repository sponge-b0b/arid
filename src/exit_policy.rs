use crate::outcome::ExitStatus;

pub(crate) const fn apply_no_fail_on_findings(
    status: ExitStatus,
    no_fail_on_findings: bool,
) -> ExitStatus {
    if no_fail_on_findings && matches!(status, ExitStatus::Findings) {
        ExitStatus::Success
    } else {
        status
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_fail_only_maps_findings_to_success() {
        assert_eq!(
            apply_no_fail_on_findings(ExitStatus::Findings, true),
            ExitStatus::Success
        );
        assert_eq!(
            apply_no_fail_on_findings(ExitStatus::Success, true),
            ExitStatus::Success
        );
        assert_eq!(
            apply_no_fail_on_findings(ExitStatus::Error, true),
            ExitStatus::Error
        );
    }

    #[test]
    fn default_policy_preserves_all_statuses() {
        for status in [
            ExitStatus::Success,
            ExitStatus::Findings,
            ExitStatus::Error,
        ] {
            assert_eq!(apply_no_fail_on_findings(status, false), status);
        }
    }
}
