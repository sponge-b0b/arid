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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunResult {
    stdout: String,
    stderr: String,
    exit_status: ExitStatus,
}

impl RunResult {
    pub(crate) fn new(
        stdout: impl Into<String>,
        stderr: impl Into<String>,
        exit_status: ExitStatus,
    ) -> Self {
        Self {
            stdout: stdout.into(),
            stderr: stderr.into(),
            exit_status,
        }
    }

    pub(crate) fn failure(message: impl AsRef<str>) -> Self {
        Self::new(
            "",
            format!("error: {}\n", message.as_ref()),
            ExitStatus::Error,
        )
    }

    #[must_use]
    pub fn stdout(&self) -> &str {
        &self.stdout
    }

    #[must_use]
    pub fn stderr(&self) -> &str {
        &self.stderr
    }

    #[must_use]
    pub const fn exit_status(&self) -> ExitStatus {
        self.exit_status
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

    #[test]
    fn process_result_exposes_streams_and_status() {
        let result = RunResult::new("out", "err", ExitStatus::Findings);

        assert_eq!(result.stdout(), "out");
        assert_eq!(result.stderr(), "err");
        assert_eq!(result.exit_status(), ExitStatus::Findings);
    }

    #[test]
    fn failure_is_a_deterministic_error_outcome() {
        let result = RunResult::failure("broken");

        assert_eq!(result.stdout(), "");
        assert_eq!(result.stderr(), "error: broken\n");
        assert_eq!(result.exit_status(), ExitStatus::Error);
    }
}
