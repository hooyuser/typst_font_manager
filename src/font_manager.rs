use crate::command::FontCommand;
use crate::parse_font_config::{
    FontConfig, TypstFont, deserialize_fonts_from_file, deserialize_fonts_from_toml,
};
use crate::{DiscoveredFont, create_font_entries, create_font_entries_from_dirs, utils};
use colored::Colorize;
use reqwest::blocking::{Client, get};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::slice::Iter;
use typst::text::{AxisValue, FontAxis, FontStretch, FontStyle, FontWeight, StandardAxes, Tag};

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

pub(crate) enum LibraryDirs {
    Local(Vec<PathBuf>),  // Local font library directories, like /usr/share/fonts
    GitHub(Vec<PathBuf>), // GitHub repositories, like "owner/repo"
}

// Implement IntoIterator for `&LibraryDirs`
impl<'a> IntoIterator for &'a LibraryDirs {
    type Item = &'a PathBuf;
    type IntoIter = Iter<'a, PathBuf>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            LibraryDirs::Local(paths) => paths.iter(),
            LibraryDirs::GitHub(paths) => paths.iter(),
        }
    }
}

pub(crate) struct FontManager<'a> {
    config_file: PathBuf,       // Path to the configuration file
    font_config: FontConfig,    // Font configuration deserialized from font_config.toml
    library_dirs: LibraryDirs,  // Source font library directory paths
    absolute_font_dir: PathBuf, // Absolute path of the project's font directory
    font_sets: FontSets,        // Font sets to manage
    action: &'a str,
}

struct FontSets {
    required: BTreeSet<TypstFont>,
    current: BTreeSet<TypstFont>,
    current_entries: Vec<DiscoveredFont>,
    embedded: BTreeSet<TypstFont>,
    missing: BTreeSet<TypstFont>,
    redundant: BTreeSet<TypstFont>,
    library_entries: Vec<DiscoveredFont>,
}

fn get_first_two_segments<P>(repo: &P) -> Option<&Path>
where
    P: AsRef<Path> + ?Sized,
{
    let p = repo.as_ref();

    // Count components first. We need at least 3:
    //   1. user_name
    //   2. my_repo
    //   3. something_else (dir/sad.txt, etc.)
    if p.components().count() < 3 {
        return None;
    }

    // Go up 2 parent directories to remove the last two components.
    // Example:
    //   "user_name/my_repo/dir/sad.txt".parent() -> "user_name/my_repo/dir"
    //                              .parent() -> "user_name/my_repo"
    p.parent().and_then(|one_up| one_up.parent())
}

fn get_remaining_after_two_segments<P>(repo: &P) -> Option<&Path>
where
    P: AsRef<Path> + ?Sized,
{
    let p = repo.as_ref();
    let mut comps = p.components();

    // Skip first 2 segments.
    comps.next()?; // "user_name"
    comps.next()?; // "my_repo"

    // The rest of the components are our remainder.
    let remainder = comps.as_path();
    if remainder.as_os_str().is_empty() {
        None
    } else {
        Some(remainder)
    }
}

fn font_entries_to_set(entries: &[DiscoveredFont]) -> BTreeSet<TypstFont> {
    entries.iter().map(|entry| entry.font.clone()).collect()
}

fn font_is_satisfied_by_entries(font: &TypstFont, entries: &[DiscoveredFont]) -> bool {
    entries
        .iter()
        .any(|entry| font_entry_satisfies(entry, font))
}

fn font_entry_satisfies(entry: &DiscoveredFont, intent: &TypstFont) -> bool {
    if entry.font.family_name != intent.family_name {
        return false;
    }

    let standard = StandardAxes::parse(&entry.axes);

    style_satisfies(entry.font.style, intent.style, &standard)
        && weight_satisfies(entry.font.weight, intent.weight, standard.wght)
        && stretch_satisfies(entry.font.stretch, intent.stretch, standard.wdth)
}

