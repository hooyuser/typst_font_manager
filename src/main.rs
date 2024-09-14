mod process_font;
mod parse_font_config;
mod utils;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use typst::diag::StrResult;
use typst::text::{FontStretch, FontStyle, FontVariant, FontWeight};
use walkdir::WalkDir;
use clap::{Parser, Subcommand};
use colored::Colorize;

use crate::parse_font_config::{deserialize_fonts_from_file, deserialize_fonts_from_toml, TypstFont};

const EMBEDDED_FONTS: &str = r#"
[[fonts]]
family_name = "DejaVu Sans Mono"
style = "Normal"
weight = [400, 700]
stretch = 1000

[[fonts]]
family_name = "DejaVu Sans Mono"
style = "Italic"
weight = [400, 700]
stretch = 1000

[[fonts]]
family_name = "Linux Libertine"
style = "Normal"
weight = [400, 700]
stretch = 1000

[[fonts]]
family_name = "Linux Libertine"
style = "Italic"
weight = [400, 700]
stretch = 1000

[[fonts]]
family_name = "New Computer Modern"
style = "Normal"
weight = [400, 700]
stretch = 1000

[[fonts]]
family_name = "New Computer Modern"
style = "Italic"
weight = [400, 700]
stretch = 1000

[[fonts]]
family_name = "New Computer Modern Math"
style = "Normal"
weight = [400, 450]
stretch = 1000
"#;


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
                            let FontVariant { style, weight, stretch } = info.variant;
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


