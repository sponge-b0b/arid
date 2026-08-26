//! Supported Rust API for embedding Arid.
//!
//! Arid remains primarily a CLI application. The supported Rust surface is
//! intentionally small and available directly from the crate root.
//!
//! ```no_run
//! use arid::{
//!     Cli, ColorEnvironment, ExitStatus, RunContext, RunResult, run, run_with_context,
//! };
//! use clap::Parser;
//!
//! let cli = Cli::parse_from(["arid", "--capabilities"]);
//! let result: RunResult = run(&cli);
//! let _ = (result.stdout(), result.stderr(), result.exit_status());
//!
//! let context = RunContext::new(false, ColorEnvironment::default());
//! let contextual = run_with_context(&cli, context);
//! assert_eq!(contextual.exit_status(), ExitStatus::Success);
//! ```
//!
//! Implementation modules are intentionally not part of the supported API:
//!
//! ```compile_fail
//! use arid::report::Report;
//! ```
//! ```compile_fail
//! use arid::cli::Cli;
//! ```

use std::time::{Duration, Instant};

mod app;
mod baseline;
mod baseline_filter;
mod baseline_status;
mod capabilities;
mod cli;
mod config;
mod corpus;
mod detect;
mod error;
mod exit_policy;
mod files;
mod fingerprint;
mod introspection;
mod markdown;
mod metrics;
mod model;
mod normalize;
mod outcome;
mod output;
mod path_explanation;
mod project_path;
mod python;
mod report;
mod sarif;
mod source;
mod suffix;
mod suppression;
mod suppression_command;
mod text;

pub use app::{ColorEnvironment, RunContext};
pub use cli::Cli;
pub use outcome::{ExitStatus, RunResult};

/// Runs one complete Arid invocation with deterministic non-terminal defaults.
#[must_use]
pub fn run(cli: &Cli) -> RunResult {
    if cli.explain_path.is_some() {
        path_explanation::run(cli)
    } else if cli.suppression_status {
        suppression_command::run(cli)
    } else {
        run_timed_scan(cli, || app::run(cli))
    }
}

/// Runs one complete Arid invocation with explicit output-environment context.
#[must_use]
pub fn run_with_context(cli: &Cli, context: RunContext) -> RunResult {
    if cli.explain_path.is_some() {
        path_explanation::run(cli)
    } else if cli.suppression_status {
        suppression_command::run(cli)
    } else {
        run_timed_scan(cli, || app::run_with_context(cli, context))
    }
}

fn run_timed_scan(cli: &Cli, operation: impl FnOnce() -> RunResult) -> RunResult {
    if !should_time_scan(cli) {
        return operation();
    }

    let started = Instant::now();
    let result = operation();

    if result.exit_status() == ExitStatus::Error {
        return result;
    }

    let elapsed = started.elapsed();
    let mut stdout = result.stdout().to_owned();

    if !stdout.ends_with('\n') {
        stdout.push('\n');
    }
    stdout.push('\n');
    stdout.push_str("Total time: ");
    stdout.push_str(&format_elapsed(elapsed));
    stdout.push('\n');

    RunResult::new(stdout, result.stderr(), result.exit_status())
}

fn should_time_scan(cli: &Cli) -> bool {
    cli.output_format() == cli::OutputFormat::Text
        && !cli.capabilities
        && !cli.show_config
        && !cli.list_files
        && !cli.suppression_status
        && cli.explain_path.is_none()
        && cli.baseline_status.is_none()
        && cli.prune_baseline.is_none()
        && cli.write_baseline.is_none()
}

fn format_elapsed(duration: Duration) -> String {
    if duration < Duration::from_millis(1) {
        return format!("{} µs", duration.as_micros());
    }

    if duration < Duration::from_secs(1) {
        let millis = duration.as_secs_f64() * 1_000.0;

        if millis >= 999.95 {
            return "1.00 s".to_owned();
        }

        return if millis < 10.0 {
            format!("{millis:.2} ms")
        } else {
            format!("{millis:.1} ms")
        };
    }

    let seconds = duration.as_secs_f64();
    if seconds < 10.0 {
        format!("{seconds:.2} s")
    } else if seconds < 100.0 {
        format!("{seconds:.1} s")
    } else {
        format!("{seconds:.0} s")
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[test]
    fn formats_elapsed_time_with_adaptive_units() {
        assert_eq!(format_elapsed(Duration::from_micros(750)), "750 µs");
        assert_eq!(format_elapsed(Duration::from_micros(1_200)), "1.20 ms");
        assert_eq!(format_elapsed(Duration::from_micros(18_700)), "18.7 ms");
        assert_eq!(format_elapsed(Duration::from_micros(999_960)), "1.00 s");
        assert_eq!(format_elapsed(Duration::from_millis(1_200)), "1.20 s");
    }

    #[test]
    fn times_only_normal_text_scans() {
        assert!(should_time_scan(&Cli::try_parse_from(["arid", "."]).unwrap()));
        assert!(!should_time_scan(
            &Cli::try_parse_from(["arid", "--json", "."]).unwrap()
        ));
        assert!(!should_time_scan(
            &Cli::try_parse_from(["arid", "--suppression-status", "."]).unwrap()
        ));
        assert!(!should_time_scan(
            &Cli::try_parse_from(["arid", "--explain-path", "src/a.py", "."]).unwrap()
        ));
        assert!(!should_time_scan(
            &Cli::try_parse_from(["arid", "--baseline-status", "debt.json", "."]).unwrap()
        ));
    }

    #[test]
    fn error_results_do_not_gain_timing_footer() {
        let cli = Cli::try_parse_from(["arid", "."]).unwrap();
        let result = run_timed_scan(&cli, || RunResult::failure("broken"));

        assert_eq!(result.exit_status(), ExitStatus::Error);
        assert!(!result.stdout().contains("Total time:"));
    }
}