fn style_satisfies(actual: FontStyle, intent: FontStyle, axes: &StandardAxes<'_>) -> bool {
    if actual == intent {
        return true;
    }

    match intent {
        FontStyle::Normal => {
            axis_contains(axes.ital, AxisValue(0.0)) || axis_contains(axes.slnt, AxisValue(0.0))
        }
        FontStyle::Italic => axis_contains(axes.ital, AxisValue(1.0)),
        FontStyle::Oblique => axes.slnt.is_some_and(axis_has_non_zero),
    }
}

fn weight_satisfies(actual: FontWeight, intent: FontWeight, axis: Option<&FontAxis>) -> bool {
    axis.map_or(actual == intent, |axis| {
        axis_contains(Some(axis), intent.to_wght())
    })
}

fn stretch_satisfies(actual: FontStretch, intent: FontStretch, axis: Option<&FontAxis>) -> bool {
    axis.map_or(actual == intent, |axis| {
        axis_contains(Some(axis), intent.to_wdth())
    })
}

fn axis_contains(axis: Option<&FontAxis>, value: AxisValue) -> bool {
    axis.is_some_and(|axis| value.0 >= axis.min.0 && value.0 <= axis.max.0)
}

fn axis_has_non_zero(axis: &FontAxis) -> bool {
    axis.min.0 < 0.0 || axis.max.0 > 0.0
}

fn entry_has_variant_axis(entry: &DiscoveredFont) -> bool {
    let standard = StandardAxes::parse(&entry.axes);
    standard.ital.is_some()
        || standard.slnt.is_some()
        || standard.wght.is_some()
        || standard.wdth.is_some()
}

fn format_discovered_font(entry: &DiscoveredFont) -> String {
    let standard = StandardAxes::parse(&entry.axes);
    let weight = standard
        .wght
        .map(format_weight_range)
        .unwrap_or_else(|| entry.font.weight.to_number().to_string());
    let stretch = standard
        .wdth
        .map(format_stretch_range)
        .unwrap_or_else(|| stretch_to_number(entry.font.stretch).to_string());

    format!(
        "{:<30}    (style: {:?}, weight: {}, stretch: {})",
        entry.font.family_name, entry.font.style, weight, stretch
    )
}

fn format_weight_range(axis: &FontAxis) -> String {
    format_range(
        FontWeight::from_wght(axis.min).to_number(),
        FontWeight::from_wght(axis.max).to_number(),
    )
}

fn format_stretch_range(axis: &FontAxis) -> String {
    format_range(
        stretch_to_number(FontStretch::from_wdth(axis.min)),
        stretch_to_number(FontStretch::from_wdth(axis.max)),
    )
}

fn format_range(min: u16, max: u16) -> String {
    if min == max {
        min.to_string()
    } else {
        format!("{min}-{max}")
    }
}

fn stretch_to_number(stretch: FontStretch) -> u16 {
    (stretch.to_ratio().get() * 1000.0) as u16
}

fn select_best_font_entry<'a>(
    font: &TypstFont,
    entries: &'a [DiscoveredFont],
) -> Option<&'a DiscoveredFont> {
    entries
        .iter()
        .filter(|entry| font_entry_satisfies(entry, font))
        .min_by_key(|entry| {
            (
                !entry_has_variant_axis(entry),
                entry.path.to_string_lossy().to_string(),
            )
        })
}

