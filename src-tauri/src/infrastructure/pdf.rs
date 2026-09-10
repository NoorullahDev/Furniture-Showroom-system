use std::fs;
use std::io::{BufWriter, Write};
use std::path::Path;

use printpdf::{BuiltinFont, Mm, PdfDocument};

use crate::error::AppError;

const PAGE_W: f32 = 210.0;
const PAGE_H: f32 = 297.0;
const MARGIN_LEFT: f32 = 15.0;
const MARGIN_RIGHT: f32 = 15.0;
const CONTENT_W: f32 = PAGE_W - MARGIN_LEFT - MARGIN_RIGHT;
const HEADER_TOP_Y: f32 = 280.0;
const ROW_HEIGHT: f32 = 7.0;
const PAGE_BREAK_Y: f32 = 35.0;

pub struct ProofPdf {
    pub path: String,
    pub pages: usize,
    pub bytes: u64,
}

pub struct InvoicePdf {
    pub path: String,
    pub pages: usize,
    pub bytes: u64,
}

pub struct InvoiceLine {
    pub article: String,
    pub name: String,
    pub quantity: i64,
    pub price_minor: i64,
    pub total_minor: i64,
}

pub struct InvoiceRecord {
    pub number: String,
    pub sale_date: String,
    pub customer_name: Option<String>,
    pub shop_name: String,
    pub shop_address: Option<String>,
    pub items: Vec<InvoiceLine>,
    pub subtotal_minor: i64,
    pub discount_minor: i64,
    pub delivery_charge_minor: i64,
    pub tax_minor: i64,
    pub total_minor: i64,
    pub paid_minor: i64,
    pub advance_used_minor: i64,
    pub due_minor: i64,
}

pub struct ReceiptPdf {
    pub path: String,
    pub pages: usize,
    pub bytes: u64,
}

pub struct ReceiptAllocationLine {
    pub sale_id: i64,
    pub sale_number: Option<String>,
    pub amount_minor: i64,
}

pub struct ReceiptRecord {
    pub receipt_number: String,
    pub payment_date: String,
    pub customer_name: String,
    pub method: String,
    pub cash_account: String,
    pub amount_minor: i64,
    pub advance_minor: i64,
    pub allocations: Vec<ReceiptAllocationLine>,
    pub shop_name: String,
    pub shop_address: Option<String>,
}

pub struct DeliveryNoteLine {
    pub article: String,
    pub name: String,
    pub quantity: i64,
    pub unit_price_minor: i64,
    pub line_total_minor: i64,
}

pub struct DeliveryNoteRecord {
    pub number: String,
    pub delivery_date: String,
    pub customer_name: Option<String>,
    pub address: Option<String>,
    pub contact_phone: Option<String>,
    pub driver_note: Option<String>,
    pub vehicle_note: Option<String>,
    pub items: Vec<DeliveryNoteLine>,
    pub total_units: i64,
    pub shop_name: String,
    pub shop_address: Option<String>,
}

pub struct DeliveryNotePdf {
    pub path: String,
    pub pages: usize,
    pub bytes: u64,
}

pub struct CreditNoteRecord {
    pub number: String,
    pub note_date: String,
    pub customer_name: Option<String>,
    pub return_number: Option<String>,
    pub amount_minor: i64,
    pub reason: String,
    pub shop_name: String,
    pub shop_address: Option<String>,
}

pub struct CreditNotePdf {
    pub path: String,
    pub pages: usize,
    pub bytes: u64,
}

fn format_pkr(minor: i64) -> String {
    let major = minor.div_euclid(100);
    let paisa = (minor % 100).abs();
    let digits = major.abs().to_string();
    let mut grouped = String::new();
    let len = digits.len();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let sign = if major < 0 { "-" } else { "" };
    format!("PKR {sign}{grouped}.{paisa:02}")
}

