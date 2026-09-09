use std::fs;
use std::path::Path;

use image::metadata::Orientation;
use image::{ImageDecoder, ImageFormat, ImageReader};
use sha2::{Digest, Sha256};

use crate::error::AppError;

pub struct ImportedImage {
    pub original_path: String,
    pub stored_name: String,
    pub thumbnail_name: String,
    pub width: u32,
    pub height: u32,
    pub mime_type: String,
    pub sha256: String,
    pub thumbnail_bytes: u64,
}

const MAX_SOURCE_PIXELS: u64 = 40_000_000; // ~6000x6000 before resizing
const MAX_IMAGE_DIMENSION: u32 = 3000; // long edge of the stored "original"
const THUMBNAIL_SIZE: u32 = 240;
pub const MAX_PRODUCT_IMAGES: i64 = 8;

/// Validates an image by decoding, corrects orientation (EXIF), caps the long
/// edge, re-encodes it as WebP (which strips embedded metadata), and writes a
/// thumbnail beside it. Filenames are server-generated (UUID v7), never the
/// user-supplied name. Returns the relative file names plus derived metadata.
pub fn import_image(source: &Path, images_dir: &Path) -> Result<ImportedImage, AppError> {
    if !source.is_file() {
        return Err(AppError::Validation("selected file does not exist".into()));
    }

    let bytes = fs::read(source)?;
    let mut reader = ImageReader::new(std::io::Cursor::new(&bytes));
    reader = reader
        .with_guessed_format()
        .map_err(|e| AppError::Image(format!("unrecognized or corrupt image: {e}")))?;
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

    // Read just the header dimensions first (no pixel decoding) so a crafted
    // file with an absurd declared size is rejected before any large buffer is
    // allocated, and the overall pixel budget is enforced up front.
    if let Some((w, h)) = ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .ok()
        .and_then(|r| r.into_dimensions().ok())
    {
        if w == 0 || h == 0 {
            return Err(AppError::Image("image has invalid dimensions".into()));
        }
        if (w as u64) * (h as u64) > MAX_SOURCE_PIXELS {
            return Err(AppError::Image(
                "image exceeds the maximum pixel dimension".into(),
            ));
        }
    }

    // Read the EXIF orientation from a separate probe reader, then decode and
    // correct the image so portrait phone photos are stored upright. image
    // 0.25 does not auto-apply orientation on decode and has no `exif`
    // feature, so this is applied manually.
    let orientation = ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .ok()
        .and_then(|reader| reader.into_decoder().ok())
        .and_then(|mut decoder| decoder.orientation().ok())
        .unwrap_or(Orientation::NoTransforms);
    let mut img = reader
        .decode()
        .map_err(|e| AppError::Image(format!("decode failed: {e}")))?;
    img.apply_orientation(orientation);

    let (mut width, mut height) = (img.width(), img.height());
    if width == 0 || height == 0 {
        return Err(AppError::Image("image has invalid dimensions".into()));
    }
    if (width as u64) * (height as u64) > MAX_SOURCE_PIXELS {
        return Err(AppError::Image(
            "image exceeds the maximum pixel dimension".into(),
        ));
    }

    if width.max(height) > MAX_IMAGE_DIMENSION {
        let scale = MAX_IMAGE_DIMENSION as f64 / width.max(height) as f64;
        width = ((width as f64 * scale).round() as u32).max(1);
        height = ((height as f64 * scale).round() as u32).max(1);
        img = img.resize(width, height, image::imageops::FilterType::Lanczos3);
    }

    let stem = uuid::Uuid::now_v7().to_string();
    let stored_name = format!("{stem}.webp");
    let thumb_name = format!("{stem}-thumb.webp");

    // Re-encode the validated original as (lossy) WebP; re-encoding strips any
    // remaining metadata so the stored file never carries EXIF/normal EXIF.
    let original_path = images_dir.join(&stored_name);
    img.save_with_format(&original_path, ImageFormat::WebP)
        .map_err(|e| AppError::Image(format!("re-encode failed: {e}")))?;

    let stored_bytes = fs::read(&original_path)?;
    let sha256 = to_hex(&Sha256::digest(&stored_bytes));

    // Thumbnail keeps the rendering light for catalogue grids.
    let thumb = img.thumbnail(THUMBNAIL_SIZE, THUMBNAIL_SIZE);
    let thumb_path = images_dir.join(&thumb_name);
    encode_webp(&thumb, &thumb_path)?;

    let thumbnail_bytes = fs::metadata(&thumb_path)?.len();

    Ok(ImportedImage {
        original_path: source.to_string_lossy().into_owned(),
        stored_name,
        thumbnail_name: thumb_name,
        width,
        height,
        mime_type: "image/webp".into(),
        sha256,
        thumbnail_bytes,
    })
}

/// Best-effort removal of generated files (originals and thumbnails). Used to
/// keep the images directory free of orphans when a product write fails after
/// files were already staged. Only plain generated file names are accepted.
pub fn discard_generated_images(images_dir: &Path, names: &[&str]) -> Result<(), AppError> {
    for name in names {
        if name.is_empty() || name.contains(['/', '\\']) {
            continue;
        }
        let path = images_dir.join(name);
        match fs::remove_file(&path) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(AppError::Io(e)),
        }
    }
    Ok(())
}

fn encode_webp(img: &image::DynamicImage, path: &Path) -> Result<(), AppError> {
    img.save_with_format(path, ImageFormat::WebP)
        .map_err(|e| AppError::Image(format!("thumbnail encode failed: {e}")))
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