impl<'a> FontManager<'a> {
    pub(crate) fn new(args: &'a FontCommand, action: &'a str) -> Result<Self, String> {
        let config_file = Self::resolve_config_file(&args.project_or_config);

        if !config_file.exists() {
            return Err(format!("Config file not found: {:?}", config_file));
        }

        // use user-specified font directories (args.library) if provided,
        // otherwise, use the system's default font directories.
        let library_dirs = if args.github {
            LibraryDirs::GitHub(
                args.library
                    .clone()
                    .expect("GitHub repository not provided"),
            )
        } else {
            LibraryDirs::Local(
                args.library
                    .clone()
                    .unwrap_or_else(utils::font_utils::get_system_font_directories),
            )
        };

        // Deserialize the font configuration from font_config.toml
        let font_config = deserialize_fonts_from_file(&config_file)
            .map_err(|_| "Failed to parse font config file")?;

        // Resolve the absolute path of the project's font directory if specified in font_config.toml
        // Otherwise, use the default relative path "fonts"
        let absolute_font_dir = Self::resolve_font_directory(&config_file, &font_config)?;

        // Initialize the FontSets struct
        let font_sets =
            Self::initialize_font_sets(&library_dirs, &font_config, &absolute_font_dir)?;

        Ok(FontManager {
            config_file,
            font_config,
            library_dirs,
            absolute_font_dir,
            font_sets,
            action,
        })
    }