pub fn generate_invoice_pdf(
    reports_dir: &Path,
    record: &InvoiceRecord,
) -> Result<InvoicePdf, AppError> {
    let (doc, page1, layer1) = PdfDocument::new(&record.shop_name, Mm(210.0), Mm(297.0), "Invoice");

    let helvetica = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    let helvetica_bold = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| AppError::Pdf(e.to_string()))?;

    let layer = doc.get_page(page1).get_layer(layer1);

    layer.use_text(
        &record.shop_name,
        18.0,
        Mm(20.0),
        Mm(272.0),
        &helvetica_bold,
    );
    if let Some(address) = &record.shop_address {
        layer.use_text(address, 9.0, Mm(20.0), Mm(264.0), &helvetica);
    }
    layer.use_text("TAX INVOICE", 14.0, Mm(150.0), Mm(272.0), &helvetica_bold);

    let header = [
        ("Invoice No.", record.number.as_str()),
        ("Date", record.sale_date.as_str()),
        (
            "Customer",
            record
                .customer_name
                .as_deref()
                .unwrap_or("Walk-in Customer"),
        ),
    ];
    let mut y: f32 = 248.0;
    for (label, value) in header {
        layer.use_text(label, 10.0, Mm(20.0), Mm(y), &helvetica);
        layer.use_text(value, 10.0, Mm(80.0), Mm(y), &helvetica_bold);
        y -= 11.0;
    }

    // Column heads.
    let mut y: f32 = 206.0;
    layer.use_text("Item", 10.0, Mm(20.0), Mm(y), &helvetica_bold);
    layer.use_text("Qty", 10.0, Mm(130.0), Mm(y), &helvetica_bold);
    layer.use_text("Price", 10.0, Mm(150.0), Mm(y), &helvetica_bold);
    layer.use_text("Amount", 10.0, Mm(178.0), Mm(y), &helvetica_bold);
    y -= 10.0;

    for line in &record.items {
        layer.use_text(&line.article, 9.0, Mm(20.0), Mm(y), &helvetica);
        layer.use_text(&line.name, 9.0, Mm(36.0), Mm(y), &helvetica);
        layer.use_text(
            line.quantity.to_string(),
            10.0,
            Mm(130.0),
            Mm(y),
            &helvetica,
        );
        layer.use_text(
            format_pkr(line.price_minor),
            10.0,
            Mm(150.0),
            Mm(y),
            &helvetica,
        );
        layer.use_text(
            format_pkr(line.total_minor),
            10.0,
            Mm(178.0),
            Mm(y),
            &helvetica,
        );
        y -= 10.0;
    }

    let summary = [
        ("Subtotal", format_pkr(record.subtotal_minor)),
        ("Discount", format_pkr(-record.discount_minor)),
        ("Delivery", format_pkr(record.delivery_charge_minor)),
        ("Tax", format_pkr(record.tax_minor)),
        ("TOTAL", format_pkr(record.total_minor)),
        ("Paid", format_pkr(record.paid_minor)),
        ("Advance Used", format_pkr(record.advance_used_minor)),
        ("DUE", format_pkr(record.due_minor)),
    ];
    for (label, value) in summary {
        y -= 2.0;
        let bold = label == "TOTAL" || label == "DUE";
        let font = if bold { &helvetica_bold } else { &helvetica };
        layer.use_text(
            label,
            if bold { 12.0 } else { 10.0 },
            Mm(150.0),
            Mm(y),
            font,
        );
        layer.use_text(
            &value,
            if bold { 12.0 } else { 10.0 },
            Mm(178.0),
            Mm(y),
            font,
        );
    }

    layer.use_text(
        "Thank you for shopping with us!",
        9.0,
        Mm(20.0),
        Mm(30.0),
        &helvetica,
    );
    layer.use_text(
        format!("Powered by {}", record.shop_name),
        8.0,
        Mm(20.0),
        Mm(22.0),
        &helvetica,
    );

    let filename = format!("invoice-{}.pdf", uuid::Uuid::now_v7());
    let path = reports_dir.join(filename);
    let mut out = BufWriter::new(fs::File::create(&path)?);
    doc.save(&mut out)
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    out.flush()?;

    let bytes = fs::metadata(&path)?.len();

    Ok(InvoicePdf {
        pages: 1,
        bytes,
        path: path.to_string_lossy().into_owned(),
    })
}

