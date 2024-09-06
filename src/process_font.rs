//! Default implementation for searching local and system installed fonts as
//! well as loading embedded default fonts.
//!
//! # Embedded fonts
//! The following fonts are available as embedded fonts via the `embed-fonts`
//! feature flag:
//! - For text: Linux Libertine, New Computer Modern
//! - For math: New Computer Modern Math
//! - For code: Deja Vu Sans Mono

use std::path::PathBuf;
use std::sync::OnceLock;
use std::{fs, path::Path};

use fontdb::{Database, Source};

use typst::text::{Font, FontBook, FontInfo};


/// Holds details about the location of a font and lazily the font itself.
#[derive(Debug)]
pub struct FontSlot {
    /// The path at which the font can be found on the system.
    path: Option<PathBuf>,
    /// The index of the font in its collection. Zero if the path does not point
    /// to a collection.
    index: u32,
    /// The lazily loaded font.
    font: OnceLock<Option<Font>>,
}

impl FontSlot {
    /// Returns the path at which the font can be found on the system, or `None`
    /// if the font was embedded.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Returns the index of the font in its collection. Zero if the path does
    /// not point to a collection.
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Get the font for this slot. This loads the font into memory on first
    /// access.
    pub fn get(&self) -> Option<Font> {
        self.font
            .get_or_init(|| {
                let data = fs::read(
                    self.path
                        .as_ref()
                        .expect("`path` is not `None` if `font` is uninitialized"),
                )
                    .ok()?
                    .into();
                Font::new(data, self.index)
            })
            .clone()
    }
}

/// The result of a font search, created by calling [`FontSearcher::search`].
#[derive(Debug)]
pub struct Fonts {
    /// Metadata about all discovered fonts.
    pub book: FontBook,
    /// Slots that the fonts are loaded into.
    pub fonts: Vec<FontSlot>,
}

impl Fonts {
    /// Creates a new font searcer with the default settings.
    pub fn searcher() -> FontSearcher {
        FontSearcher::new()
    }
}

/// Searches for fonts.
///
/// Fonts are added in the following order (descending priority):
/// 1. Font directories
/// 2. System fonts (if included & enabled)
/// 3. Embedded fonts (if enabled)
#[derive(Debug)]
pub struct FontSearcher {
    db: Database,
    book: FontBook,
    fonts: Vec<FontSlot>,
}

impl FontSearcher {
    /// Create a new, empty system searcher. The searcher is created with the
    /// default configuration, it will include embedded fonts and system fonts.
    pub fn new() -> Self {
        Self {
            db: Database::new(),
            book: FontBook::new(),
            fonts: vec![],
        }
    }


    /// Start searching for and loading fonts. To additionally load fonts
    /// from specific directories, use [`search_with`][Self::search_with].
    ///
    /// # Examples
    /// ```no_run
    /// # use typst_kit::fonts::FontSearcher;
    /// let fonts = FontSearcher::new()
    ///     .include_system_fonts(true)
    ///     .search();
    /// ```
    // pub fn search(&mut self) -> Fonts {
    //     self.search_dirs::<_, &str>([])
    // }

    /// Start searching for and loading fonts, with additional directories.
    ///
    /// # Examples
    /// ```no_run
    /// # use typst_kit::fonts::FontSearcher;
    /// let fonts = FontSearcher::new()
    ///     .include_system_fonts(true)
    ///     .search_with(["./assets/fonts/"]);
    /// ```
    // pub fn search_dirs<I, P>(&mut self, font_dirs: I) -> Fonts
    // where
    //     I: IntoIterator<Item=P>,
    //     P: AsRef<Path>,
    // {
    //     // Font paths have the highest priority.
    //     for path in font_dirs {
    //         self.db.load_fonts_dir(path);
    //     }
    //
    //     for face in self.db.faces() {
    //         let path = match &face.source {
    //             Source::File(path) | Source::SharedFile(path, _) => path,
    //             // We never add binary sources to the database, so there
    //             // shouldn't be any.
    //             Source::Binary(_) => continue,
    //         };
    //
    //         let info = self
    //             .db
    //             .with_face_data(face.id, FontInfo::new)
    //             .expect("database must contain this font");
    //
    //         if let Some(info) = info {
    //             self.book.push(info);
    //             self.fonts.push(FontSlot {
    //                 path: Some(path.clone()),
    //                 index: face.index,
    //                 font: OnceLock::new(),
    //             });
    //         }
    //     }
    //
    //     Fonts {
    //         book: std::mem::take(&mut self.book),
    //         fonts: std::mem::take(&mut self.fonts),
    //     }
    // }

    pub fn search_file<P: AsRef<Path>>(&mut self, font_path: P) -> Fonts
    {
        // Font paths have the highest priority.
        self.db.load_font_file(&font_path).unwrap();


        for face in self.db.faces() {
            let path = match &face.source {
                Source::File(path) | Source::SharedFile(path, _) => path,
                // We never add binary sources to the database, so there
                // shouln't be any.
                Source::Binary(_) => continue,
            };

            let info = self
                .db
                .with_face_data(face.id, FontInfo::new)
                .expect("database must contain this font");

            if let Some(info) = info {
                self.book.push(info);
                self.fonts.push(FontSlot {
                    path: Some(path.clone()),
                    index: face.index,
                    font: OnceLock::new(),
                });
            }
        }

        Fonts {
            book: std::mem::take(&mut self.book),
            fonts: std::mem::take(&mut self.fonts),
        }
    }
}

impl Default for FontSearcher {
    fn default() -> Self {
        Self::new()
    }
}
