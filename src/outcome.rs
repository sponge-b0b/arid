#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExitStatus {
    Success = 0,
    Findings = 1,
    Error = 2,
}

impl ExitStatus {
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_status_codes_match_cli_contract() {
        assert_eq!(ExitStatus::Success.code(), 0);
        assert_eq!(ExitStatus::Findings.code(), 1);
        assert_eq!(ExitStatus::Error.code(), 2);
    }
}
