mod command;
mod font_manager;
mod parse_font_config;
mod process_font;
mod utils;

use clap::Parser;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use typst::text::{AxisValue, FontAxis, FontStretch, FontVariant, FontWeight, StandardAxes};
use walkdir::WalkDir;

use crate::command::{Commands, FontCommand};
use crate::font_manager::{LibraryDirs, get_github_font_library_entries};
use crate::parse_font_config::TypstFont;

#[derive(Clone, Debug)]
pub(crate) struct DiscoveredFont {
    pub(crate) font: TypstFont,
    pub(crate) path: PathBuf,
    pub(crate) axes: Vec<FontAxis>,
}

#[derive(Debug)]
struct FontLibraryExport {
    fonts: Vec<FontLibraryEntry>,
}

#[derive(Debug)]
struct FontLibraryEntry {
    family_name: String,
    style: String,
    weight: FontProperty<u16>,
    stretch: FontProperty<u16>,
    optical_size: Option<AxisRange<AxisNumber>>,
    axes: Vec<CustomAxis>,
    path: PathBuf,
}

#[derive(Debug)]
enum FontProperty<T> {
    Fixed(T),
    Range(AxisRange<T>),
}

#[derive(Clone, Copy, Debug)]
struct AxisRange<T> {
    min: T,
    max: T,
    default: T,
}

#[derive(Debug)]
struct CustomAxis {
    tag: String,
    min: AxisNumber,
    max: AxisNumber,
    default: AxisNumber,
}

#[derive(Clone, Copy, Debug)]
struct AxisNumber(f32);

pub fn create_font_path_map<P: AsRef<Path>>(font_dir: P) -> BTreeMap<TypstFont, PathBuf> {
    font_entries_to_path_map(create_font_entries(font_dir))
}

pub(crate) fn create_font_entries<P: AsRef<Path>>(font_dir: P) -> Vec<DiscoveredFont> {
    let mut fonts = Vec::new();

    // Walk through the directory recursively
    for entry in WalkDir::new(&font_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();

        font_entries_update(&mut fonts, path);
    }

    fonts
}

#[allow(dead_code)]
pub(crate) fn create_font_path_map_from_dirs(
    library_dirs: &LibraryDirs,
) -> BTreeMap<TypstFont, PathBuf> {
    font_entries_to_path_map(create_font_entries_from_dirs(library_dirs))
}

pub(crate) fn create_font_entries_from_dirs(library_dirs: &LibraryDirs) -> Vec<DiscoveredFont> {
    let mut fonts = Vec::new();

    match library_dirs {
        LibraryDirs::GitHub(github_repos) => {
            for github_repo in github_repos {
                // github_repo is a string like "owner/repo"
                let github_font_entries = get_github_font_library_entries(&github_repo)
                    .expect("Error Occurs when getting fonts from GitHub");
                fonts.extend(github_font_entries);
            }
        }
        LibraryDirs::Local(font_dirs) => {
            for font_dir in font_dirs {
                for entry in WalkDir::new(&font_dir).into_iter().filter_map(|e| e.ok()) {
                    let path = entry.path();

                    font_entries_update(&mut fonts, path);
                }
            }
        }
    }

    fonts
}

fn font_entries_to_path_map<I>(fonts: I) -> BTreeMap<TypstFont, PathBuf>
where
    I: IntoIterator<Item = DiscoveredFont>,
{
    fonts
        .into_iter()
        .map(|entry| (entry.font, entry.path))
        .collect()
}

fn font_entries_update(fonts: &mut Vec<DiscoveredFont>, path: &Path) {
    if path.is_file() {
        // Print the file name
        if let Some(_file_name) = path.file_name() {
            //println!("Processing [{}]", &file_name.to_string_lossy());
            let searched = process_font::Fonts::searcher().search_file(&path);

            for info in searched.infos {
                let FontVariant {
                    style,
                    weight,
                    stretch,
                } = info.variant;
                //println!("- Style: {style:?}, Weight: {weight}, Stretch: {stretch}\n");

                let font = TypstFont {
                    family_name: info.family,
                    style,
                    weight,
                    stretch,
                };

                fonts.push(DiscoveredFont {
                    font,
                    path: path.to_path_buf(),
                    axes: info.axes,
                });
            }
        }
    }
}

fn strip_font_entry_root_paths(fonts: &mut [DiscoveredFont], library_root_path: &Path) {
    for font in fonts {
        if let Ok(stripped) = font.path.strip_prefix(library_root_path) {
            font.path = stripped.to_path_buf();
        }
    }
}