/// Generates a payment receipt PDF, mirroring the invoice layout so customer
/// documents share the same visual identity.
pub fn generate_receipt_pdf(
    reports_dir: &Path,
    record: &ReceiptRecord,
) -> Result<ReceiptPdf, AppError> {
    let (doc, page1, layer1) = PdfDocument::new(&record.shop_name, Mm(210.0), Mm(297.0), "Receipt");

    let helvetica = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    let helvetica_bold = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| AppError::Pdf(e.to_string()))?;

    let layer = doc.get_page(page1).get_layer(layer1);

    layer.use_text(
        &record.shop_name,
        18.0,
        Mm(20.0),
        Mm(272.0),
        &helvetica_bold,
    );
    if let Some(address) = &record.shop_address {
        layer.use_text(address, 9.0, Mm(20.0), Mm(264.0), &helvetica);
    }
    layer.use_text(
        "PAYMENT RECEIPT",
        14.0,
        Mm(150.0),
        Mm(272.0),
        &helvetica_bold,
    );

    let header = [
        ("Receipt No.", record.receipt_number.as_str()),
        ("Date", record.payment_date.as_str()),
        ("Customer", record.customer_name.as_str()),
        ("Payment Method", record.method.as_str()),
        ("Cash Account", record.cash_account.as_str()),
    ];
    let mut y: f32 = 246.0;
    for (label, value) in header {
        layer.use_text(label, 10.0, Mm(20.0), Mm(y), &helvetica);
        layer.use_text(value, 10.0, Mm(80.0), Mm(y), &helvetica_bold);
        y -= 11.0;
    }

    y -= 5.0;
    layer.use_text("Invoice", 10.0, Mm(20.0), Mm(y), &helvetica_bold);
    layer.use_text("Amount", 10.0, Mm(178.0), Mm(y), &helvetica_bold);
    y -= 10.0;

    for line in &record.allocations {
        match &line.sale_number {
            Some(number) => {
                layer.use_text(number, 10.0, Mm(20.0), Mm(y), &helvetica);
            }
            None => layer.use_text(
                format!("sale #{}", line.sale_id),
                10.0,
                Mm(20.0),
                Mm(y),
                &helvetica,
            ),
        }
        layer.use_text(
            format_pkr(line.amount_minor),
            10.0,
            Mm(178.0),
            Mm(y),
            &helvetica,
        );
        y -= 10.0;
    }

    let allocated_minor = record.amount_minor - record.advance_minor;
    let summary = [
        ("Amount Received", format_pkr(record.amount_minor)),
        ("Applied to Invoices", format_pkr(allocated_minor)),
        ("Advance", format_pkr(record.advance_minor)),
        (
            "Balance Due",
            format_pkr(record.amount_minor - allocated_minor - record.advance_minor),
        ),
    ];
    for (label, value) in summary {
        y -= 2.0;
        layer.use_text(label, 10.0, Mm(150.0), Mm(y), &helvetica);
        layer.use_text(&value, 10.0, Mm(178.0), Mm(y), &helvetica);
    }

    layer.use_text(
        "Thank you for your payment!",
        9.0,
        Mm(20.0),
        Mm(30.0),
        &helvetica,
    );
    layer.use_text(
        format!("Powered by {}", record.shop_name),
        8.0,
        Mm(20.0),
        Mm(22.0),
        &helvetica,
    );

    let filename = format!("receipt-{}.pdf", uuid::Uuid::now_v7());
    let path = reports_dir.join(filename);
    let mut out = BufWriter::new(fs::File::create(&path)?);
    doc.save(&mut out)
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    out.flush()?;

    let bytes = fs::metadata(&path)?.len();

    Ok(ReceiptPdf {
        pages: 1,
        bytes,
        path: path.to_string_lossy().into_owned(),
    })
}

