use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fs::File;
use std::io::Read;
use std::error::Error;
use serde::ser::SerializeStruct;
use typst::text::{FontStretch, FontStyle, FontWeight};

#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct TypstFont {
    family_name: String,
    #[serde(default, with = "typst_font_serde")]
    style: FontStyle,
    #[serde(default)]
    weight: FontWeight,
    #[serde(default)]
    stretch: FontStretch,
}

mod typst_font_serde {
    use typst::text::FontStyle;
    use serde::{Deserialize, Deserializer, Serializer};

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
            _ => Err(serde::de::Error::custom(format!("Invalid FontStyle: {}", s))),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct FontConfig {
    fonts: Vec<TypstFont>,
}


/// Function to deserialize TOML string into a Vec of TypstFont
pub fn deserialize_fonts_from_toml(toml_content: &str) -> Result<FontConfig, Box<dyn Error>> {
    let font_config: FontConfig = toml::from_str(toml_content)?;
    Ok(font_config)
}

/// Function to read a TOML file and deserialize it into Vec<TypstFont>
pub fn deserialize_fonts_from_file(file_path: &str) -> Result<FontConfig, Box<dyn Error>> {
    let mut file = File::open(file_path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    deserialize_fonts_from_toml(&content)
}

pub fn serialize_fonts_to_toml(font_config: FontConfig) -> Result<String, Box<dyn Error>> {
    let toml_string = toml::to_string(&font_config)?;
    Ok(toml_string)
}

// add test
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_fonts_to_toml() {
        let fonts_config = FontConfig {
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
        let expected_toml = r#"[[fonts]]
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
        ];

        assert_eq!(font_config.fonts, expected_fonts);
    }
}