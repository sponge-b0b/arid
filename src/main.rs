use std::process::ExitCode;

use arid::cli::Cli;
use arid::outcome::ExitStatus;
use clap::Parser;

fn main() -> ExitCode {
    let cli = Cli::parse();

    match arid::run(&cli) {
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
