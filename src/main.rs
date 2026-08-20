use std::io::{self, IsTerminal};
use std::process::ExitCode;

use arid::cli::Cli;
use arid::{ColorEnvironment, RunContext};
use clap::Parser;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let context = RunContext {
        text_color_capable: io::stdout().is_terminal(),
        color_environment: ColorEnvironment::from_process(),
    };

    let result = arid::run_with_context(&cli, context);

    write_stdout(result.stdout());
    write_stderr(result.stderr());

    ExitCode::from(result.exit_status().code())
}

fn write_stdout(output: &str) {
    if output.is_empty() {
        return;
    }

    if output.ends_with('\n') {
        print!("{output}");
    } else {
        println!("{output}");
    }
}

fn write_stderr(output: &str) {
    if output.is_empty() {
        return;
    }

    if output.ends_with('\n') {
        eprint!("{output}");
    } else {
        eprintln!("{output}");
    }
}