impl From<DiscoveredFont> for FontLibraryEntry {
    fn from(entry: DiscoveredFont) -> Self {
        let standard = StandardAxes::parse(&entry.axes);

        let weight = standard
            .wght
            .map_or(FontProperty::Fixed(entry.font.weight.to_number()), |axis| {
                FontProperty::Range(weight_range(axis))
            });

        let stretch = standard.wdth.map_or(
            FontProperty::Fixed(stretch_to_number(entry.font.stretch)),
            |axis| FontProperty::Range(stretch_range(axis)),
        );

        let optical_size = standard.opsz.map(axis_number_range);

        let axes = entry
            .axes
            .iter()
            .filter(|axis| !StandardAxes::knows(axis.tag))
            .map(|axis| CustomAxis {
                tag: axis.tag.to_str_lossy().to_string(),
                min: AxisNumber(axis.min.0),
                max: AxisNumber(axis.max.0),
                default: AxisNumber(axis.default.0),
            })
            .collect();

        Self {
            family_name: entry.font.family_name,
            style: format!("{:?}", entry.font.style),
            weight,
            stretch,
            optical_size,
            axes,
            path: entry.path,
        }
    }
}

impl From<Vec<DiscoveredFont>> for FontLibraryExport {
    fn from(mut fonts: Vec<DiscoveredFont>) -> Self {
        fonts.sort_by(|a, b| {
            (
                a.font.family_name.to_lowercase(),
                a.font.style,
                a.font.weight,
                a.font.stretch,
                &a.path,
            )
                .cmp(&(
                    b.font.family_name.to_lowercase(),
                    b.font.style,
                    b.font.weight,
                    b.font.stretch,
                    &b.path,
                ))
        });

        Self {
            fonts: fonts.into_iter().map(FontLibraryEntry::from).collect(),
        }
    }
}

impl FontLibraryExport {
    fn to_toml_string(&self) -> String {
        let mut toml = String::new();

        for (index, font) in self.fonts.iter().enumerate() {
            if index > 0 {
                toml.push('\n');
            }

            toml.push_str("[[fonts]]\n");
            writeln!(toml, "family_name = {}", toml_string(&font.family_name)).unwrap();
            writeln!(toml, "style = {}", toml_string(&font.style)).unwrap();
            writeln!(toml, "weight = {}", font.weight.to_toml()).unwrap();
            writeln!(toml, "stretch = {}", font.stretch.to_toml()).unwrap();

            if let Some(optical_size) = font.optical_size {
                writeln!(
                    toml,
                    "optical_size = {}",
                    optical_size.to_toml(AxisNumber::to_toml)
                )
                .unwrap();
            }

            if !font.axes.is_empty() {
                toml.push_str("axes = [\n");
                for (axis_index, axis) in font.axes.iter().enumerate() {
                    let suffix = if axis_index + 1 < font.axes.len() {
                        ","
                    } else {
                        ""
                    };
                    writeln!(
                        toml,
                        "  {{ tag = {}, min = {}, max = {}, default = {} }}{suffix}",
                        toml_string(&axis.tag),
                        axis.min.to_toml(),
                        axis.max.to_toml(),
                        axis.default.to_toml()
                    )
                    .unwrap();
                }
                toml.push_str("]\n");
            }

            writeln!(
                toml,
                "path = {}",
                toml_string(font.path.to_string_lossy().as_ref())
            )
            .unwrap();
        }

        toml
    }
}

impl FontProperty<u16> {
    fn to_toml(&self) -> String {
        match self {
            Self::Fixed(value) => value.to_string(),
            Self::Range(range) => range.to_toml(|value| value.to_string()),
        }
    }
}

impl<T> AxisRange<T>
where
    T: Copy,
{
    fn to_toml(self, show: impl Fn(T) -> String) -> String {
        format!(
            "{{ min = {}, max = {}, default = {} }}",
            show(self.min),
            show(self.max),
            show(self.default)
        )
    }
}

impl AxisNumber {
    fn to_toml(self) -> String {
        let value = (self.0 * 100.0).round() / 100.0;
        let rounded = value.round();
        if (value - rounded).abs() < f32::EPSILON {
            return (rounded as i64).to_string();
        }

        let mut text = format!("{value:.2}");
        while text.contains('.') && text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
        text
    }
}

fn toml_string(value: &str) -> String {
    toml::Value::String(value.to_string()).to_string()
}

fn weight_range(axis: &FontAxis) -> AxisRange<u16> {
    AxisRange {
        min: FontWeight::from_wght(axis.min).to_number(),
        max: FontWeight::from_wght(axis.max).to_number(),
        default: FontWeight::from_wght(axis.default).to_number(),
    }
}

fn stretch_range(axis: &FontAxis) -> AxisRange<u16> {
    AxisRange {
        min: stretch_to_number(FontStretch::from_wdth(axis.min)),
        max: stretch_to_number(FontStretch::from_wdth(axis.max)),
        default: stretch_to_number(FontStretch::from_wdth(axis.default)),
    }
}

fn axis_number_range(axis: &FontAxis) -> AxisRange<AxisNumber> {
    AxisRange {
        min: AxisNumber(axis.min.0),
        max: AxisNumber(axis.max.0),
        default: AxisNumber(axis.default.0),
    }
}

fn stretch_to_number(stretch: FontStretch) -> u16 {
    (stretch.to_ratio().get() * 1000.0) as u16
}

