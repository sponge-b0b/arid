use std::io::{self, IsTerminal, Read};
use std::process::ExitCode;

use arid::{Cli, ColorEnvironment, RunContext};
use clap::Parser;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let mut context = RunContext::new(io::stdout().is_terminal(), ColorEnvironment::from_process());

    if cli.stdin_path.is_some() {
        let mut source = String::new();
        if let Err(error) = io::stdin().read_to_string(&mut source) {
            eprintln!("error: failed to read standard input: {error}");
            return ExitCode::from(2);
        }
        context = context.with_stdin_source(source);
    }

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
