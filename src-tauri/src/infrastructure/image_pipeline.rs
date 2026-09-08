use std::fs;
use std::path::Path;

use image::{ImageFormat, ImageReader};

use crate::error::AppError;

pub struct ImportedImage {
    pub original_path: String,
    pub stored_name: String,
    pub width: u32,
    pub height: u32,
    pub thumbnail_bytes: u64,
}

const MAX_IMAGE_PIXELS: u64 = 40_000_000; // ~6000x6000
const THUMBNAIL_SIZE: u32 = 240;

/// Validates an image by decoding, re-encodes it as WebP, and writes a
/// thumbnail beside it. Filenames are server-generated (UUID v7), never the
/// user-supplied name.
pub fn import_image(source: &Path, images_dir: &Path) -> Result<ImportedImage, AppError> {
    if !source.is_file() {
        return Err(AppError::Validation("selected file does not exist".into()));
    }

    let reader = ImageReader::open(source)?;
    let format = reader
        .format()
        .ok_or_else(|| AppError::Image("unrecognized or corrupt image".into()))?;

    match format {
        ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP => {}
        other => {
            return Err(AppError::Image(format!(
                "unsupported format {other:?}; only JPEG, PNG, WebP are allowed"
            )))
        }
    }

    let img = reader
        .decode()
        .map_err(|e| AppError::Image(format!("decode failed: {e}")))?;

    let (width, height) = (img.width(), img.height());
    if width == 0 || height == 0 {
        return Err(AppError::Image("image has invalid dimensions".into()));
    }
    if (width as u64) * (height as u64) > MAX_IMAGE_PIXELS {
        return Err(AppError::Image(
            "image exceeds the maximum pixel dimension".into(),
        ));
    }

    let stem = uuid::Uuid::now_v7().to_string();
    let stored_name = format!("{stem}.webp");

    // Re-encode the validated original as (lossy) WebP.
    let original_path = images_dir.join(&stored_name);
    img.save_with_format(&original_path, ImageFormat::WebP)
        .map_err(|e| AppError::Image(format!("re-encode failed: {e}")))?;

    // Thumbnail keeps the rendering light for catalogue grids.
    let thumb = img.thumbnail(THUMBNAIL_SIZE, THUMBNAIL_SIZE);
    let thumb_name = format!("{stem}-thumb.webp");
    let thumb_path = images_dir.join(&thumb_name);
    encode_webp(&thumb, &thumb_path)?;

    let thumbnail_bytes = fs::metadata(&thumb_path)?.len();

    Ok(ImportedImage {
        original_path: source.to_string_lossy().into_owned(),
        stored_name,
        width,
        height,
        thumbnail_bytes,
    })
}

fn encode_webp(img: &image::DynamicImage, path: &Path) -> Result<(), AppError> {
    img.save_with_format(path, ImageFormat::WebP)
        .map_err(|e| AppError::Image(format!("thumbnail encode failed: {e}")))
}