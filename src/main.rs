mod process_font;
mod parse_font_config;


use std::path::PathBuf;
use typst::diag::StrResult;
use typst::text::{FontStretch, FontStyle, FontVariant, FontWeight};

use walkdir::WalkDir;

struct TypstFont {
    family_name: String,
    style: FontStyle,
    weight: FontWeight,
    stretch: FontStretch,
}

pub fn fonts() -> StrResult<()> {
    let font_paths: Vec<PathBuf> = vec!["./assets/fonts/".into()];

    for font_path in font_paths {
        // Walk through the directory recursively
        for entry in WalkDir::new(&font_path).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();

            if path.is_file() {
                // Print the file name
                if let Some(file_name) = path.file_name() {
                    println!("Processing {}", file_name.to_string_lossy());
                    let fonts = process_font::Fonts::searcher().search_file(&path);

                    for (name, infos) in fonts.book.families() {
                        println!("{name}");

                        for info in infos {
                            let FontVariant { style, weight, stretch } = info.variant;
                            println!("- Style: {style:?}, Weight: {weight:?}, Stretch: {stretch:?}");
                        }
                    }
                }
            }
        }
    }

    Ok(())
}


fn main() {
    fonts().unwrap();
    println!("Hello, world!");
}
