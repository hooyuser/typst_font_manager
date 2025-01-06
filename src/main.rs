mod command;
mod font_manager;
mod parse_font_config;
mod process_font;
mod utils;

use clap::Parser;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use typst::text::FontVariant;
use walkdir::WalkDir;

use crate::command::{Commands, FontCommand};
use crate::font_manager::{get_github_font_library_info, LibraryDirs};
use crate::parse_font_config::TypstFont;

// pub fn fonts(path_str: &str) -> StrResult<()> {
//     let font_paths: Vec<PathBuf> = vec![path_str.into()];
//
//     for font_path in font_paths {
//         // Walk through the directory recursively
//         for entry in WalkDir::new(&font_path).into_iter().filter_map(|e| e.ok()) {
//             let path = entry.path();
//
//             if path.is_file() {
//                 // Print the file name
//                 if let Some(file_name) = path.file_name() {
//                     //println!("Processing [{}]", file_name.to_string_lossy());
//                     let fonts = process_font::Fonts::searcher().search_file(&path);
//
//                     for (name, infos) in fonts.book.families() {
//                         //println!("{name}");
//
//                         for info in infos {
//                             let FontVariant {
//                                 style,
//                                 weight,
//                                 stretch,
//                             } = info.variant;
//                             //println!("- Style: {style:?}, Weight: {weight:?}, Stretch: {stretch:?}\n");
//                         }
//                     }
//                 }
//             }
//         }
//     }
//
//     Ok(())
// }

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

// fn show_fonts() {
//     let path_str = "./assets/FONTS_LIBRARY/";
//     //let path_str = "/Users/chy/Projects/Typst/algebraic_geometry/fonts/";
//     fonts(path_str).unwrap();
// }

fn main() {
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

            for (font, path) in font_lib_map {
                println!("{font} - {path:?}");
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use crate::utils::font_utils::get_system_font_directories;

    // #[test]
    // fn test_show_fonts() {
    //     show_fonts();
    // }

    #[test]
    fn test_get_system_font_dirs() {
        let font_dirs = get_system_font_directories();
        for font_dir in font_dirs {
            println!("{:?}", font_dir);
        }
    }
}