fn print_font_variants(fonts: &[DiscoveredFont]) {
    let mut families = BTreeMap::<String, Vec<&DiscoveredFont>>::new();
    for font in fonts {
        families
            .entry(font.font.family_name.to_lowercase())
            .or_default()
            .push(font);
    }

    for (index, family_fonts) in families.values().enumerate() {
        if let Some(first) = family_fonts.first() {
            println!("{}", first.font.family_name);
        }

        let mut family_fonts = family_fonts.iter().peekable();
        while let Some(entry) = family_fonts.next() {
            let last = family_fonts.peek().is_none();
            print_font_variant(entry, last);
        }

        if index + 1 < families.len() {
            println!();
        }
    }
}

fn print_font_variant(entry: &DiscoveredFont, last: bool) {
    let marker = if last { '└' } else { '├' };
    let pad = if last { "     " } else { "  │  " };
    let path = entry.path.display();

    if entry.axes.is_empty() {
        println!("  {marker} {path}");
        println!(
            "{pad} Style: {:?}, Weight: {}, Stretch: {}",
            entry.font.style, entry.font.weight, entry.font.stretch
        );
    } else {
        println!("  {marker} {path} (Variable)");
        let mut axes = entry.axes.clone();
        axes.sort_by_key(|axis| StandardAxes::order(axis.tag));

        let standard = StandardAxes::parse(&axes);
        if standard.ital.is_none() && standard.slnt.is_none() {
            println!("{pad} Style: {:?}", entry.font.style);
        }
        if standard.wght.is_none() {
            println!("{pad} Weight: {}", entry.font.weight);
        }
        if standard.wdth.is_none() {
            println!("{pad} Stretch: {}", entry.font.stretch);
        }
        for axis in &axes {
            println!("{pad} {}", format_axis(axis));
        }
    }

    if !last {
        println!("  │");
    }
}

fn format_axis(axis: &FontAxis) -> String {
    use std::convert::identity;

    match axis.tag {
        StandardAxes::ITAL => {
            format_axis_with(axis, "Italic", |value| format!("{}", identity(value)))
        }
        StandardAxes::SLNT => {
            format_axis_with(axis, "Slant", |value| format!("{}", identity(value)))
        }
        StandardAxes::WGHT => format_axis_with(axis, "Weight", |value| {
            format!("{}", FontWeight::from_wght(value))
        }),
        StandardAxes::WDTH => format_axis_with(axis, "Stretch", |value| {
            format!("{}", FontStretch::from_wdth(value))
        }),
        StandardAxes::OPSZ => format_axis_with(axis, "Optical Size", |value| format!("{value}pt")),
        _ => {
            let name = axis.tag.to_str_lossy();
            format_axis_with(axis, &name, |value| format!("{value}"))
        }
    }
}

fn format_axis_with(axis: &FontAxis, name: &str, show: impl Fn(AxisValue) -> String) -> String {
    format!(
        "{name}: {}-{} (Default: {})",
        show(axis.min),
        show(axis.max),
        show(axis.default)
    )
}

/// Typst Font Manager
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

fn process_command(args: &FontCommand, action: &str, dry_run: bool) {
    args.validate().unwrap();
    match font_manager::FontManager::new(args, action) {
        Ok(font_manager) => {
            font_manager.print_status();

            if action == "Updating" {
                if let Err(e) = font_manager.update_fonts(dry_run) {
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
        Commands::Check(args) => process_command(args, "Checking", false),
        Commands::Update(args) => process_command(&args.font, "Updating", args.dry_run),
        Commands::CheckLib(args) => {
            let library_dirs = if args.github {
                LibraryDirs::GitHub(args.library.clone().unwrap())
            } else {
                LibraryDirs::Local(match &args.library {
                    Some(dirs) => dirs.clone(),
                    None => utils::font_utils::get_system_font_directories(),
                })
            };
            let font_entries = create_font_entries_from_dirs(&library_dirs);

            println!("\n=== Font Library ===\n");

            println!("\n- Font library directories:");
            for dir in &library_dirs {
                println!("  {dir:?}");
            }
            println!("\n- Font Info:");

            print_font_variants(&font_entries);

            if let Some(output_dir_arg) = &args.output {
                match library_dirs {
                    LibraryDirs::GitHub(_) => {}
                    LibraryDirs::Local(library_dirs) => {
                        // if length of library_dirs is greater than 1, print an error message
                        if library_dirs.len() > 1 {
                            println!(
                                "Error: If output directory is provided, there should be only one library directory."
                            );
                            return;
                        }

                        // if output_dir is provided, write the font library info to the output directory
                        // otherwise, write to the library_dirs[0]
                        let output_dir = match &output_dir_arg {
                            Some(dir) => dir.clone(),
                            None => library_dirs[0].clone(),
                        };

                        let mut output_entries = font_entries.clone();
                        // For the output toml file, strip the library root path
                        strip_font_entry_root_paths(&mut output_entries, &output_dir);

                        let library = FontLibraryExport::from(output_entries);
                        // Serialize to TOML and write to the target directory
                        let toml = library.to_toml_string();

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
