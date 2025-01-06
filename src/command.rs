use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Subcommand, Debug)]
pub(crate) enum Commands {
    /// Check font configuration
    Check(FontCommand),
    /// Update font configuration
    Update(FontCommand),
    /// Show font library information
    CheckLib(CheckLibCommand),
}

#[derive(Parser, Debug)]
pub(crate) struct FontCommand {
    /// Path to the configuration file
    #[arg(default_value = "./font_config.toml")]
    pub(crate) config: PathBuf,

    /// Source font library directory paths
    /// For GitHub repositories, use the format "owner/repo"
    #[arg(short, long, num_args = 1.., value_name = "DIR")]
    pub(crate) library: Option<Vec<PathBuf>>,

    /// Whether source font libraries are GitHub repositories
    #[arg(short, long, default_value = "false")]
    pub(crate) github: bool,
}

#[derive(Parser, Debug)]
pub(crate) struct CheckLibCommand {
    /// Path to the font library directory
    #[arg(short, long, num_args = 1.., value_name = "DIR")]
    pub(crate) library: Option<Vec<PathBuf>>,

    /// Whether source font libraries are GitHub repositories
    #[arg(short, long, default_value = "false")]
    pub(crate) github: bool,
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
