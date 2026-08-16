use std::io::{self, IsTerminal};
use std::process::ExitCode;

use arid::cli::Cli;
use arid::outcome::ExitStatus;
use arid::{ColorEnvironment, RunContext};
use clap::Parser;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let context = RunContext {
        text_color_capable: io::stdout().is_terminal(),
        color_environment: ColorEnvironment::from_process(),
    };

    match arid::run_with_context(&cli, context) {
        Ok(result) => {
            write_output(&result.output);

            ExitCode::from(result.exit_status.code())
        }
        Err(error) => {
            eprintln!("error: {error}");

            ExitCode::from(ExitStatus::Error.code())
        }
    }
}

fn write_output(output: &str) {
    if output.ends_with('\n') {
        print!("{output}");
    } else {
        println!("{output}");
    }
}
