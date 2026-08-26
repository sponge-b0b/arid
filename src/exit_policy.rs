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

pub(crate) const fn apply_fail_on_stale(
    status: ExitStatus,
    has_stale: bool,
    fail_on_stale: bool,
) -> ExitStatus {
    if fail_on_stale && has_stale && !matches!(status, ExitStatus::Error) {
        ExitStatus::Findings
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
        for status in [ExitStatus::Success, ExitStatus::Findings, ExitStatus::Error] {
            assert_eq!(apply_no_fail_on_findings(status, false), status);
        }
    }

    #[test]
    fn fail_on_stale_promotes_success_to_findings() {
        assert_eq!(
            apply_fail_on_stale(ExitStatus::Success, true, true),
            ExitStatus::Findings
        );
    }

    #[test]
    fn fail_on_stale_preserves_existing_findings() {
        assert_eq!(
            apply_fail_on_stale(ExitStatus::Findings, true, true),
            ExitStatus::Findings
        );
        assert_eq!(
            apply_fail_on_stale(ExitStatus::Findings, false, true),
            ExitStatus::Findings
        );
    }

    #[test]
    fn fail_on_stale_never_masks_errors() {
        assert_eq!(
            apply_fail_on_stale(ExitStatus::Error, true, true),
            ExitStatus::Error
        );
    }

    #[test]
    fn stale_policy_is_informational_without_flag() {
        assert_eq!(
            apply_fail_on_stale(ExitStatus::Success, true, false),
            ExitStatus::Success
        );
    }
}
