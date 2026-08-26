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
mod project_path;
mod python;
mod report;
mod sarif;
mod source;
mod suffix;
mod suppression;
mod text;

pub use app::{ColorEnvironment, RunContext, run, run_with_context};
pub use cli::Cli;
pub use outcome::{ExitStatus, RunResult};
