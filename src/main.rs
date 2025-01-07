mod command;
mod font_manager;
mod parse_font_config;
mod process_font;
mod utils;

use clap::Parser;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use typst::text::FontVariant;
use walkdir::WalkDir;

use crate::command::{Commands, FontCommand};
use crate::font_manager::{
    get_github_font_library_info, strip_library_root_path, LibraryDirs, TypstFontLibrary,
};
use crate::parse_font_config::TypstFont;


pub fn create_font_path_map<P: AsRef<Path>>(font_dir: P) -> BTreeMap<TypstFont, PathBuf> {
    let mut font_map = BTreeMap::<TypstFont, PathBuf>::new();

    // Walk through the directory recursively
    for entry in WalkDir::new(&font_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();

        font_path_map_update(&mut font_map, path);
    }

    font_map
}

fn create_font_path_map_from_dirs(library_dirs: &LibraryDirs) -> BTreeMap<TypstFont, PathBuf> {
    let mut font_map = BTreeMap::<TypstFont, PathBuf>::new();

    match library_dirs {
        LibraryDirs::GitHub(github_repos) => {
            for github_repo in github_repos {
                // github_repo is a string like "owner/repo"
                let github_font_map = get_github_font_library_info(&github_repo)
                    .expect("Error Occurs when getting fonts from GitHub");
                font_map.extend(github_font_map);
            }
        }
        LibraryDirs::Local(font_dirs) => {
            for font_dir in font_dirs {
                for entry in WalkDir::new(&font_dir).into_iter().filter_map(|e| e.ok()) {
                    let path = entry.path();

                    font_path_map_update(&mut font_map, path);
                }
            }
        }
    }

    font_map
}

fn font_path_map_update(font_map: &mut BTreeMap<TypstFont, PathBuf>, path: &Path) {
    if path.is_file() {
        // Print the file name
        if let Some(_file_name) = path.file_name() {
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

fn process_command(args: &FontCommand, action: &str) {
    args.validate().unwrap();
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


fn main() {
    #[cfg(debug_assertions)]
    {
        use colored::Colorize;
        println!("{}", "Dev Version".bold().red());
    }

    let cli = Cli::parse();
    match &cli.command {
        Commands::Check(args) => process_command(args, "Checking"),
        Commands::Update(args) => process_command(args, "Updating"),
        Commands::CheckLib(args) => {
            let library_dirs = if args.github {
                LibraryDirs::GitHub(args.library.clone().unwrap())
            } else {
                LibraryDirs::Local(match &args.library {
                    Some(dirs) => dirs.clone(),
                    None => utils::font_utils::get_system_font_directories(),
                })
            };
            let font_lib_map = create_font_path_map_from_dirs(&library_dirs);

            println!("\n=== Font Library ===\n");

            println!("\n- Font library directories:");
            for dir in &library_dirs {
                println!("  {dir:?}");
            }
            println!("\n- Font Info:");

            for (font, path) in &font_lib_map {
                println!("{font} - {path:?}");
            }

            if let Some(output_dir_arg) = &args.output {
                match library_dirs {
                    LibraryDirs::GitHub(_) => {}
                    LibraryDirs::Local(library_dirs) => {
                        // if length of library_dirs is greater than 1, print an error message
                        if library_dirs.len() > 1 {
                            println!("Error: If output directory is provided, there should be only one library directory.");
                            return;
                        }

                        // if output_dir is provided, write the font library info to the output directory
                        // otherwise, write to the library_dirs[0]
                        let output_dir = match &output_dir_arg {
                            Some(dir) => dir.clone(),
                            None => library_dirs[0].clone(),
                        };

                        let mut font_lib_map = font_lib_map.clone();
                        // For the output toml file, strip the library root path
                        strip_library_root_path(&mut font_lib_map, &output_dir);

                        // Sample TypstFontLibrary
                        let library = TypstFontLibrary {
                            fonts: font_lib_map,
                        };
                        // Serialize to TOML and write to the target directory
                        let toml =
                            toml::to_string_pretty(&library).expect("Failed to serialize to TOML");

                        // Define the file path in target/test_outputs
                        let file_path = output_dir.join("font_library.toml");
                        fs::write(&file_path, toml.as_bytes()).expect("Failed to write to file");
                    }
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use crate::utils::font_utils::get_system_font_directories;


    #[test]
    fn test_get_system_font_dirs() {
        let font_dirs = get_system_font_directories();
        for font_dir in font_dirs {
            println!("{:?}", font_dir);
        }
    }
}
