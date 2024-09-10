mod process_font;
mod parse_font_config;
mod utils;


use std::path::PathBuf;
use typst::diag::StrResult;
use typst::text::{FontStretch, FontStyle, FontVariant, FontWeight};

use walkdir::WalkDir;

use clap::{Parser, Subcommand};

use std::collections::HashSet;


struct TypstFont {
    family_name: String,
    style: FontStyle,
    weight: FontWeight,
    stretch: FontStretch,
}

pub fn fonts(path_str: &str) -> StrResult<()> {
    let font_paths: Vec<PathBuf> = vec![path_str.into()];

    for font_path in font_paths {
        // Walk through the directory recursively
        for entry in WalkDir::new(&font_path).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();

            if path.is_file() {
                // Print the file name
                if let Some(file_name) = path.file_name() {
                    println!("Processing [{}]", file_name.to_string_lossy());
                    let fonts = process_font::Fonts::searcher().search_file(&path);

                    for (name, infos) in fonts.book.families() {
                        println!("{name}");

                        for info in infos {
                            let FontVariant { style, weight, stretch } = info.variant;
                            println!("- Style: {style:?}, Weight: {weight:?}, Stretch: {stretch:?}\n");
                        }
                    }
                }
            }
        }
    }

    Ok(())
}


/// Typst Font Manager
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Check font configuration
    Check(FontCommand),
    /// Update font configuration
    Update(FontCommand),
}

#[derive(Parser, Debug)]
struct FontCommand {
    /// Path to the configuration file
    #[arg(default_value = "./font_config.toml")]
    config: PathBuf,

    /// Source directory paths
    #[arg(short, long, num_args = 1.., value_name = "DIR")]
    source: Option<Vec<PathBuf>>,
}


fn process_command(args: &FontCommand, action: &str) {
    println!("{} fonts", action);
    println!("Config file: {}", args.config.display());

    let source_dirs = match &args.source {
        Some(dirs) => dirs.clone(),
        None => utils::font_utils::get_system_font_directories(),
    };

    println!("Source directories:");
    for dir in source_dirs {
        println!("  {}", dir.display());
    }
}

fn show_fonts() {
    let path_str = "./assets/FONTS_LIBRARY/";
    //let path_str = "/Users/chy/Projects/Typst/algebraic_geometry/fonts/";
    fonts(path_str).unwrap();
}


fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Check(args) => process_command(args, "Checking"),
        Commands::Update(args) => process_command(args, "Updating"),
    }
}
#[cfg(test)]
mod tests {
    use crate::utils::font_utils::get_system_font_directories;
    use super::*;

    #[test]
    fn test_show_fonts() {
        show_fonts();
    }

    #[test]
    fn test_get_system_font_dirs() {
        let font_dirs = get_system_font_directories();
        for font_dir in font_dirs {
            println!("{:?}", font_dir);
        }
    }
}
