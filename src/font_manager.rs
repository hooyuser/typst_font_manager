use crate::command::FontCommand;
use crate::parse_font_config::{
    deserialize_fonts_from_file, deserialize_fonts_from_toml, FontConfig, TypstFont,
};
use crate::{create_font_path_map, create_font_path_map_from_dirs, utils};
use colored::Colorize;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

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

pub(crate) struct FontManager<'a> {
    config_file: &'a Path,      // Path to the configuration file
    font_config: FontConfig,    // Font configuration deserialized from font_config.toml
    library_dirs: Vec<PathBuf>, // Source font library directory paths
    absolute_font_dir: PathBuf, // Absolute path of the project's font directory
    font_sets: FontSets,        // Font sets to manage
    action: &'a str,
}

struct FontSets {
    required: BTreeSet<TypstFont>,
    current: BTreeSet<TypstFont>,
    embedded: BTreeSet<TypstFont>,
    missing: BTreeSet<TypstFont>,
    redundant: BTreeSet<TypstFont>,
    library: BTreeMap<TypstFont, PathBuf>,
}

impl<'a> FontManager<'a> {
    pub(crate) fn new(args: &'a FontCommand, action: &'a str) -> Result<Self, String> {
        // args.config is the path of font_config.toml specified by the user or the default value
        // Check if the file specified by args.config exists
        if !args.config.exists() {
            return Err(format!("Config file not found: {:?}", args.config));
        }

        // use user-specified font directories (args.library) if provided,
        // otherwise, use the system's default font directories.
        let library_dirs = args
            .library
            .clone()
            .unwrap_or_else(utils::font_utils::get_system_font_directories);

        // Deserialize the font configuration from font_config.toml
        let font_config = deserialize_fonts_from_file(&args.config)
            .map_err(|_| "Failed to parse font config file")?;

        // Resolve the absolute path of the project's font directory if specified in font_config.toml
        // Otherwise, use the default relative path "fonts"
        let absolute_font_dir = Self::resolve_font_directory(&args.config, &font_config)?;

        // Initialize the FontSets struct
        let font_sets =
            Self::initialize_font_sets(&library_dirs, &font_config, &absolute_font_dir)?;

        Ok(FontManager {
            config_file: &args.config,
            font_config,
            library_dirs,
            absolute_font_dir,
            font_sets,
            action,
        })
    }

    fn resolve_font_directory(
        config_file: &Path,
        font_config: &FontConfig,
    ) -> Result<PathBuf, String> {
        // Use the font directory specified in font_config.toml if exists,
        // otherwise, use the default relative path "fonts"
        let font_dir = font_config
            .font_dir
            .as_deref()
            .map(Path::new)
            .unwrap_or(Path::new("fonts"));

        // If the font directory path is relative, resolves its absolute path
        // relative to the parent of font_config.toml, or . if there's no parent
        if font_dir.is_relative() {
            Ok(config_file
                .parent()
                .unwrap_or(Path::new("."))
                .join(font_dir)
                .to_path_buf())
        } else {
            // If the font directory path is absolute, returns the path unchanged
            Ok(font_dir.to_path_buf())
        }
    }

    fn initialize_font_sets(
        library_dirs: &[PathBuf],
        font_config: &FontConfig,
        font_dir: &Path,
    ) -> Result<FontSets, String> {
        let required = BTreeSet::from_iter(font_config.fonts.clone());
        let current = create_font_path_map(font_dir).keys().cloned().collect();
        let embedded = deserialize_fonts_from_toml(EMBEDDED_FONTS)
            .map_err(|_| "Failed to parse embedded fonts")?
            .fonts
            .into_iter()
            .collect();

        let missing = required
            .difference(&embedded)
            .cloned()
            .collect::<BTreeSet<_>>()
            .difference(&current)
            .cloned()
            .collect();

        let redundant = current.difference(&required).cloned().collect();

        let font_lib_map = create_font_path_map_from_dirs(library_dirs);

        Ok(FontSets {
            required,
            current,
            embedded,
            missing,
            redundant,
            library: font_lib_map,
        })
    }

    pub(crate) fn print_status(&self) {
        self.print_header();
        self.print_directories(); // Print the directories used by the font manager
        self.print_legend();
        self.print_font_sets();
    }

