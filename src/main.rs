mod parse_font_config;
mod process_font;
mod utils;
mod font_manager;
mod command;



use clap::{Parser, Subcommand};
use colored::Colorize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use typst::diag::StrResult;
use typst::text::{FontStretch, FontStyle, FontVariant, FontWeight};
use walkdir::WalkDir;



use crate::command::{FontCommand};
use crate::parse_font_config::TypstFont;

pub fn fonts(path_str: &str) -> StrResult<()> {
    let font_paths: Vec<PathBuf> = vec![path_str.into()];

    for font_path in font_paths {
        // Walk through the directory recursively
        for entry in WalkDir::new(&font_path).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();

            if path.is_file() {
                // Print the file name
                if let Some(file_name) = path.file_name() {
                    //println!("Processing [{}]", file_name.to_string_lossy());
                    let fonts = process_font::Fonts::searcher().search_file(&path);

                    for (name, infos) in fonts.book.families() {
                        //println!("{name}");

                        for info in infos {
                            let FontVariant {
                                style,
                                weight,
                                stretch,
                            } = info.variant;
                            //println!("- Style: {style:?}, Weight: {weight:?}, Stretch: {stretch:?}\n");
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

pub fn create_font_path_map<P: AsRef<Path>>(font_dir: P) -> BTreeMap<TypstFont, PathBuf> {
    let mut font_map = BTreeMap::<TypstFont, PathBuf>::new();

    // Walk through the directory recursively
    for entry in WalkDir::new(&font_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();

        font_path_map_update(&mut font_map, path);
    }

    font_map
}

fn create_font_path_map_from_dirs<P: AsRef<Path>>(
    font_dirs: &[P],
) -> BTreeMap<TypstFont, PathBuf> {
    let mut font_map = BTreeMap::<TypstFont, PathBuf>::new();

    for font_dir in font_dirs {
        // Walk through the directory recursively
        for entry in WalkDir::new(&font_dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();

            font_path_map_update(&mut font_map, path);
        }
    }

    font_map
}

fn font_path_map_update(font_map: &mut BTreeMap<TypstFont, PathBuf>, path: &Path) {
    if path.is_file() {
        // Print the file name
        if let Some(file_name) = path.file_name() {
            //println!("Processing [{}]", &file_name.to_string_lossy());
            let fonts = process_font::Fonts::searcher().search_file(&path);

            for (name, infos) in fonts.book.families() {
                //println!("{name}");

                for info in infos {
                    let FontVariant {
                        style,
                        weight,
                        stretch,
                    } = info.variant;
                    //println!("- Style: {style:?}, Weight: {weight:?}, Stretch: {stretch:?}\n");

                    let font = TypstFont {
                        family_name: String::from(name),
                        style,
                        weight,
                        stretch,
                    };

                    font_map.insert(font, path.to_path_buf());
                }
            }
        }
    }
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
    /// Show font library information
    CheckLib(CheckLibCommand),
}



#[derive(Parser, Debug)]
struct CheckLibCommand {
    /// Path to the font library directory
    #[arg(short, long, num_args = 1.., value_name = "DIR")]
    library: Option<Vec<PathBuf>>,
}


fn process_command(args: &FontCommand, action: &str) {
    match font_manager::FontManager::new(args, action) {
        Ok(font_manager) => {
            font_manager.print_status();

            if action == "Updating" {
                if let Err(e) = font_manager.update_fonts() {
                    println!("Error updating fonts: {e}");
                }
            }

            println!("\n=== Done ===");
        }
        Err(e) => println!("Error initializing font manager: {e}"),
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
        Commands::CheckLib(args) => {
            let library_dirs = match &args.library {
                Some(dirs) => dirs.clone(),
                None => utils::font_utils::get_system_font_directories(),
            };
            let font_lib_map = create_font_path_map_from_dirs(&library_dirs);

            println!("\n=== Font Library ===\n");

            println!("\n- Font library directories:");
            for dir in &library_dirs {
                println!("  {dir:?}");
            }
            println!("\n- Font Info:");

            for (font, path) in font_lib_map {
                println!("{font} - {path:?}");
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::font_utils::get_system_font_directories;

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
