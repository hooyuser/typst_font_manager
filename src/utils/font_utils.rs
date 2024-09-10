

use std::path::PathBuf;

pub fn get_system_font_directories() -> Vec<PathBuf> {
    let mut font_dirs = Vec::new();

    if cfg!(target_os = "windows") {
        // Windows font directories
        if let Some(windir) = std::env::var_os("WINDIR") {
            font_dirs.push(PathBuf::from(windir).join("Fonts"));
        }
    } else if cfg!(target_os = "macos") {
        // macOS font directories
        font_dirs.extend_from_slice(&[
            PathBuf::from("/System/Library/Fonts"),
            PathBuf::from("/Library/Fonts"),
            PathBuf::from(std::env::var("HOME").unwrap_or_default()).join("Library/Fonts"),
        ]);
    } else if cfg!(target_os = "linux") {
        // Linux font directories
        font_dirs.extend_from_slice(&[
            PathBuf::from("/usr/share/fonts"),
            PathBuf::from("/usr/local/share/fonts"),
            PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".fonts"),
        ]);
    }

    // Filter out directories that don't exist
    font_dirs.retain(|path| path.exists());

    font_dirs
}