    fn print_header(&self) {
        println!("\n=== {} ===\n", "Typst Font Manager".bold());
        println!("- Action: {}\n", self.action);
    }

    fn print_directories(&self) {
        println!("- Config file: {:?}", self.config_file);
        println!("\n- Font library directories:");
        for dir in &self.library_dirs {
            println!("  {dir:?}");
        }
        println!(
            "\n- Project font directory: {:?}",
            self.font_config.font_dir.as_deref().unwrap_or("fonts")
        );
    }

    fn print_legend(&self) {
        if !self.font_sets.required.is_empty() {
            println!("\n※ Legend:");
            println!(
                "  {} - Font is required and exists in the project",
                "●".green()
            );
            println!(
                "  {} - Font is required and is embedded in the compiler",
                "●".bright_green()
            );
            println!(
                "  {} - Font is not required but exists in the project",
                "●".blue()
            );
            println!(
                "  {} - Font is missing but can be fixed (available in font library)",
                "○".yellow()
            );
            println!("  {} - Font is missing", "○".red());
        }
    }

    fn print_font_sets(&self) {
        self.print_font_set("Current fonts", &self.font_sets.current, |font| {
            if self.font_sets.required.contains(font) {
                "●".green()
            } else {
                "●".blue()
            }
        });

        self.print_font_set("Required fonts", &self.font_sets.required, |font| {
            if self.font_sets.embedded.contains(font) {
                "●".bright_green()
            } else if !self.font_sets.missing.contains(font) {
                "●".green()
            } else if self.font_sets.library.contains_key(font) {
                "○".yellow()
            } else {
                "○".red()
            }
        });

        self.print_font_set("Missing fonts", &self.font_sets.missing, |font| {
            if self.font_sets.library.contains_key(font) {
                "○".yellow()
            } else {
                "○".red()
            }
        });

        self.print_font_set("Redundant fonts", &self.font_sets.redundant, |_| "●".blue());
    }

    fn print_font_set<F>(&self, title: &str, fonts: &BTreeSet<TypstFont>, get_bullet: F)
    where
        F: Fn(&TypstFont) -> colored::ColoredString,
    {
        println!(
            "\n- {} (total {}){}",
            title.bold(),
            fonts.len(),
            if fonts.is_empty() { "" } else { ":" }
        );
        for font in fonts {
            println!("  {} {}", get_bullet(font), font);
        }
    }

    pub(crate) fn download_fonts_from_github(
        &self,
        web_library: &BTreeMap<TypstFont, PathBuf>,
        github_repo: &str,
    ) -> Result<(), String> {
        let client = Client::new();

        if web_library.is_empty() {
            println!("\nNo missing fonts to download");
            return Ok(());
        }

        println!("\n- {}", "Downloading fonts from GitHub".bold());

        for (font, relative_path) in web_library {
            let url = format!(
                "https://raw.githubusercontent.com/{}/main/{}",
                github_repo,
                relative_path.display()
            );
            let dest_path = self
                .absolute_font_dir
                .join(relative_path.file_name().unwrap());

            println!("  Downloading {url} to {:?}", dest_path);

            // Perform the HTTP GET request to download the font
            let response = client
                .get(&url)
                .send()
                .map_err(|e| format!("Failed to download {}: {}", font, e))?;

            if response.status().is_success() {
                let mut file = fs::File::create(&dest_path)
                    .map_err(|e| format!("Failed to create file {:?}: {}", dest_path, e))?;
                let content = response
                    .bytes()
                    .map_err(|e| format!("Failed to read content of {}: {}", font, e))?;
                file.write_all(&content)
                    .map_err(|e| format!("Failed to write font file {:?}: {}", dest_path, e))?;
                println!("  Successfully downloaded {:?}", font);
            } else {
                return Err(format!(
                    "Failed to download {}. HTTP status: {}",
                    font,
                    response.status()
                ));
            }
        }

        Ok(())
    }