/// Generates a delivery note PDF, mirroring the invoice layout so delivery
/// copies share the same visual identity as the customer documents.
pub fn generate_delivery_note_pdf(
    reports_dir: &Path,
    record: &DeliveryNoteRecord,
) -> Result<DeliveryNotePdf, AppError> {
    let (doc, page1, layer1) =
        PdfDocument::new(&record.shop_name, Mm(210.0), Mm(297.0), "Delivery");

    let helvetica = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    let helvetica_bold = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| AppError::Pdf(e.to_string()))?;

    let layer = doc.get_page(page1).get_layer(layer1);

    layer.use_text(
        &record.shop_name,
        18.0,
        Mm(20.0),
        Mm(272.0),
        &helvetica_bold,
    );
    if let Some(address) = &record.shop_address {
        layer.use_text(address, 9.0, Mm(20.0), Mm(264.0), &helvetica);
    }
    layer.use_text("DELIVERY NOTE", 14.0, Mm(150.0), Mm(272.0), &helvetica_bold);

    let mut y: f32 = 248.0;
    let header = [
        ("Delivery No.", record.number.as_str()),
        ("Date", record.delivery_date.as_str()),
        (
            "Customer",
            record
                .customer_name
                .as_deref()
                .unwrap_or("Walk-in Customer"),
        ),
        ("Address", record.address.as_deref().unwrap_or("-")),
        ("Contact", record.contact_phone.as_deref().unwrap_or("-")),
    ];
    for (label, value) in header {
        layer.use_text(label, 10.0, Mm(20.0), Mm(y), &helvetica);
        layer.use_text(value, 10.0, Mm(75.0), Mm(y), &helvetica_bold);
        y -= 11.0;
    }

    let mut y: f32 = 186.0;
    layer.use_text("Item", 10.0, Mm(20.0), Mm(y), &helvetica_bold);
    layer.use_text("Qty", 10.0, Mm(130.0), Mm(y), &helvetica_bold);
    layer.use_text("Price", 10.0, Mm(150.0), Mm(y), &helvetica_bold);
    layer.use_text("Amount", 10.0, Mm(178.0), Mm(y), &helvetica_bold);
    y -= 10.0;

    for line in &record.items {
        layer.use_text(&line.article, 9.0, Mm(20.0), Mm(y), &helvetica);
        layer.use_text(&line.name, 9.0, Mm(36.0), Mm(y), &helvetica);
        layer.use_text(
            line.quantity.to_string(),
            10.0,
            Mm(130.0),
            Mm(y),
            &helvetica,
        );
        layer.use_text(
            format_pkr(line.unit_price_minor),
            10.0,
            Mm(150.0),
            Mm(y),
            &helvetica,
        );
        layer.use_text(
            format_pkr(line.line_total_minor),
            10.0,
            Mm(178.0),
            Mm(y),
            &helvetica,
        );
        y -= 10.0;
    }

    if let Some(note) = &record.driver_note {
        if !note.is_empty() {
            layer.use_text("Driver note", 10.0, Mm(20.0), Mm(y), &helvetica_bold);
            y -= 10.0;
            layer.use_text(note, 9.0, Mm(20.0), Mm(y), &helvetica);
            y -= 10.0;
        }
    }
    if let Some(note) = &record.vehicle_note {
        if !note.is_empty() {
            layer.use_text("Vehicle note", 10.0, Mm(20.0), Mm(y), &helvetica_bold);
            y -= 10.0;
            layer.use_text(note, 9.0, Mm(20.0), Mm(y), &helvetica);
        }
    }

    layer.use_text(
        format!("Total Units: {}", record.total_units),
        10.0,
        Mm(20.0),
        Mm(40.0),
        &helvetica,
    );
    layer.use_text(
        format!("Powered by {}", record.shop_name),
        8.0,
        Mm(20.0),
        Mm(22.0),
        &helvetica,
    );

    let filename = format!("delivery-note-{}.pdf", uuid::Uuid::now_v7());
    let path = reports_dir.join(filename);
    let mut out = BufWriter::new(fs::File::create(&path)?);
    doc.save(&mut out)
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    out.flush()?;

    let bytes = fs::metadata(&path)?.len();

    Ok(DeliveryNotePdf {
        pages: 1,
        bytes,
        path: path.to_string_lossy().into_owned(),
    })
}

