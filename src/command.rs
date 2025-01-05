use std::path::PathBuf;
use clap::Parser;

#[derive(Parser, Debug)]
pub(crate) struct FontCommand {
    /// Path to the configuration file
    #[arg(default_value = "./font_config.toml")]
    pub(crate) config: PathBuf,

    /// Source font library directory paths
    #[arg(short, long, num_args = 1.., value_name = "DIR")]
    pub(crate) library: Option<Vec<PathBuf>>,
}