use std::fs;
use std::io::{BufWriter, Write};
use std::path::Path;

use printpdf::{BuiltinFont, Mm, PdfDocument};

use crate::error::AppError;

pub struct ProofPdf {
    pub path: String,
    pub pages: usize,
    pub bytes: u64,
}

/// Generates a Phase 0 proof PDF: A4 page with English lines (PKR amounts),
/// a Unicode Urdu line via an embedded font, and a footer.
///
/// Known Phase 0 limitation: printpdf does not perform complex Arabic-script
/// shaping, so Urdu renders as individual glyph forms. Full shaping is a
/// later-phase concern (HarfBuzz-backed rendering) and is flagged in the
/// risk register as "PDF works in English but fails in Urdu".
pub fn generate_proof_pdf(
    reports_dir: &Path,
    fonts_dir: &Path,
) -> Result<ProofPdf, AppError> {
    let font_candidates = super::fonts::resolve_fonts(fonts_dir)?;

    let (doc, page1, layer1) =
        PdfDocument::new("Furniture Shop — Technical Proof", Mm(210.0), Mm(297.0), "Page 1");

    let helvetica = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    let helvetica_bold = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| AppError::Pdf(e.to_string()))?;

    // Choose the first font candidate the PDF backend can parse.
    let mut urdu_font = None;
    for candidate in &font_candidates {
        match fs::File::open(candidate) {
            Ok(file) => match doc.add_external_font(file) {
                Ok(idx) => {
                    urdu_font = Some(idx);
                    break;
                }
                Err(_) => continue,
            },
            Err(_) => continue,
        }
    }

    let urdu_font = urdu_font.ok_or_else(|| {
        AppError::Pdf("no Unicode font could be embedded for Urdu text".into())
    })?;

    let layer = doc.get_page(page1).get_layer(layer1);

    layer.use_text(
        "EagleNest Creations",
        18.0,
        Mm(20.0),
        Mm(272.0),
        &helvetica_bold,
    );
    layer.use_text(
        "Furniture Shop - Phase 0 Technical Proof",
        12.0,
        Mm(20.0),
        Mm(262.0),
        &helvetica,
    );

    let header = [
        ("Invoice No.", "INV-0001"),
        ("Date", "2026-09-08"),
        ("Customer", "Demo Customer"),
    ];
    let mut y: f32 = 246.0;
    for (label, value) in header {
        layer.use_text(label, 10.0, Mm(20.0), Mm(y), &helvetica);
        layer.use_text(value, 10.0, Mm(70.0), Mm(y), &helvetica);
        y -= 10.0;
    }

    let rows = [
        ("ART-0001", "Modern Sofa", "1", "PKR 125,000"),
        ("ART-0002", "Dining Table", "1", "PKR 75,500"),
        ("ART-0003", "Wardrobe 5ft", "2", "PKR 98,750"),
    ];
    let mut y: f32 = 206.0;
    for (article, name, qty, amount) in rows {
        layer.use_text(article, 10.0, Mm(20.0), Mm(y), &helvetica);
        layer.use_text(name, 10.0, Mm(70.0), Mm(y), &helvetica);
        layer.use_text(qty, 10.0, Mm(150.0), Mm(y), &helvetica);
        layer.use_text(amount, 10.0, Mm(175.0), Mm(y), &helvetica);
        y -= 10.0;
    }

    layer.use_text(
        "Total: PKR 398,000",
        12.0,
        Mm(145.0),
        Mm(150.0),
        &helvetica_bold,
    );

    // Urdu sample line using the embedded Noto Nastaliq Urdu font.
    let urdu = "یہ ایک ٹیسٹ انوائس ہے — فرنیچر شاپ";
    layer.use_text(urdu, 16.0, Mm(20.0), Mm(120.0), &urdu_font);

    layer.use_text(
        "Bank: Meezan Bank | Account: 0000-123456",
        9.0,
        Mm(20.0),
        Mm(40.0),
        &helvetica,
    );
    layer.use_text(
        "Powered by EagleNest Creations",
        8.0,
        Mm(20.0),
        Mm(30.0),
        &helvetica,
    );

    let filename = format!("proof-{}.pdf", uuid::Uuid::now_v7());
    let path = reports_dir.join(filename);
    let mut out = BufWriter::new(fs::File::create(&path)?);
    doc.save(&mut out).map_err(|e| AppError::Pdf(e.to_string()))?;
    out.flush()?;

    let bytes = fs::metadata(&path)?.len();

    Ok(ProofPdf {
        pages: 1,
        bytes,
        path: path.to_string_lossy().into_owned(),
    })
}