/// Generates a credit note PDF for a customer's return credit balance.
pub fn generate_credit_note_pdf(
    reports_dir: &Path,
    record: &CreditNoteRecord,
) -> Result<CreditNotePdf, AppError> {
    let (doc, page1, layer1) =
        PdfDocument::new(&record.shop_name, Mm(210.0), Mm(297.0), "Credit Note");

    let helvetica = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    let helvetica_bold = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| AppError::Pdf(e.to_string()))?;

    let layer = doc.get_page(page1).get_layer(layer1);

    layer.use_text(
        &record.shop_name,
        18.0,
        Mm(20.0),
        Mm(272.0),
        &helvetica_bold,
    );
    if let Some(address) = &record.shop_address {
        layer.use_text(address, 9.0, Mm(20.0), Mm(264.0), &helvetica);
    }
    layer.use_text("CREDIT NOTE", 14.0, Mm(150.0), Mm(272.0), &helvetica_bold);

    let mut y: f32 = 248.0;
    let header = [
        ("Credit Note No.", record.number.as_str()),
        ("Date", record.note_date.as_str()),
        (
            "Customer",
            record
                .customer_name
                .as_deref()
                .unwrap_or("Walk-in Customer"),
        ),
        (
            "Against Return",
            record.return_number.as_deref().unwrap_or("-"),
        ),
    ];
    for (label, value) in header {
        layer.use_text(label, 10.0, Mm(20.0), Mm(y), &helvetica);
        layer.use_text(value, 10.0, Mm(80.0), Mm(y), &helvetica_bold);
        y -= 11.0;
    }

    if !record.reason.is_empty() {
        layer.use_text("Reason", 10.0, Mm(20.0), Mm(y), &helvetica);
        layer.use_text(&record.reason, 10.0, Mm(80.0), Mm(y), &helvetica);
        y -= 11.0;
    }

    layer.use_text(
        format!("This note balances {}.", format_pkr(record.amount_minor)),
        12.0,
        Mm(20.0),
        Mm(y - 8.0),
        &helvetica_bold,
    );

    layer.use_text(
        "It may be applied as an advance against a future purchase.",
        9.0,
        Mm(20.0),
        Mm(30.0),
        &helvetica,
    );
    layer.use_text(
        format!("Powered by {}", record.shop_name),
        8.0,
        Mm(20.0),
        Mm(22.0),
        &helvetica,
    );

    let filename = format!("credit-note-{}.pdf", uuid::Uuid::now_v7());
    let path = reports_dir.join(filename);
    let mut out = BufWriter::new(fs::File::create(&path)?);
    doc.save(&mut out)
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    out.flush()?;

    let bytes = fs::metadata(&path)?.len();

    Ok(CreditNotePdf {
        pages: 1,
        bytes,
        path: path.to_string_lossy().into_owned(),
    })
}

/// Generates a Phase 0 proof PDF: A4 page with English lines (PKR amounts),
/// a Unicode Urdu line via an embedded font, and a footer.
///
/// Known Phase 0 limitation: printpdf does not perform complex Arabic-script
/// shaping, so Urdu renders as individual glyph forms. Full shaping is a
/// later-phase concern (HarfBuzz-backed rendering) and is flagged in the
/// risk register as "PDF works in English but fails in Urdu".
pub fn generate_proof_pdf(reports_dir: &Path, fonts_dir: &Path) -> Result<ProofPdf, AppError> {
    let font_candidates = super::fonts::resolve_fonts(fonts_dir)?;

    let (doc, page1, layer1) = PdfDocument::new(
        "Furniture Shop — Technical Proof",
        Mm(210.0),
        Mm(297.0),
        "Page 1",
    );

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

    let urdu_font = urdu_font
        .ok_or_else(|| AppError::Pdf("no Unicode font could be embedded for Urdu text".into()))?;

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
    doc.save(&mut out)
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    out.flush()?;

    let bytes = fs::metadata(&path)?.len();

    Ok(ProofPdf {
        pages: 1,
        bytes,
        path: path.to_string_lossy().into_owned(),
    })
}

#[derive(Clone)]
pub struct ReportPdfColumn {
    pub header: String,
    pub width_ratio: f32,
    pub align_left: bool,
}

pub struct ReportPdfInput {
    pub title: String,
    pub shop_name: String,
    pub shop_address: Option<String>,
    pub filter_summary: String,
    pub generated_at: String,
    pub columns: Vec<ReportPdfColumn>,
    pub rows: Vec<Vec<String>>,
    pub totals: Option<Vec<String>>,
}

pub struct ReportPdf {
    pub path: String,
    pub pages: usize,
    pub bytes: u64,
}

