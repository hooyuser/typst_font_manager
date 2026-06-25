use clap::{Args, Subcommand, ValueHint};
use std::path::PathBuf;

#[derive(Subcommand, Debug)]
pub(crate) enum Commands {
    /// Check font configuration
    Check(FontCommand),
    /// Update font configuration
    Update(UpdateCommand),
    /// Show font library information
    CheckLib(CheckLibCommand),
}

#[derive(Args, Debug)]
pub(crate) struct FontCommand {
    /// Project root directory or path to font_config.toml
    #[arg(default_value = ".", value_name = "PROJECT_OR_CONFIG")]
    pub(crate) project_or_config: PathBuf,

    /// Source font library directory paths
    /// For GitHub repositories, use the format "owner/repo"
    #[arg(short, long, num_args = 1.., value_name = "DIR")]
    pub(crate) library: Option<Vec<PathBuf>>,

    /// Whether source font libraries are GitHub repositories
    #[arg(short, long, default_value = "false")]
    pub(crate) github: bool,
}

#[derive(Args, Debug)]
pub(crate) struct UpdateCommand {
    #[command(flatten)]
    pub(crate) font: FontCommand,

    /// Print the planned font updates without copying or downloading files
    #[arg(long, default_value = "false")]
    pub(crate) dry_run: bool,
}

#[derive(Args, Debug)]
pub(crate) struct CheckLibCommand {
    /// Path to the font library directory
    #[arg(short, long, num_args = 1.., value_name = "DIR")]
    pub(crate) library: Option<Vec<PathBuf>>,

    /// Whether source font libraries are GitHub repositories
    #[arg(short, long, default_value = "false")]
    pub(crate) github: bool,

    /// Output path for the results (optional, can be specified without a value)
    #[arg(short, long, value_name = "OUTPUT", num_args = 0..=1, value_hint = ValueHint::FilePath)]
    pub(crate) output: Option<Option<PathBuf>>,
}

impl FontCommand {
    /// Validate the configuration
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.github && self.library.is_none() {
            return Err(
                "When '--github' is set to true, '--library' must also be provided.".to_string(),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Commands;
    use clap::Parser;
    use std::path::PathBuf;

    #[derive(Parser, Debug)]
    struct TestCli {
        #[command(subcommand)]
        command: Commands,
    }

    #[test]
    fn update_accepts_dry_run() {
        let cli = TestCli::parse_from([
            "typfont",
            "update",
            "--dry-run",
            "-l",
            "/Users/goodguy/font_lib",
        ]);

        match cli.command {
            Commands::Update(args) => {
                assert!(args.dry_run);
                assert_eq!(
                    args.font.library,
                    Some(vec![PathBuf::from("/Users/goodguy/font_lib")])
                );
            }
            _ => panic!("expected update command"),
        }
    }

    #[test]
    fn check_does_not_accept_dry_run() {
        assert!(TestCli::try_parse_from(["typfont", "check", "--dry-run"]).is_err());
    }
}
