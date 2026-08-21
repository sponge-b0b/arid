use arid::{Cli, ColorEnvironment, ExitStatus, RunContext, RunResult, run, run_with_context};
use clap::Parser;

#[test]
fn supported_root_api_is_usable() {
    let cli = Cli::try_parse_from(["arid", "--capabilities"]).unwrap();

    let result: RunResult = run(&cli);
    assert_eq!(result.exit_status(), ExitStatus::Success);
    assert!(result.stderr().is_empty());
    assert!(!result.stdout().is_empty());

    let context = RunContext::new(false, ColorEnvironment::default());
    let contextual = run_with_context(&cli, context);
    assert_eq!(contextual, result);
}

#[test]
fn run_context_supports_virtual_source_input() {
    let context = RunContext::non_terminal().with_stdin_source("value = 1\n".to_owned());
    let _ = context;
}
