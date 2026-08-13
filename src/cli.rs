use std::path::PathBuf;

use clap::Parser;

/// Fast Python duplicate-code checker written in Rust.
#[derive(Debug, Parser)]
#[command(name = "arid", version, about)]
pub struct Cli {
    /// Files or directories to scan. Defaults to the current directory.
    #[arg(value_name = "PATH")]
    pub paths: Vec<PathBuf>,

    /// Emit JSON instead of human-readable output.
    #[arg(long)]
    pub json: bool,

    /// Include source text in reported duplicate locations.
    #[arg(long)]
    pub show_source: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_no_paths() {
        let cli = Cli::try_parse_from(["arid"]).unwrap();

        assert!(cli.paths.is_empty());
        assert!(!cli.json);
        assert!(!cli.show_source);
    }

    #[test]
    fn accepts_paths_and_output_options() {
        let cli =
            Cli::try_parse_from(["arid", "--json", "--show-source", "src", "tests/example.py"])
                .unwrap();

        assert_eq!(
            cli.paths,
            vec![PathBuf::from("src"), PathBuf::from("tests/example.py"),]
        );

        assert!(cli.json);
        assert!(cli.show_source);
    }
}
