use std::fs;
use std::path::{Path, PathBuf};

use crate::error::AppError;

const URDU_FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoNastaliqUrdu-VF.ttf");

/// Materializes the embedded Noto Nastaliq Urdu font and returns an ordered
/// list of candidate font files for the PDF renderer.
pub fn resolve_fonts(fonts_dir: &Path) -> Result<Vec<PathBuf>, AppError> {
    let urdu_path = fonts_dir.join("NotoNastaliqUrdu-Regular.ttf");
    if !urdu_path.exists() {
        fs::write(&urdu_path, URDU_FONT_BYTES)?;
    }

    let mut candidates = vec![urdu_path];

    // Fallbacks for environments where the embedded font cannot be parsed
    // (e.g. variable-font handling differences between PDF backends).
    for name in [
        "C:\\Windows\\Fonts\\tahoma.ttf",
        "C:\\Windows\\Fonts\\arial.ttf",
    ] {
        let p = PathBuf::from(name);
        if p.exists() {
            candidates.push(p);
        }
    }

    Ok(candidates)
}