fn create_font_path_map_from_dirs<P: AsRef<Path>>(font_dirs: &Vec<P>) -> BTreeMap<TypstFont, PathBuf> {
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

fn font_path_map_update(mut font_map: &mut BTreeMap<TypstFont, PathBuf>, path: &Path) {
    if path.is_file() {
        // Print the file name
        if let Some(file_name) = path.file_name() {
            //println!("Processing [{}]", &file_name.to_string_lossy());
            let fonts = process_font::Fonts::searcher().search_file(&path);

            for (name, infos) in fonts.book.families() {
                //println!("{name}");

                for info in infos {
                    let FontVariant { style, weight, stretch } = info.variant;
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
}

#[derive(Parser, Debug)]
struct FontCommand {
    /// Path to the configuration file
    #[arg(default_value = "./font_config.toml")]
    config: PathBuf,

    /// Source font library directory paths
    #[arg(short, long, num_args = 1.., value_name = "DIR")]
    source: Option<Vec<PathBuf>>,
}


fn process_command(args: &FontCommand, action: &str) {
    println!("\n=== {} ===\n", "Typst Font Manager".bold());

    println!("- Action: {}\n", action);
    let config_file: &Path = &args.config;

    // if config file not found, log error and return
    if !config_file.exists() {
        println!("Config file not found: {:?}", config_file);
        return;
    } else {
        println!("- Config file: {:?}", config_file);
    }

    let source_dirs = match &args.source {
        Some(dirs) => dirs.clone(),
        None => utils::font_utils::get_system_font_directories(),
    };

    println!("\n- Font library directories:");
    for dir in &source_dirs {
        println!("  {dir:?}");
    }

    let font_config = deserialize_fonts_from_file(&config_file)
        .expect("Failed to parse font config file");

    let font_dir = font_config.font_dir
        .as_deref()
        .map(Path::new)
        .unwrap_or(Path::new("fonts"));

    // Check if font_dir is relative, then make it absolute relative to config_file's directory
    let absolute_font_dir = if font_dir.is_relative() {
        config_file
            .parent()  // Get the directory of config_file
            .unwrap_or(Path::new("."))  // Fallback to current directory if no parent
            .join(font_dir)  // Join relative font_dir with config_file's directory
            .to_path_buf()
    } else {
        font_dir.to_path_buf()
    };

    // check if the font directory exists
    if !absolute_font_dir.exists() {
        println!(r#"Cannot find the font directory {:?} specified in {:?}"#, font_dir, config_file);
        if action == "Checking" {
            println!("You may want to create the directory or update the font configuration");
            return;
        } else if action == "Updating" {
            println!("Creating font directory: {font_dir:?}");
            std::fs::create_dir_all(&absolute_font_dir)
                .expect("Failed to create font directory");
        }
    } else {
        println!("\n- Project font directory: {:?}", font_dir);
    }

    let required_font_set: BTreeSet<_> = BTreeSet::from_iter(font_config.fonts);


    let current_font_map = create_font_path_map(&absolute_font_dir);

    let current_font_set: BTreeSet<_> = current_font_map.keys().cloned().collect();


    let embedded_fonts: BTreeSet<_> = deserialize_fonts_from_toml(EMBEDDED_FONTS)
        .expect("Failed to parse embedded fonts").fonts.iter().cloned().collect();

    //let missing_fonts = required_font_set.difference(&current_font_set);
    // Collect missing_fonts into a vector so it can be reused
    let missing_fonts: BTreeSet<_> = required_font_set
        .difference(&embedded_fonts)
        .cloned()
        .collect::<BTreeSet<_>>() // Ensure owned values
        .difference(&current_font_set)
        .cloned() // Ensure owned values for the final result
        .collect();

    let font_lib_map = create_font_path_map_from_dirs(&source_dirs);

    if !required_font_set.is_empty() {
        // Print the legend explaining the colors
        println!("\n※ Legend:");
        println!("  {} - Font is required and exists in the project", "●".green());
        println!("  {} - Font is not required but exists in the project", "●".blue());
        println!("  {} - Font is missing but can be fixed (available in font library)", "○".yellow());
        println!("  {} - Font is missing", "○".red());
    }

    // print the current font set
    println!("\n- {} (total {}){}", "Current fonts".bold(), current_font_set.len(), if current_font_set.is_empty() { "" } else { ":" });
    for font in &current_font_set {
        let bullet = if required_font_set.contains(font) {
            "●".green()
        } else {
            "●".blue()
        };
        println!("  {} {}", bullet, font);
    }


    // print the required font set
    println!("\n- {} (total {}){}", "Required fonts".bold(), required_font_set.len(), if required_font_set.is_empty() { "" } else { ":" });
    for font in &required_font_set {
        let bullet = if !missing_fonts.contains(font) {
            "●".green()
        } else if font_lib_map.contains_key(font) {
            "○".yellow()
        } else {
            "○".red()
        };
        println!("  {} {}", bullet, font);
    }


    println!("\n- {} (total {}){}", "Missing fonts".bold(), missing_fonts.len(), if missing_fonts.is_empty() { "" } else { ":" });
    for font in &missing_fonts {
        let bullet = if font_lib_map.contains_key(font) {
            "○".yellow()
        } else {
            "○".red()
        };
        println!("  {} {}", bullet, font);
    }

    let redundant_fonts: Vec<_> = current_font_set.difference(&required_font_set).collect();

    println!("\n- {} (total {}){}", "Redundant fonts".bold(), redundant_fonts.len(), if redundant_fonts.is_empty() { "" } else { ":" });
    for font in redundant_fonts {
        println!("  {} {}", "●".blue(), font);
    }

    if action == "Updating" {
        println!("\n- {}", "Updating fonts".bold());
        // for each missing font, check if it is available in the source directories, and copy it to the font directory if found

        if !missing_fonts.is_empty() {
            for font in &missing_fonts {
                if let Some(source_path) = font_lib_map.get(font) {
                    let dest_path = absolute_font_dir.join(&source_path.file_name().unwrap());
                    println!("  Copying {source_path:?} to {:?}", font_dir.join(&source_path.file_name().unwrap()));
                    std::fs::copy(&source_path, &dest_path)
                        .expect("Failed to copy font file");
                } else {
                    println!("Font not found in source library: {:?}", font);
                }
            }
        }
        else{
            println!("No missing fonts to update");
        }
    }

    println!("\n=== Done ===");
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
