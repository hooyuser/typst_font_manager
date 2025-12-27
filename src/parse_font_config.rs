use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use anyhow::Result;
use toml::Value;
use typst::text::{FontStretch, FontStyle, FontWeight};

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub struct TypstFont {
    pub(crate) family_name: String,
    #[serde(default, with = "typst_font_serde")]
    pub(crate) style: FontStyle,
    #[serde(default)]
    pub(crate) weight: FontWeight,
    #[serde(default)]
    pub(crate) stretch: FontStretch,
}

impl fmt::Display for TypstFont {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let stretch = (self.stretch.to_ratio().get() * 1000.0) as u16;
        write!(
            f,
            "{:<30}    (style: {:?}, weight: {:?}, stretch: {})",
            self.family_name, self.style, self.weight, stretch
        )
    }
}

mod typst_font_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use typst::text::FontStyle;

    //Custom serializer for FontStyle
    pub fn serialize<S>(style: &FontStyle, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let style_str = match style {
            FontStyle::Normal => "Normal",
            FontStyle::Italic => "Italic",
            FontStyle::Oblique => "Oblique",
        };
        serializer.serialize_str(style_str)
    }

    //Custom deserializer for FontStyle
    pub fn deserialize<'de, D>(deserializer: D) -> Result<FontStyle, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "normal" => Ok(FontStyle::Normal),
            "italic" => Ok(FontStyle::Italic),
            "oblique" => Ok(FontStyle::Oblique),
            _ => Err(serde::de::Error::custom(format!(
                "Invalid FontStyle: {}",
                s
            ))),
        }
    }
}

// This struct represents the font configuration of a project, i.e. font_config.toml
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct FontConfig {
    #[serde(default)]
    pub(crate) font_dir: Option<String>, // Path to the font directory of the project
    pub(crate) fonts: Vec<TypstFont>, // List of fonts required by the project
}

/// Function to deserialize TOML string into a Vec of TypstFont
pub fn deserialize_fonts_from_toml(toml_content: &str) -> Result<FontConfig> {
    let font_config: FontConfig = toml::from_str(preprocess_font_config(toml_content)?.as_str())?;
    Ok(font_config)
}

/// Function to read a TOML file and deserialize it into Vec<TypstFont>
pub fn deserialize_fonts_from_file<P: AsRef<Path>>(file_path: P) -> Result<FontConfig> {
    let mut file = File::open(file_path).expect("Font config file not found");
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    deserialize_fonts_from_toml(&content)
}

#[allow(dead_code)]
pub fn serialize_fonts_to_toml(font_config: FontConfig) -> Result<String> {
    let toml_string = toml::to_string(&font_config)?;
    Ok(toml_string)
}

// Function to preprocess the font configuration TOML string,
// expanding the "weight" field if it is an array
fn preprocess_font_config(toml_str: &str) -> Result<String> {
    // Parse the TOML string into a Value
    let mut toml_value: Value = toml::from_str(toml_str)?;

    // Process the TOML data
    if let Some(fonts) = toml_value.get("fonts") {
        // Extract the "fonts" section in the original TOML structure
        if let Some(fonts_array) = fonts.as_array() {
            let mut expanded_fonts = Vec::new();

            // Iterate over each font entry
            for font in fonts_array {
                // Check if weight exists
                if let Some(weight) = font.get("weight") {
                    // If weight is an array, expand it
                    if let Some(weights) = weight.as_array() {
                        for w in weights {
                            let mut new_font = font.clone();
                            if let Some(map) = new_font.as_table_mut() {
                                map.insert("weight".to_string(), w.clone());
                            }
                            expanded_fonts.push(Value::Table(new_font.as_table().unwrap().clone()));
                        }
                    } else {
                        expanded_fonts.push(font.clone());
                    }
                } else {
                    // If no weight field, just push the original font entry
                    expanded_fonts.push(font.clone());
                }
            }

            // Get a mutable reference of the TOML table
            if let Some(table) = toml_value.as_table_mut() {
                // Replace the original "fonts" section with the expanded fonts
                table.insert("fonts".to_string(), Value::Array(expanded_fonts));
            }
        }
    }

    // Convert back to TOML string
    let new_toml_string = toml::to_string(&toml_value)?;

    // Return the updated TOML as a string
    Ok(new_toml_string)
}

// add test
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_serialize_fonts_to_toml() {
        let fonts_config = FontConfig {
            font_dir: Some("fonts".into()),
            fonts: vec![
                TypstFont {
                    family_name: "Arial".to_string(),
                    style: FontStyle::Normal,
                    weight: FontWeight::from_number(400),
                    stretch: FontStretch::NORMAL,
                },
                TypstFont {
                    family_name: "Times New Roman".to_string(),
                    style: FontStyle::Italic,
                    weight: FontWeight::from_number(700),
                    stretch: FontStretch::ULTRA_EXPANDED,
                },
            ],
        };

        let toml_string = serialize_fonts_to_toml(fonts_config).unwrap();
        let expected_toml = r#"font_dir = "fonts"

[[fonts]]
family_name = "Arial"
style = "Normal"
weight = 400
stretch = 1000

[[fonts]]
family_name = "Times New Roman"
style = "Italic"
weight = 700
stretch = 2000
"#;
        assert_eq!(toml_string, expected_toml);
    }

    #[test]
    fn test_deserialize_fonts_from_toml() {
        let toml_string = r#"[[fonts]]
family_name = "Noto Sans"


[[fonts]]
family_name = "Stix Two Text"
style = "Italic"

weight = 700
stretch = 1250

[[fonts]]
family_name = "Lato"
style = "Italic"
weight = [500, 700]

"#;

        let font_config = deserialize_fonts_from_toml(toml_string).unwrap();
        let expected_fonts = vec![
            TypstFont {
                family_name: "Noto Sans".to_string(),
                style: FontStyle::Normal,
                weight: FontWeight::from_number(400),
                stretch: FontStretch::NORMAL,
            },
            TypstFont {
                family_name: "Stix Two Text".to_string(),
                style: FontStyle::Italic,
                weight: FontWeight::from_number(700),
                stretch: FontStretch::EXPANDED,
            },
            TypstFont {
                family_name: "Lato".to_string(),
                style: FontStyle::Italic,
                weight: FontWeight::from_number(500),
                stretch: FontStretch::NORMAL,
            },
            TypstFont {
                family_name: "Lato".to_string(),
                style: FontStyle::Italic,
                weight: FontWeight::from_number(700),
                stretch: FontStretch::NORMAL,
            },
        ];

        assert_eq!(font_config.fonts, expected_fonts);
        assert_eq!(font_config.font_dir, None);
    }

    #[test]
    #[ignore]
    fn test_deserialize_fonts_from_file() {
        let config_file =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/font_configs/font_config.toml");
        // check if the file exists
        assert!(config_file.exists());
        let font_config = deserialize_fonts_from_file(&config_file).unwrap();
        let expected_fonts = vec![
            TypstFont {
                family_name: "Arial".to_string(),
                style: FontStyle::Normal,
                weight: FontWeight::from_number(400),
                stretch: FontStretch::NORMAL,
            },
            TypstFont {
                family_name: "Times New Roman".to_string(),
                style: FontStyle::Italic,
                weight: FontWeight::from_number(700),
                stretch: FontStretch::ULTRA_EXPANDED,
            },
        ];

        assert_eq!(font_config.fonts, expected_fonts);
        assert_eq!(font_config.font_dir.unwrap(), "fonts".to_string());
    }
}