fn draw_header(
    layer: &printpdf::PdfLayerReference,
    input: &ReportPdfInput,
    helvetica: &printpdf::IndirectFontRef,
    helvetica_bold: &printpdf::IndirectFontRef,
) {
    layer.use_text(
        &input.shop_name,
        16.0,
        Mm(MARGIN_LEFT),
        Mm(HEADER_TOP_Y),
        helvetica_bold,
    );
    if let Some(addr) = &input.shop_address {
        layer.use_text(
            addr,
            8.0,
            Mm(MARGIN_LEFT),
            Mm(HEADER_TOP_Y - 8.0),
            helvetica,
        );
    }
    layer.use_text(
        &input.title,
        13.0,
        Mm(MARGIN_LEFT),
        Mm(HEADER_TOP_Y - 18.0),
        helvetica_bold,
    );
    layer.use_text(
        &input.filter_summary,
        8.0,
        Mm(MARGIN_LEFT),
        Mm(HEADER_TOP_Y - 26.0),
        helvetica,
    );
    layer.use_text(
        format!("Generated: {}", input.generated_at),
        7.0,
        Mm(130.0),
        Mm(HEADER_TOP_Y - 26.0),
        helvetica,
    );
}

fn draw_column_headers(
    layer: &printpdf::PdfLayerReference,
    input: &ReportPdfInput,
    y: f32,
    col_x: &[f32],
    helvetica_bold: &printpdf::IndirectFontRef,
) {
    for (i, col) in input.columns.iter().enumerate() {
        layer.use_text(&col.header, 8.0, Mm(col_x[i]), Mm(y), helvetica_bold);
    }
}

pub fn generate_report_pdf(
    input: &ReportPdfInput,
    _fonts_dir: &Path,
    reports_dir: &Path,
) -> Result<ReportPdf, AppError> {
    let (doc, page1, layer1) = PdfDocument::new(&input.shop_name, Mm(PAGE_W), Mm(PAGE_H), "Report");

    let helvetica = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    let helvetica_bold = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| AppError::Pdf(e.to_string()))?;

    let total_ratio: f32 = input.columns.iter().map(|c| c.width_ratio).sum();
    let mut col_x: Vec<f32> = Vec::with_capacity(input.columns.len());
    let mut x = MARGIN_LEFT;
    for col in &input.columns {
        col_x.push(x);
        x += CONTENT_W * (col.width_ratio / total_ratio);
    }

    let mut page_count: usize = 1;
    let mut layer = doc.get_page(page1).get_layer(layer1);

    draw_header(&layer, input, &helvetica, &helvetica_bold);

    let mut y = HEADER_TOP_Y - 36.0;
    draw_column_headers(&layer, input, y, &col_x, &helvetica_bold);
    y -= ROW_HEIGHT;

    for row in &input.rows {
        if y < PAGE_BREAK_Y {
            page_count += 1;
            let (page_ref, layer_ref) =
                doc.add_page(Mm(PAGE_W), Mm(PAGE_H), format!("Report p{page_count}"));
            layer = doc.get_page(page_ref).get_layer(layer_ref);
            draw_header(&layer, input, &helvetica, &helvetica_bold);
            y = HEADER_TOP_Y - 36.0;
            draw_column_headers(&layer, input, y, &col_x, &helvetica_bold);
            y -= ROW_HEIGHT;
        }
        for (i, cell) in row.iter().enumerate() {
            layer.use_text(cell, 8.0, Mm(col_x[i]), Mm(y), &helvetica);
        }
        y -= ROW_HEIGHT;
    }

    if let Some(totals) = &input.totals {
        if y < PAGE_BREAK_Y + ROW_HEIGHT {
            page_count += 1;
            let (page_ref, layer_ref) =
                doc.add_page(Mm(PAGE_W), Mm(PAGE_H), format!("Report p{page_count}"));
            layer = doc.get_page(page_ref).get_layer(layer_ref);
            draw_header(&layer, input, &helvetica, &helvetica_bold);
            y = HEADER_TOP_Y - 36.0;
            draw_column_headers(&layer, input, y, &col_x, &helvetica_bold);
            y -= ROW_HEIGHT;
        }
        y -= 2.0;
        for (i, cell) in totals.iter().enumerate() {
            layer.use_text(cell, 9.0, Mm(col_x[i]), Mm(y), &helvetica_bold);
        }
    }

    let filename = format!("report-{}.pdf", uuid::Uuid::now_v7());
    let path = reports_dir.join(filename);
    let mut out = BufWriter::new(fs::File::create(&path)?);
    doc.save(&mut out)
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    out.flush()?;

    let bytes = fs::metadata(&path)?.len();

    Ok(ReportPdf {
        pages: page_count,
        bytes,
        path: path.to_string_lossy().into_owned(),
    })
}