    fn resolve_config_file(project_or_config: &Path) -> PathBuf {
        if project_or_config.is_dir() {
            project_or_config.join("font_config.toml")
        } else {
            project_or_config.to_path_buf()
        }
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
        library_dirs: &LibraryDirs,
        font_config: &FontConfig,
        font_dir: &Path,
    ) -> Result<FontSets, String> {
        let required = BTreeSet::from_iter(font_config.fonts.clone());
        let current_entries = create_font_entries(font_dir);
        let current = font_entries_to_set(&current_entries);
        let embedded: BTreeSet<TypstFont> = deserialize_fonts_from_toml(EMBEDDED_FONTS)
            .map_err(|_| "Failed to parse embedded fonts")?
            .fonts
            .into_iter()
            .collect();

        let missing = required
            .iter()
            .filter(|font| {
                !embedded.contains(*font) && !font_is_satisfied_by_entries(font, &current_entries)
            })
            .cloned()
            .collect::<BTreeSet<_>>();

        let redundant = current_entries
            .iter()
            .filter(|entry| {
                !required
                    .iter()
                    .any(|font| font_entry_satisfies(entry, font))
            })
            .map(|entry| entry.font.clone())
            .collect();

        let library_entries = create_font_entries_from_dirs(&library_dirs);

        Ok(FontSets {
            required,
            current,
            current_entries,
            embedded,
            missing,
            redundant,
            library_entries,
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
                "◆".bright_green()
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
        self.print_font_set_with(
            "Current fonts",
            &self.font_sets.current,
            |font| {
                if self.font_sets.required.contains(font)
                    || self.current_entry_satisfies_required(font)
                {
                    "●".green()
                } else {
                    "●".blue()
                }
            },
            |font| self.format_current_font(font),
        );

        self.print_font_set("Required fonts", &self.font_sets.required, |font| {
            if self.font_sets.embedded.contains(font) {
                "◆".bright_green()
            } else if font_is_satisfied_by_entries(font, &self.font_sets.current_entries) {
                "●".green()
            } else if self.select_library_candidate(font).is_some() {
                "○".yellow()
            } else {
                "○".red()
            }
        });

        self.print_font_set("Missing fonts", &self.font_sets.missing, |font| {
            if self.select_library_candidate(font).is_some() {
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
        self.print_font_set_with(title, fonts, get_bullet, |font| font.to_string());
    }

    fn print_font_set_with<F, G>(
        &self,
        title: &str,
        fonts: &BTreeSet<TypstFont>,
        get_bullet: F,
        format_font: G,
    ) where
        F: Fn(&TypstFont) -> colored::ColoredString,
        G: Fn(&TypstFont) -> String,
    {
        println!(
            "\n- {} (total {}){}",
            title.bold(),
            fonts.len(),
            if fonts.is_empty() { "" } else { ":" }
        );
        for font in fonts {
            println!("  {} {}", get_bullet(font), format_font(font));
        }
    }

    fn format_current_font(&self, font: &TypstFont) -> String {
        self.font_sets
            .current_entries
            .iter()
            .find(|entry| entry.font == *font)
            .map_or_else(|| font.to_string(), format_discovered_font)
    }

    fn current_entry_satisfies_required(&self, current: &TypstFont) -> bool {
        self.font_sets
            .current_entries
            .iter()
            .filter(|entry| entry.font == *current)
            .any(|entry| {
                self.font_sets
                    .required
                    .iter()
                    .any(|required| font_entry_satisfies(entry, required))
            })
    }

    fn select_library_candidate(&self, font: &TypstFont) -> Option<&DiscoveredFont> {
        select_best_font_entry(font, &self.font_sets.library_entries)
    }

    pub(crate) fn download_font_from_github_path(
        &self,
        font: &TypstFont,
        relative_path: &Path,
    ) -> Result<(), String> {
        let client = Client::new();

        println!("\n- {}", "Downloading fonts from GitHub".bold());

        let github_repo = get_first_two_segments(&relative_path).expect("Invalid GitHub repo path");

        let font_relative_path =
            get_remaining_after_two_segments(&relative_path).expect("Invalid font path");

        let url = format!(
            "https://raw.githubusercontent.com/{}/main/{}",
            github_repo.display(),
            font_relative_path.display()
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
            // Ensure the parent directory exists
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create directories {:?}: {}", parent, e))?;
            }
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

        Ok(())
    }

    pub(crate) fn update_fonts(&self, dry_run: bool) -> Result<(), String> {
        if self.font_sets.missing.is_empty() {
            println!("\nNo missing fonts to update");
            return Ok(());
        }

        if dry_run {
            println!("\n- {}", "Dry run: planned font updates".bold());
        } else {
            println!("\n- {}", "Updating fonts".bold());
        }

        let mut copied_sources = BTreeSet::<PathBuf>::new();

        for font in &self.font_sets.missing {
            // Get the path of the font file in the library
            if let Some(source_entry) = self.select_library_candidate(font) {
                let source_path = &source_entry.path;
                if !copied_sources.insert(source_path.clone()) {
                    continue;
                }

                match self.library_dirs {
                    LibraryDirs::Local(_) => {
                        // dest_path is where the font file will be copied to
                        // it is the project's font directory joined with the file name of the font file
                        let dest_path = self
                            .absolute_font_dir
                            .join(&source_path.file_name().unwrap());
                        println!(
                            "  {} {source_path:?} to {:?}",
                            if dry_run { "Would copy" } else { "Copying" },
                            Path::new(
                                &self
                                    .font_config
                                    .font_dir
                                    .clone()
                                    .unwrap_or_else(|| "fonts".to_string())
                            )
                            .join(&source_path.file_name().unwrap())
                        );
                        if dry_run {
                            continue;
                        }
                        // Copy the font file from the library to the project's font directory
                        fs::copy(&source_path, &dest_path)
                            .map_err(|_| format!("Failed to copy font file: {:?}", font))?;
                    }
                    LibraryDirs::GitHub(_) => {
                        if dry_run {
                            let github_repo = get_first_two_segments(source_path)
                                .expect("Invalid GitHub repo path");
                            let font_relative_path = get_remaining_after_two_segments(source_path)
                                .expect("Invalid font path");
                            let url = format!(
                                "https://raw.githubusercontent.com/{}/main/{}",
                                github_repo.display(),
                                font_relative_path.display()
                            );
                            let dest_path = self
                                .absolute_font_dir
                                .join(source_path.file_name().unwrap());
                            println!("  Would download {url} to {:?}", dest_path);
                            continue;
                        }
                        self.download_font_from_github_path(font, source_path)
                            .expect("Failed to download fonts from GitHub");
                    }
                }
            } else {
                println!("Font not found in source library: {:?}", font);
            }
        }
        Ok(())
    }
}

/// Wrapper struct for serializing/deserializing the library
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypstFontLibrary {
    #[serde(with = "font_map_serde")]
    pub fonts: BTreeMap<TypstFont, PathBuf>,
}

#[derive(Debug, Deserialize)]
struct TypstFontLibraryEntries {
    fonts: Vec<FontLibraryEntryDe>,
}

#[derive(Debug, Deserialize)]
struct FontLibraryEntryDe {
    family_name: String,
    #[serde(default, with = "crate::parse_font_config::typst_font_serde")]
    style: FontStyle,
    #[serde(default)]
    weight: LibraryFontValue<FontWeight>,
    #[serde(default)]
    stretch: LibraryFontValue<FontStretch>,
    #[serde(default)]
    optical_size: Option<LibraryAxisRange<f32>>,
    #[serde(default)]
    axes: Vec<LibraryCustomAxis>,
    path: PathBuf,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(untagged)]
enum LibraryFontValue<T> {
    Fixed(T),
    Range(LibraryAxisRange<T>),
}

impl<T: Default> Default for LibraryFontValue<T> {
    fn default() -> Self {
        Self::Fixed(T::default())
    }
}

impl<T: Copy> LibraryFontValue<T> {
    fn default_value(&self) -> T {
        match self {
            Self::Fixed(value) => *value,
            Self::Range(range) => range.default,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct LibraryAxisRange<T> {
    min: T,
    max: T,
    default: T,
}

#[derive(Debug, Deserialize)]
struct LibraryCustomAxis {
    tag: String,
    min: f32,
    max: f32,
    default: f32,
}

impl FontLibraryEntryDe {
    fn into_discovered(self) -> DiscoveredFont {
        let mut axes = Vec::new();

        if let LibraryFontValue::Range(range) = self.weight {
            axes.push(FontAxis {
                tag: StandardAxes::WGHT,
                min: range.min.to_wght(),
                max: range.max.to_wght(),
                default: range.default.to_wght(),
            });
        }

        if let LibraryFontValue::Range(range) = self.stretch {
            axes.push(FontAxis {
                tag: StandardAxes::WDTH,
                min: range.min.to_wdth(),
                max: range.max.to_wdth(),
                default: range.default.to_wdth(),
            });
        }

        if let Some(range) = self.optical_size {
            axes.push(FontAxis {
                tag: StandardAxes::OPSZ,
                min: AxisValue(range.min),
                max: AxisValue(range.max),
                default: AxisValue(range.default),
            });
        }

        axes.extend(self.axes.into_iter().map(|axis| FontAxis {
            tag: Tag::from_bytes_lossy(axis.tag.as_bytes()),
            min: AxisValue(axis.min),
            max: AxisValue(axis.max),
            default: AxisValue(axis.default),
        }));

        DiscoveredFont {
            font: TypstFont {
                family_name: self.family_name,
                style: self.style,
                weight: self.weight.default_value(),
                stretch: self.stretch.default_value(),
            },
            path: self.path,
            axes,
        }
    }
}

// Wrapper struct for serialization
#[allow(dead_code)]
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

    #[derive(Deserialize)]
    struct FontMapEntryDe {
        family_name: String,
        #[serde(default, with = "crate::parse_font_config::typst_font_serde")]
        style: FontStyle,
        #[serde(default)]
        weight: FontValue<FontWeight>,
        #[serde(default)]
        stretch: FontValue<FontStretch>,
        path: PathBuf,
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum FontValue<T> {
        Fixed(T),
        Range { default: T },
    }

    impl<T: Default> Default for FontValue<T> {
        fn default() -> Self {
            Self::Fixed(T::default())
        }
    }

    impl<T> FontValue<T> {
        fn into_value(self) -> T {
            match self {
                Self::Fixed(value) | Self::Range { default: value } => value,
            }
        }
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
        let entries: Vec<FontMapEntryDe> = Vec::deserialize(deserializer)?;
        Ok(entries
            .into_iter()
            .map(|entry| {
                let font = TypstFont {
                    family_name: entry.family_name,
                    style: entry.style,
                    weight: entry.weight.into_value(),
                    stretch: entry.stretch.into_value(),
                };
                (font, entry.path)
            })
            .collect())
    }
}

#[allow(dead_code)]
pub fn strip_library_root_path(
    font_lib_map: &mut BTreeMap<TypstFont, PathBuf>,
    library_root_path: &Path,
) {
    for path in font_lib_map.values_mut() {
        if let Ok(stripped) = path.strip_prefix(library_root_path) {
            *path = stripped.to_path_buf();
        }
    }
}

pub fn download_font_library_info<P>(github_repo: P) -> Result<String, Box<dyn std::error::Error>>
where
    P: AsRef<Path>,
{
    // Convert the input into a string
    let repo_str = github_repo
        .as_ref()
        .to_str()
        .ok_or_else(|| "Failed to convert path to string")?;

    // Construct the URL to the raw file on GitHub
    let url = format!(
        "https://raw.githubusercontent.com/{}/main/font_library.toml",
        repo_str
    );

    // Send a GET request to fetch the file
    let response = get(&url)?;
    if !response.status().is_success() {
        return Err(format!("Failed to download file: HTTP {}", response.status()).into());
    }

    // Read the response body as text
    let content = response.text()?;

    Ok(content)
}

#[allow(dead_code)]
pub fn get_github_font_library_info<P>(
    github_repo: P,
) -> Result<BTreeMap<TypstFont, PathBuf>, Box<dyn std::error::Error>>
where
    P: AsRef<Path>,
{
    // Download the font library info
    let content =
        download_font_library_info(&github_repo).expect("Failed to download font library info");

    // deserialize the font_library.toml file
    let mut library: TypstFontLibrary =
        toml::from_str(&content).expect("Failed to deserialize from TOML");

    // Prepend the github_repo to the font paths
    for path in library.fonts.values_mut() {
        *path = PathBuf::from(&github_repo.as_ref()).join(&mut *path);
    }

    Ok(library.fonts)
}

pub fn get_github_font_library_entries<P>(
    github_repo: P,
) -> Result<Vec<DiscoveredFont>, Box<dyn std::error::Error>>
where
    P: AsRef<Path>,
{
    let content =
        download_font_library_info(&github_repo).expect("Failed to download font library info");

    let library: TypstFontLibraryEntries =
        toml::from_str(&content).expect("Failed to deserialize from TOML");

    let entries = library
        .fonts
        .into_iter()
        .map(|entry| {
            let mut entry = entry.into_discovered();
            entry.path = PathBuf::from(&github_repo.as_ref()).join(&entry.path);
            entry
        })
        .collect();

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::create_font_path_map_from_dirs;
    use std::collections::BTreeSet;
    use std::env;
    use typst::text::{AxisValue, FontAxis, FontStretch, FontStyle, FontWeight, StandardAxes};

    fn font(family_name: &str, style: FontStyle, weight: u16, stretch: FontStretch) -> TypstFont {
        TypstFont {
            family_name: family_name.to_string(),
            style,
            weight: FontWeight::from_number(weight),
            stretch,
        }
    }

    fn discovered(font: TypstFont, path: &str, axes: Vec<FontAxis>) -> DiscoveredFont {
        DiscoveredFont {
            font,
            path: PathBuf::from(path),
            axes,
        }
    }

    fn axis(tag: typst::text::Tag, min: f32, max: f32, default: f32) -> FontAxis {
        FontAxis {
            tag,
            min: AxisValue(min),
            max: AxisValue(max),
            default: AxisValue(default),
        }
    }

    #[test]
    fn test_variable_font_entry_satisfies_variant_intent() {
        let entry = discovered(
            font("Baskervville", FontStyle::Normal, 400, FontStretch::NORMAL),
            "Baskervville-VariableFont_wght.ttf",
            vec![axis(StandardAxes::WGHT, 400.0, 700.0, 400.0)],
        );

        assert!(font_entry_satisfies(
            &entry,
            &font("Baskervville", FontStyle::Normal, 600, FontStretch::NORMAL)
        ));
        assert!(!font_entry_satisfies(
            &entry,
            &font("Baskervville", FontStyle::Normal, 800, FontStretch::NORMAL)
        ));
    }

    #[test]
    fn test_library_candidate_prefers_variable_over_static() {
        let static_entry = discovered(
            font("Baskervville", FontStyle::Normal, 600, FontStretch::NORMAL),
            "Baskervville-SemiBold.ttf",
            vec![],
        );
        let variable_entry = discovered(
            font("Baskervville", FontStyle::Normal, 400, FontStretch::NORMAL),
            "Baskervville-VariableFont_wght.ttf",
            vec![axis(StandardAxes::WGHT, 400.0, 700.0, 400.0)],
        );
        let entries = vec![static_entry, variable_entry];

        let selected = select_best_font_entry(
            &font("Baskervville", FontStyle::Normal, 600, FontStretch::NORMAL),
            &entries,
        )
        .unwrap();

        assert_eq!(
            selected.path,
            PathBuf::from("Baskervville-VariableFont_wght.ttf")
        );
    }

    #[test]
    fn test_font_status_display_uses_numeric_and_variable_ranges() {
        let fixed = font("Example Fixed", FontStyle::Normal, 400, FontStretch::NORMAL);
        assert!(format!("{fixed}").contains("weight: 400"));
        assert!(!format!("{fixed}").contains("FontWeight"));

        let variable = discovered(
            font(
                "Example Variable",
                FontStyle::Normal,
                400,
                FontStretch::NORMAL,
            ),
            "ExampleVariable.ttf",
            vec![axis(StandardAxes::WGHT, 100.0, 900.0, 400.0)],
        );

        let formatted = format_discovered_font(&variable);
        assert!(formatted.contains("weight: 100-900"));
        assert!(!formatted.contains("FontWeight"));
    }

    #[test]
    fn test_dry_run_update_does_not_copy_local_font() {
        let target_dir = env::var("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("target"));
        let test_dir = target_dir.join("dry_run_update_does_not_copy_local_font");
        fs::remove_dir_all(&test_dir).ok();

        let library_dir = test_dir.join("library");
        let project_dir = test_dir.join("project");
        let source_path = library_dir.join("Example-Regular.ttf");
        let absolute_font_dir = project_dir.join("fonts");
        fs::create_dir_all(&library_dir).unwrap();
        fs::create_dir_all(&project_dir).unwrap();
        fs::write(&source_path, b"not a real font").unwrap();

        let missing_font = font("Example", FontStyle::Normal, 400, FontStretch::NORMAL);
        let manager = FontManager {
            config_file: project_dir.join("font_config.toml"),
            font_config: FontConfig {
                font_dir: Some("fonts".to_string()),
                fonts: vec![missing_font.clone()],
            },
            library_dirs: LibraryDirs::Local(vec![library_dir]),
            absolute_font_dir: absolute_font_dir.clone(),
            font_sets: FontSets {
                required: BTreeSet::from([missing_font.clone()]),
                current: BTreeSet::new(),
                current_entries: Vec::new(),
                embedded: BTreeSet::new(),
                missing: BTreeSet::from([missing_font.clone()]),
                redundant: BTreeSet::new(),
                library_entries: vec![DiscoveredFont {
                    font: missing_font,
                    path: source_path.clone(),
                    axes: Vec::new(),
                }],
            },
            action: "Updating",
        };

        manager.update_fonts(true).unwrap();

        assert!(source_path.exists());
        assert!(!absolute_font_dir.exists());
        assert!(!absolute_font_dir.join("Example-Regular.ttf").exists());
    }

    #[test]
    fn test_resolve_config_file_accepts_project_root_or_config_path() {
        let target_dir = env::var("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("target"));
        let project_dir = target_dir.join("test_project_root");
        fs::create_dir_all(&project_dir).unwrap();

        assert_eq!(
            FontManager::resolve_config_file(&project_dir),
            project_dir.join("font_config.toml")
        );

        let config_path = project_dir.join("custom-fonts.toml");
        assert_eq!(FontManager::resolve_config_file(&config_path), config_path);
    }

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
            fonts: BTreeMap::new(),
        };

        library.fonts.insert(
            TypstFont {
                family_name: "Arial".to_string(),
                style: FontStyle::Normal,
                weight: FontWeight::REGULAR,
                stretch: FontStretch::NORMAL,
            },
            PathBuf::from("fonts/arial.ttf"),
        );

        library.fonts.insert(
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

        assert_eq!(library.fonts, deserialized.fonts);
    }

    #[test]
    fn test_variable_font_library_deserialization_uses_defaults() {
        let toml = r#"[[fonts]]
family_name = "Baskervville"
style = "Normal"
weight = { min = 400, max = 700, default = 400 }
stretch = 1000
path = "Baskervville/Baskervville-VariableFont_wght.ttf"

[[fonts]]
family_name = "Noto Sans"
style = "Italic"
weight = { min = 100, max = 900, default = 400 }
stretch = { min = 750, max = 1250, default = 1000 }
optical_size = { min = 14, max = 32, default = 14 }
axes = [
  { tag = "CRSV", min = 0, max = 1, default = 0 }
]
path = "NotoSans/NotoSans-Italic-VariableFont_wdth,wght.ttf"
"#;

        let library: TypstFontLibrary = toml::from_str(toml).unwrap();

        assert!(library.fonts.contains_key(&TypstFont {
            family_name: "Baskervville".to_string(),
            style: FontStyle::Normal,
            weight: FontWeight::from_number(400),
            stretch: FontStretch::NORMAL,
        }));

        assert!(library.fonts.contains_key(&TypstFont {
            family_name: "Noto Sans".to_string(),
            style: FontStyle::Italic,
            weight: FontWeight::from_number(400),
            stretch: FontStretch::NORMAL,
        }));

        let entries: TypstFontLibraryEntries = toml::from_str(toml).unwrap();
        let discovered = entries
            .fonts
            .into_iter()
            .map(FontLibraryEntryDe::into_discovered)
            .collect::<Vec<_>>();

        assert!(font_is_satisfied_by_entries(
            &font("Baskervville", FontStyle::Normal, 600, FontStretch::NORMAL),
            &discovered
        ));
        assert!(font_is_satisfied_by_entries(
            &font("Noto Sans", FontStyle::Italic, 700, FontStretch::CONDENSED),
            &discovered
        ));
    }

    #[test]
    #[ignore]
    fn test_local_font_library_serialization() {
        use dotenv::dotenv;
        use std::env;
        use std::fs;
        use std::path::PathBuf;

        dotenv().ok(); // Load .env file

        let target_dir = env::var("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("target"));

        let test_dir = target_dir.join("test_outputs");
        fs::create_dir_all(&test_dir).expect("Failed to create test_outputs directory");

        let file_path = test_dir.join("font_library.toml");

        let library_dir = env::var("FONT_LIBRARY_PATH")
            .map(PathBuf::from)
            .expect("FONT_LIBRARY_PATH environment variable is not set");

        let library_dirs = LibraryDirs::Local(vec![library_dir.clone()]);

        let mut font_lib_map = create_font_path_map_from_dirs(&library_dirs);

        strip_library_root_path(&mut font_lib_map, &library_dir);

        let library = TypstFontLibrary {
            fonts: font_lib_map,
        };

        let toml = toml::to_string_pretty(&library).expect("Failed to serialize to TOML");
        fs::write(&file_path, toml.as_bytes()).expect("Failed to write to file");

        println!("TOML written to: {:?}", file_path);
    }

    #[test]
    fn test_download_font_library_info() {
        let github_repo = "hooyuser/Font_Library";
        let content = download_font_library_info(github_repo).unwrap();
        println!("{}", content);

        // deserialize the content
        let library: TypstFontLibrary = toml::from_str(&content).unwrap();
        println!("{:?}", library);
    }
}