    pub(crate) fn update_fonts(&self) -> Result<(), String> {
        if self.font_sets.missing.is_empty() {
            println!("\nNo missing fonts to update");
            return Ok(());
        }

        println!("\n- {}", "Updating fonts".bold());

        for font in &self.font_sets.missing {
            // Get the path of the font file in the library
            if let Some(source_path) = self.font_sets.library.get(font) {
                // dest_path is where the font file will be copied to
                // it is the project's font directory joined with the file name of the font file
                let dest_path = self
                    .absolute_font_dir
                    .join(&source_path.file_name().unwrap());
                println!(
                    "  Copying {source_path:?} to {:?}",
                    Path::new(
                        &self
                            .font_config
                            .font_dir
                            .clone()
                            .unwrap_or_else(|| "fonts".to_string())
                    )
                    .join(&source_path.file_name().unwrap())
                );
                // Copy the font file from the library to the project's font directory
                fs::copy(&source_path, &dest_path)
                    .map_err(|_| format!("Failed to copy font file: {:?}", font))?;
            } else {
                println!("Font not found in source library: {:?}", font);
            }
        }
        Ok(())
    }
}

/// Wrapper struct for serializing/deserializing the library
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypstFontLibrary {
    #[serde(with = "font_map_serde")]
    pub library: BTreeMap<TypstFont, PathBuf>,
}

// Wrapper struct for serialization
mod font_map_serde {
    use super::*;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    /// A helper struct to represent key-value pairs
    #[derive(Serialize, Deserialize)]
    struct FontMapEntry {
        #[serde(flatten)]
        font: TypstFont,
        path: PathBuf,
    }

    pub fn serialize<S>(
        map: &BTreeMap<TypstFont, PathBuf>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let entries: Vec<FontMapEntry> = map
            .iter()
            .map(|(font, path)| FontMapEntry {
                font: font.clone(),
                path: path.clone(),
            })
            .collect();

        entries.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<BTreeMap<TypstFont, PathBuf>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entries: Vec<FontMapEntry> = Vec::deserialize(deserializer)?;
        Ok(entries
            .into_iter()
            .map(|entry| (entry.font, entry.path))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use typst::text::{FontStretch, FontStyle, FontWeight};

    #[test]
    fn test_font_library_serialization() {
        // Get the target directory
        let target_dir = env::var("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("target"));

        // Ensure the test-specific directory exists
        let test_dir = target_dir.join("test_outputs");
        fs::create_dir_all(&test_dir).expect("Failed to create test_outputs directory");

        // Define the file path in target/test_outputs
        let file_path = test_dir.join("library.toml");

        // Sample TypstFontLibrary
        let mut library = TypstFontLibrary {
            library: BTreeMap::new(),
        };

        library.library.insert(
            TypstFont {
                family_name: "Arial".to_string(),
                style: FontStyle::Normal,
                weight: FontWeight::REGULAR,
                stretch: FontStretch::NORMAL,
            },
            PathBuf::from("fonts/arial.ttf"),
        );

        library.library.insert(
            TypstFont {
                family_name: "Times New Roman".to_string(),
                style: FontStyle::Italic,
                weight: FontWeight::BOLD,
                stretch: FontStretch::NORMAL,
            },
            PathBuf::from("fonts/times.ttf"),
        );

        // Serialize to TOML and write to the target directory
        let toml = toml::to_string_pretty(&library).expect("Failed to serialize to TOML");
        fs::write(&file_path, toml.as_bytes()).expect("Failed to write to file");

        println!("TOML written to: {:?}", file_path);

        // Read and deserialize
        let contents = fs::read_to_string(&file_path).expect("Failed to read file");
        let deserialized: TypstFontLibrary =
            toml::from_str(&contents).expect("Failed to deserialize from TOML");

        assert_eq!(library.library, deserialized.library);
    }

    #[test]
    fn test_local_font_library_serialization() {
        // Get the target directory
        let target_dir = env::var("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("target"));

        // Ensure the test-specific directory exists
        let test_dir = target_dir.join("test_outputs");
        fs::create_dir_all(&test_dir).expect("Failed to create test_outputs directory");

        // Define the file path in target/test_outputs
        let file_path = test_dir.join("font_library.toml");

        let library_dirs = vec![PathBuf::from("/Users/chy/FONT_LIBRARY")];
        let font_lib_map = create_font_path_map_from_dirs(&library_dirs);

        // Sample TypstFontLibrary
        let library = TypstFontLibrary {
            library: font_lib_map,
        };
        // Serialize to TOML and write to the target directory
        let toml = toml::to_string_pretty(&library).expect("Failed to serialize to TOML");
        fs::write(&file_path, toml.as_bytes()).expect("Failed to write to file");

        println!("TOML written to: {:?}", file_path);
    }
}
