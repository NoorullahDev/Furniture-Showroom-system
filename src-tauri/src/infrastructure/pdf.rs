use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use printpdf::{
    path::{PaintMode, WindingOrder},
    BuiltinFont, Color, ColorBits, ColorSpace, Image, ImageTransform, ImageXObject, Mm,
    PdfDocument, Point, Polygon, Px, Rgb,
};

use crate::error::AppError;

const PAGE_W: f32 = 210.0;
const PAGE_H: f32 = 297.0;
const MARGIN_LEFT: f32 = 15.0;
const MARGIN_RIGHT: f32 = 15.0;
const HEADER_TOP_Y: f32 = 280.0;
const ROW_HEIGHT: f32 = 7.0;
const PAGE_BREAK_Y: f32 = 35.0;
const DEVELOPER_FOOTER_PREFIX: &str = "Software developed by ";
const DEVELOPER_FOOTER_BRAND: &str = "EagleNest Creations";
const DEVELOPER_FOOTER_SUFFIX: &str = " (0346-4451505)";

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
    pub customer_phone: Option<String>,
    pub customer_address: Option<String>,
    pub notes: Option<String>,
    pub footer_text: Option<String>,
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
    pub customer_phone: Option<String>,
    pub customer_address: Option<String>,
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
    let (doc, page1, layer1) =
        PdfDocument::new(&record.shop_name, Mm(PAGE_W), Mm(PAGE_H), "Invoice");

    let helvetica = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    let helvetica_bold = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| AppError::Pdf(e.to_string()))?;

    let mut pages = vec![page1];
    let mut current_page = page1;
    let mut layer = doc.get_page(current_page).get_layer(layer1);

    // --- Page 1: header block ------------------------------------------------
    layer.use_text(
        &record.shop_name,
        18.0,
        Mm(MARGIN_LEFT),
        Mm(272.0),
        &helvetica_bold,
    );
    if let Some(addr) = &record.shop_address {
        layer.use_text(addr, 9.0, Mm(MARGIN_LEFT), Mm(264.0), &helvetica);
    }
    layer.use_text("TAX INVOICE", 14.0, Mm(150.0), Mm(272.0), &helvetica_bold);

    // Bill-to / invoice meta
    let mut y: f32 = 248.0;
    let draw_meta = |layer: &printpdf::PdfLayerReference, y: &mut f32| {
        let items = [
            ("Invoice No.", record.number.as_str()),
            ("Date", record.sale_date.as_str()),
        ];
        for (label, value) in items {
            layer.use_text(label, 10.0, Mm(MARGIN_LEFT), Mm(*y), &helvetica);
            layer.use_text(value, 10.0, Mm(80.0), Mm(*y), &helvetica_bold);
            *y -= 11.0;
        }
        if let Some(name) = &record.customer_name {
            layer.use_text("Customer", 10.0, Mm(MARGIN_LEFT), Mm(*y), &helvetica);
            layer.use_text(name, 10.0, Mm(80.0), Mm(*y), &helvetica_bold);
            *y -= 11.0;
        }
        if let Some(phone) = &record.customer_phone {
            layer.use_text("Phone", 10.0, Mm(MARGIN_LEFT), Mm(*y), &helvetica);
            layer.use_text(phone, 10.0, Mm(80.0), Mm(*y), &helvetica_bold);
            *y -= 11.0;
        }
        if let Some(addr) = &record.customer_address {
            layer.use_text("Address", 10.0, Mm(MARGIN_LEFT), Mm(*y), &helvetica);
            layer.use_text(addr, 10.0, Mm(80.0), Mm(*y), &helvetica_bold);
            *y -= 11.0;
        }
    };
    draw_meta(&layer, &mut y);
    y -= 4.0;

    // Column headers helper
    let draw_col_headers = |layer: &printpdf::PdfLayerReference, y: f32| {
        layer.use_text("Item", 10.0, Mm(MARGIN_LEFT), Mm(y), &helvetica_bold);
        layer.use_text("Qty", 10.0, Mm(130.0), Mm(y), &helvetica_bold);
        layer.use_text("Price", 10.0, Mm(150.0), Mm(y), &helvetica_bold);
        layer.use_text("Amount", 10.0, Mm(178.0), Mm(y), &helvetica_bold);
    };
    draw_col_headers(&layer, y);
    y -= 10.0;

    // --- Items ----------------------------------------------------------------
    for line in &record.items {
        if y < PAGE_BREAK_Y {
            // new page
            let (next_page, next_layer1) = doc.add_page(Mm(PAGE_W), Mm(PAGE_H), "Invoice cont.");
            pages.push(next_page);
            current_page = next_page;
            layer = doc.get_page(current_page).get_layer(next_layer1);
            y = HEADER_TOP_Y;
            draw_col_headers(&layer, y);
            y -= 10.0;
        }
        layer.use_text(&line.article, 9.0, Mm(MARGIN_LEFT), Mm(y), &helvetica);
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

    // --- Summary --------------------------------------------------------------
    let summary_min_y = 92.0
        + if record.notes.is_some() { 14.0 } else { 0.0 }
        + if record.footer_text.is_some() {
            16.0
        } else {
            0.0
        };
    if y < summary_min_y {
        let (next_page, next_layer1) = doc.add_page(Mm(PAGE_W), Mm(PAGE_H), "Invoice summary");
        pages.push(next_page);
        current_page = next_page;
        layer = doc.get_page(current_page).get_layer(next_layer1);
        y = HEADER_TOP_Y;
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

    // --- Notes & footer text --------------------------------------------------
    if let Some(notes) = &record.notes {
        y -= 14.0;
        layer.use_text(
            format!("Notes: {notes}"),
            9.0,
            Mm(MARGIN_LEFT),
            Mm(y),
            &helvetica,
        );
    }
    if let Some(ft) = &record.footer_text {
        y -= 16.0;
        layer.use_text(ft, 9.0, Mm(PAGE_W / 2.0), Mm(y), &helvetica);
    }

    // --- Signature lines ------------------------------------------------------
    y -= 50.0;
    // Authorized signature
    let sig_w: f32 = 60.0;
    layer.add_line(printpdf::Line {
        points: vec![
            (printpdf::Point::new(Mm(MARGIN_LEFT), Mm(y)), false),
            (printpdf::Point::new(Mm(MARGIN_LEFT + sig_w), Mm(y)), false),
        ],
        is_closed: false,
    });
    layer.use_text(
        "Authorized Signature",
        8.0,
        Mm(MARGIN_LEFT + 10.0),
        Mm(y - 4.0),
        &helvetica,
    );
    // Customer signature
    layer.add_line(printpdf::Line {
        points: vec![
            (printpdf::Point::new(Mm(120.0), Mm(y)), false),
            (printpdf::Point::new(Mm(180.0), Mm(y)), false),
        ],
        is_closed: false,
    });
    layer.use_text(
        "Customer Signature",
        8.0,
        Mm(130.0),
        Mm(y - 4.0),
        &helvetica,
    );

    // --- Developer footer on every page --------------------------------------
    let page_count = pages.len();
    for page_ref in pages {
        let footer = doc.get_page(page_ref).add_layer("Developer footer");
        draw_developer_footer(&footer, PAGE_W, 8.0, 8.0, &helvetica, &helvetica_bold);
    }

    // --- Save -----------------------------------------------------------------
    let filename = format!("invoice-{}.pdf", uuid::Uuid::now_v7());
    let path = reports_dir.join(filename);
    let mut out = BufWriter::new(fs::File::create(&path)?);
    doc.save(&mut out)
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    out.flush()?;

    let bytes = fs::metadata(&path)?.len();

    Ok(InvoicePdf {
        pages: page_count,
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
    let (doc, page1, layer1) =
        PdfDocument::new(&record.shop_name, Mm(PAGE_W), Mm(PAGE_H), "Receipt");

    let helvetica = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    let helvetica_bold = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| AppError::Pdf(e.to_string()))?;

    let mut page_count: usize = 1;
    let mut current_page = page1;
    let mut layer = doc.get_page(current_page).get_layer(layer1);

    layer.use_text(
        &record.shop_name,
        18.0,
        Mm(MARGIN_LEFT),
        Mm(272.0),
        &helvetica_bold,
    );
    if let Some(addr) = &record.shop_address {
        layer.use_text(addr, 9.0, Mm(MARGIN_LEFT), Mm(264.0), &helvetica);
    }
    layer.use_text(
        "PAYMENT RECEIPT",
        14.0,
        Mm(150.0),
        Mm(272.0),
        &helvetica_bold,
    );

    let mut y: f32 = 246.0;
    let header = [
        ("Receipt No.", record.receipt_number.as_str()),
        ("Date", record.payment_date.as_str()),
        ("Customer", record.customer_name.as_str()),
        ("Payment Method", record.method.as_str()),
        ("Cash Account", record.cash_account.as_str()),
    ];
    for (label, value) in header {
        layer.use_text(label, 10.0, Mm(MARGIN_LEFT), Mm(y), &helvetica);
        layer.use_text(value, 10.0, Mm(80.0), Mm(y), &helvetica_bold);
        y -= 11.0;
    }

    // Column headers
    let draw_col_headers = |layer: &printpdf::PdfLayerReference, y: f32| {
        layer.use_text("Invoice", 10.0, Mm(MARGIN_LEFT), Mm(y), &helvetica_bold);
        layer.use_text("Amount", 10.0, Mm(178.0), Mm(y), &helvetica_bold);
    };
    y -= 5.0;
    draw_col_headers(&layer, y);
    y -= 10.0;

    for line in &record.allocations {
        if y < PAGE_BREAK_Y {
            let (next_page, next_layer1) = doc.add_page(Mm(PAGE_W), Mm(PAGE_H), "Receipt cont.");
            page_count += 1;
            current_page = next_page;
            layer = doc.get_page(current_page).get_layer(next_layer1);
            y = HEADER_TOP_Y;
            draw_col_headers(&layer, y);
            y -= 10.0;
        }
        match &line.sale_number {
            Some(number) => layer.use_text(number, 10.0, Mm(MARGIN_LEFT), Mm(y), &helvetica),
            None => layer.use_text(
                format!("sale #{}", line.sale_id),
                10.0,
                Mm(MARGIN_LEFT),
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

    // Summary
    if y < PAGE_BREAK_Y + 40.0 {
        let (next_page, next_layer1) = doc.add_page(Mm(PAGE_W), Mm(PAGE_H), "Receipt summary");
        page_count += 1;
        current_page = next_page;
        layer = doc.get_page(current_page).get_layer(next_layer1);
        y = HEADER_TOP_Y;
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

    y -= 16.0;
    layer.use_text(
        "Thank you for your payment!",
        9.0,
        Mm(MARGIN_LEFT),
        Mm(y),
        &helvetica,
    );
    y -= 8.0;
    layer.use_text(
        format!("Powered by {}", record.shop_name),
        8.0,
        Mm(MARGIN_LEFT),
        Mm(y),
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
        pages: page_count,
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
        "Furniture Shop â€” Technical Proof",
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
    let urdu = "ÛŒÛ Ø§ÛŒÚ© Ù¹ÛŒØ³Ù¹ Ø§Ù†ÙˆØ§Ø¦Ø³ ÛÛ’ â€” ÙØ±Ù†ÛŒÚ†Ø± Ø´Ø§Ù¾";
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
    pub shop_phone: Option<String>,
    pub logo_path: Option<PathBuf>,
    pub filter_summary: String,
    pub generated_at: String,
    pub generated_by: Option<String>,
    pub landscape: bool,
    pub columns: Vec<ReportPdfColumn>,
    pub rows: Vec<Vec<String>>,
    pub totals: Option<Vec<String>>,
    pub summary: Vec<(String, String)>,
}

pub struct ReportPdf {
    pub path: String,
    pub pages: usize,
    pub bytes: u64,
}

fn report_rect(
    layer: &printpdf::PdfLayerReference,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: Rgb,
) {
    layer.set_fill_color(Color::Rgb(color));
    layer.add_polygon(Polygon {
        rings: vec![vec![
            (Point::new(Mm(x), Mm(y)), false),
            (Point::new(Mm(x + width), Mm(y)), false),
            (Point::new(Mm(x + width), Mm(y + height)), false),
            (Point::new(Mm(x), Mm(y + height)), false),
        ]],
        mode: PaintMode::Fill,
        winding_order: WindingOrder::NonZero,
    });
}

fn report_line(layer: &printpdf::PdfLayerReference, x1: f32, x2: f32, y: f32, color: Rgb) {
    layer.set_outline_color(Color::Rgb(color));
    layer.set_outline_thickness(0.5);
    layer.add_line(printpdf::Line {
        points: vec![
            (Point::new(Mm(x1), Mm(y)), false),
            (Point::new(Mm(x2), Mm(y)), false),
        ],
        is_closed: false,
    });
}

fn estimated_text_width(text: &str, font_size: f32) -> f32 {
    text.chars()
        .map(|c| if c.is_ascii_punctuation() { 0.42 } else { 0.52 })
        .sum::<f32>()
        * font_size
        * 0.352_778
}

fn draw_developer_footer(
    layer: &printpdf::PdfLayerReference,
    page_width: f32,
    y: f32,
    font_size: f32,
    regular: &printpdf::IndirectFontRef,
    bold: &printpdf::IndirectFontRef,
) {
    let prefix_width = estimated_text_width(DEVELOPER_FOOTER_PREFIX, font_size);
    let brand_width = estimated_text_width(DEVELOPER_FOOTER_BRAND, font_size);
    let total_width =
        prefix_width + brand_width + estimated_text_width(DEVELOPER_FOOTER_SUFFIX, font_size);
    let x = (page_width - total_width) / 2.0;
    layer.set_fill_color(Color::Rgb(Rgb::new(0.38, 0.42, 0.39, None)));
    layer.use_text(DEVELOPER_FOOTER_PREFIX, font_size, Mm(x), Mm(y), regular);
    layer.use_text(
        DEVELOPER_FOOTER_BRAND,
        font_size,
        Mm(x + prefix_width),
        Mm(y),
        bold,
    );
    layer.use_text(
        DEVELOPER_FOOTER_SUFFIX,
        font_size,
        Mm(x + prefix_width + brand_width),
        Mm(y),
        regular,
    );
}

fn fit_report_text(text: &str, max_width: f32, font_size: f32) -> String {
    if estimated_text_width(text, font_size) <= max_width {
        return text.to_string();
    }
    let suffix = "...";
    let mut result = String::new();
    for ch in text.chars() {
        let candidate = format!("{result}{ch}{suffix}");
        if estimated_text_width(&candidate, font_size) > max_width {
            break;
        }
        result.push(ch);
    }
    result.push_str(suffix);
    result
}

fn load_report_logo(path: Option<&Path>) -> Option<ImageXObject> {
    let image = image::open(path?).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
    for pixel in image.pixels() {
        let alpha = pixel[3] as u16;
        for channel in &pixel.0[..3] {
            rgb.push(((*channel as u16 * alpha + 255 * (255 - alpha)) / 255) as u8);
        }
    }
    Some(ImageXObject {
        width: Px(width as usize),
        height: Px(height as usize),
        color_space: ColorSpace::Rgb,
        bits_per_component: ColorBits::Bit8,
        interpolate: true,
        image_data: rgb,
        image_filter: None,
        smask: None,
        clipping_bbox: None,
    })
}

fn draw_report_logo(
    layer: &printpdf::PdfLayerReference,
    logo: Option<&ImageXObject>,
    page_h: f32,
) -> bool {
    let Some(logo) = logo else { return false };
    let natural_w = logo.width.0 as f32 * 25.4 / 300.0;
    let natural_h = logo.height.0 as f32 * 25.4 / 300.0;
    let scale = 16.0 / natural_w.max(natural_h).max(0.1);
    Image::from(logo.clone()).add_to_layer(
        layer.clone(),
        ImageTransform {
            translate_x: Some(Mm(MARGIN_LEFT)),
            translate_y: Some(Mm(page_h - 32.0)),
            scale_x: Some(scale),
            scale_y: Some(scale),
            dpi: Some(300.0),
            ..Default::default()
        },
    );
    true
}

#[allow(clippy::too_many_arguments)]
fn draw_report_header(
    layer: &printpdf::PdfLayerReference,
    input: &ReportPdfInput,
    page_w: f32,
    page_h: f32,
    logo: Option<&ImageXObject>,
    helvetica: &printpdf::IndirectFontRef,
    col_x: &[f32],
    col_end: &[f32],
    helvetica_bold: &printpdf::IndirectFontRef,
) -> f32 {
    let green = Rgb::new(0.11, 0.29, 0.18, None);
    let muted = Rgb::new(0.35, 0.39, 0.37, None);
    let has_logo = draw_report_logo(layer, logo, page_h);
    if !has_logo {
        report_rect(layer, MARGIN_LEFT, page_h - 32.0, 16.0, 16.0, green.clone());
        layer.set_fill_color(Color::Rgb(Rgb::new(1.0, 1.0, 1.0, None)));
        layer.use_text(
            "FS",
            10.0,
            Mm(MARGIN_LEFT + 4.0),
            Mm(page_h - 26.0),
            helvetica_bold,
        );
    }
    let brand_x = MARGIN_LEFT + 20.0;
    let top = page_h - 17.0;

    layer.set_fill_color(Color::Rgb(green.clone()));
    layer.use_text(&input.shop_name, 15.0, Mm(brand_x), Mm(top), helvetica_bold);
    layer.set_fill_color(Color::Rgb(muted.clone()));
    let address = input.shop_address.as_deref().unwrap_or("");
    layer.use_text(
        fit_report_text(address, page_w * 0.44, 8.0),
        8.0,
        Mm(brand_x),
        Mm(top - 6.5),
        helvetica,
    );
    if let Some(phone) = &input.shop_phone {
        layer.use_text(
            format!("Tel: {}", fit_report_text(phone, page_w * 0.35, 8.0)),
            8.0,
            Mm(brand_x),
            Mm(top - 12.0),
            helvetica,
        );
    }

    let title = fit_report_text(&input.title.to_uppercase(), page_w * 0.43, 15.0);
    let title_x = page_w - MARGIN_RIGHT - estimated_text_width(&title, 15.0);
    layer.set_fill_color(Color::Rgb(green.clone()));
    layer.use_text(
        &title,
        15.0,
        Mm(title_x.max(page_w * 0.52)),
        Mm(top),
        helvetica_bold,
    );
    let period = format!("Period: {}", input.filter_summary);
    let period_x = page_w - MARGIN_RIGHT - estimated_text_width(&period, 8.0);
    layer.set_fill_color(Color::Rgb(muted.clone()));
    layer.use_text(
        &period,
        8.0,
        Mm(period_x.max(page_w * 0.52)),
        Mm(top - 7.0),
        helvetica,
    );
    let generated = input.generated_by.as_ref().map_or_else(
        || format!("Generated {}", input.generated_at),
        |user| format!("Generated {} by {}", input.generated_at, user),
    );
    let generated = fit_report_text(&generated, page_w * 0.43, 7.0);
    let generated_x = page_w - MARGIN_RIGHT - estimated_text_width(&generated, 7.0);
    layer.use_text(
        &generated,
        7.0,
        Mm(generated_x.max(page_w * 0.52)),
        Mm(top - 12.0),
        helvetica,
    );

    let divider_y = page_h - 37.0;
    report_line(
        layer,
        MARGIN_LEFT,
        page_w - MARGIN_RIGHT,
        divider_y,
        green.clone(),
    );
    let header_bottom = divider_y - 13.0;
    report_rect(
        layer,
        MARGIN_LEFT,
        header_bottom,
        page_w - MARGIN_LEFT - MARGIN_RIGHT,
        9.0,
        green,
    );
    layer.set_fill_color(Color::Rgb(Rgb::new(1.0, 1.0, 1.0, None)));
    for (i, col) in input.columns.iter().enumerate() {
        let text = fit_report_text(&col.header, col_end[i] - col_x[i] - 4.0, 8.0);
        let x = if col.align_left {
            col_x[i] + 2.0
        } else {
            col_end[i] - 2.0 - estimated_text_width(&text, 8.0)
        };
        layer.use_text(
            text,
            8.0,
            Mm(x.max(col_x[i] + 1.0)),
            Mm(header_bottom + 3.0),
            helvetica_bold,
        );
    }
    header_bottom - 6.0
}

pub fn generate_report_pdf(
    input: &ReportPdfInput,
    _fonts_dir: &Path,
    reports_dir: &Path,
) -> Result<ReportPdf, AppError> {
    if input.columns.is_empty() {
        return Err(AppError::Pdf("report has no columns".into()));
    }
    let page_w = if input.landscape { PAGE_H } else { PAGE_W };
    let page_h = if input.landscape { PAGE_W } else { PAGE_H };
    let content_w = page_w - MARGIN_LEFT - MARGIN_RIGHT;

    let (doc, page1, layer1) = PdfDocument::new(&input.shop_name, Mm(page_w), Mm(page_h), "Report");

    let helvetica = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    let helvetica_bold = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| AppError::Pdf(e.to_string()))?;

    let total_ratio: f32 = input.columns.iter().map(|c| c.width_ratio).sum();
    let mut col_x: Vec<f32> = Vec::with_capacity(input.columns.len());
    let mut col_end: Vec<f32> = Vec::with_capacity(input.columns.len());
    let mut x = MARGIN_LEFT;
    for col in &input.columns {
        col_x.push(x);
        let w = content_w * (col.width_ratio / total_ratio);
        col_end.push(x + w);
        x += w;
    }

    let mut pages = vec![page1];
    let mut layer = doc.get_page(page1).get_layer(layer1);
    let logo = load_report_logo(input.logo_path.as_deref());

    let mut y = draw_report_header(
        &layer,
        input,
        page_w,
        page_h,
        logo.as_ref(),
        &helvetica,
        &col_x,
        &col_end,
        &helvetica_bold,
    );

    for (row_index, row) in input.rows.iter().enumerate() {
        if y < 27.0 {
            let page_number = pages.len() + 1;
            let (page_ref, layer_ref) =
                doc.add_page(Mm(page_w), Mm(page_h), format!("Report p{page_number}"));
            pages.push(page_ref);
            layer = doc.get_page(page_ref).get_layer(layer_ref);
            y = draw_report_header(
                &layer,
                input,
                page_w,
                page_h,
                logo.as_ref(),
                &helvetica,
                &col_x,
                &col_end,
                &helvetica_bold,
            );
        }
        if row_index % 2 == 1 {
            report_rect(
                &layer,
                MARGIN_LEFT,
                y - 2.2,
                content_w,
                ROW_HEIGHT,
                Rgb::new(0.975, 0.988, 0.978, None),
            );
        }
        layer.set_fill_color(Color::Rgb(Rgb::new(0.09, 0.10, 0.10, None)));
        for (i, col) in input.columns.iter().enumerate() {
            let cell = row.get(i).map(String::as_str).unwrap_or("");
            let text = fit_report_text(cell, col_end[i] - col_x[i] - 4.0, 8.0);
            let x_pos = if col.align_left {
                col_x[i] + 2.0
            } else {
                col_end[i] - 2.0 - estimated_text_width(&text, 8.0)
            };
            layer.use_text(text, 8.0, Mm(x_pos.max(col_x[i] + 1.0)), Mm(y), &helvetica);
        }
        report_line(
            &layer,
            MARGIN_LEFT,
            page_w - MARGIN_RIGHT,
            y - 2.4,
            Rgb::new(0.87, 0.91, 0.88, None),
        );
        y -= ROW_HEIGHT;
    }

    let ending_height = if input.totals.is_some() { 10.0 } else { 0.0 }
        + if input.summary.is_empty() { 0.0 } else { 23.0 };
    if ending_height > 0.0 && y - ending_height < 24.0 {
        let page_number = pages.len() + 1;
        let (page_ref, layer_ref) =
            doc.add_page(Mm(page_w), Mm(page_h), format!("Report p{page_number}"));
        pages.push(page_ref);
        layer = doc.get_page(page_ref).get_layer(layer_ref);
        y = draw_report_header(
            &layer,
            input,
            page_w,
            page_h,
            logo.as_ref(),
            &helvetica,
            &col_x,
            &col_end,
            &helvetica_bold,
        );
    }

    if let Some(totals) = &input.totals {
        report_line(
            &layer,
            MARGIN_LEFT,
            page_w - MARGIN_RIGHT,
            y + 2.0,
            Rgb::new(0.11, 0.29, 0.18, None),
        );
        layer.set_fill_color(Color::Rgb(Rgb::new(0.11, 0.29, 0.18, None)));
        for (i, col) in input.columns.iter().enumerate() {
            let cell = totals.get(i).map(String::as_str).unwrap_or("");
            let text = fit_report_text(cell, col_end[i] - col_x[i] - 4.0, 8.5);
            let x_pos = if col.align_left {
                col_x[i] + 2.0
            } else {
                col_end[i] - 2.0 - estimated_text_width(&text, 8.5)
            };
            layer.use_text(
                text,
                8.5,
                Mm(x_pos.max(col_x[i] + 1.0)),
                Mm(y - 3.0),
                &helvetica_bold,
            );
        }
        y -= 10.0;
    }

    if !input.summary.is_empty() {
        let summary_y = y - 17.0;
        report_rect(
            &layer,
            MARGIN_LEFT,
            summary_y,
            content_w,
            17.0,
            Rgb::new(0.96, 0.98, 0.965, None),
        );
        let items = input.summary.iter().take(4).collect::<Vec<_>>();
        let item_width = content_w / items.len() as f32;
        for (index, (label, value)) in items.iter().enumerate() {
            let x = MARGIN_LEFT + index as f32 * item_width + 4.0;
            layer.set_fill_color(Color::Rgb(Rgb::new(0.36, 0.42, 0.38, None)));
            layer.use_text(
                fit_report_text(&label.to_uppercase(), item_width - 8.0, 7.0),
                7.0,
                Mm(x),
                Mm(summary_y + 10.5),
                &helvetica_bold,
            );
            layer.set_fill_color(Color::Rgb(Rgb::new(0.11, 0.29, 0.18, None)));
            layer.use_text(
                fit_report_text(value, item_width - 8.0, 10.0),
                10.0,
                Mm(x),
                Mm(summary_y + 4.0),
                &helvetica_bold,
            );
        }
    }

    let page_count = pages.len();
    for (index, page_ref) in pages.into_iter().enumerate() {
        let footer = doc.get_page(page_ref).add_layer("Report footer");
        report_line(
            &footer,
            MARGIN_LEFT,
            page_w - MARGIN_RIGHT,
            20.0,
            Rgb::new(0.80, 0.85, 0.81, None),
        );
        footer.set_fill_color(Color::Rgb(Rgb::new(0.38, 0.42, 0.39, None)));
        let page_label = format!("Page {} of {}", index + 1, page_count);
        footer.use_text(
            &page_label,
            7.0,
            Mm(page_w - MARGIN_RIGHT - estimated_text_width(&page_label, 7.0)),
            Mm(14.0),
            &helvetica,
        );
        draw_developer_footer(&footer, page_w, 7.0, 7.0, &helvetica, &helvetica_bold);
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

#[cfg(test)]
mod report_pdf_tests {
    use super::*;

    fn columns() -> Vec<ReportPdfColumn> {
        vec![
            ReportPdfColumn {
                header: "Date".into(),
                width_ratio: 1.4,
                align_left: true,
            },
            ReportPdfColumn {
                header: "Reference".into(),
                width_ratio: 1.5,
                align_left: true,
            },
            ReportPdfColumn {
                header: "Customer".into(),
                width_ratio: 2.5,
                align_left: true,
            },
            ReportPdfColumn {
                header: "Total".into(),
                width_ratio: 1.6,
                align_left: false,
            },
            ReportPdfColumn {
                header: "Paid".into(),
                width_ratio: 1.6,
                align_left: false,
            },
            ReportPdfColumn {
                header: "Due".into(),
                width_ratio: 1.6,
                align_left: false,
            },
            ReportPdfColumn {
                header: "Status".into(),
                width_ratio: 1.2,
                align_left: true,
            },
        ]
    }

    fn row(index: usize) -> Vec<String> {
        vec![
            "2026-09-12".into(),
            format!("INV-{index:04}"),
            format!("Customer {index}"),
            format!("PKR {:},000.00", index + 10),
            format!("PKR {:},500.00", index + 5),
            "PKR 4,500.00".into(),
            "Confirmed".into(),
        ]
    }

    fn inspect_pdf(path: &str, expected_pages: usize, expected_rows: usize) {
        let document = printpdf::lopdf::Document::load(path).unwrap();
        let pages = document.get_pages();
        assert_eq!(pages.len(), expected_pages);
        let mut combined = String::new();
        for page_id in pages.values() {
            let content = document.get_page_content(*page_id).unwrap();
            assert!(
                content.len() > 500,
                "page content stream should not be blank"
            );
            let operations = printpdf::lopdf::content::Content::decode(&content).unwrap();
            let mut page_text = String::new();
            for operation in operations.operations {
                if operation.operator == "Tj" {
                    for operand in operation.operands {
                        if let Ok(bytes) = operand.as_str() {
                            page_text.push_str(&String::from_utf8_lossy(bytes));
                        }
                    }
                }
            }
            assert!(
                page_text.contains("Page"),
                "each page should contain its page number"
            );
            assert!(
                page_text.contains("Software developed by EagleNest Creations (0346-4451505)"),
                "each page should contain the developer footer"
            );
            combined.push_str(&page_text);
        }
        assert_eq!(combined.matches("INV-").count(), expected_rows);
    }

    #[test]
    fn generates_short_and_multi_page_a4_reports() {
        let output = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/report-smoke");
        fs::create_dir_all(&output).unwrap();
        let logo_path = output.join("logo.png");
        image::RgbImage::from_pixel(40, 40, image::Rgb([28, 74, 46]))
            .save(&logo_path)
            .unwrap();

        let base = |rows: Vec<Vec<String>>| ReportPdfInput {
            title: "Sales Report".into(),
            shop_name: "EagleNest Furniture".into(),
            shop_address: Some("Main Showroom, Lahore".into()),
            shop_phone: Some("+92 300 1234567".into()),
            logo_path: Some(logo_path.clone()),
            filter_summary: "2026-09-01 to 2026-09-12".into(),
            generated_at: "2026-09-12 12:00:00 PKT".into(),
            generated_by: Some("admin".into()),
            landscape: true,
            columns: columns(),
            rows,
            totals: None,
            summary: vec![
                ("Net Revenue".into(), "PKR 985,000.00".into()),
                ("Outstanding".into(), "PKR 54,000.00".into()),
            ],
        };

        let short =
            generate_report_pdf(&base(vec![row(1), row(2)]), Path::new(""), &output).unwrap();
        assert_eq!(short.pages, 1);
        assert!(short.bytes > 1_000);
        inspect_pdf(&short.path, 1, 2);

        let long_rows = (1..=75).map(row).collect();
        let long = generate_report_pdf(&base(long_rows), Path::new(""), &output).unwrap();
        assert_eq!(long.pages, 5);
        assert!(long.bytes > short.bytes);
        inspect_pdf(&long.path, 5, 75);
        println!("SHORT_PDF={}", short.path);
        println!("LONG_PDF={}", long.path);
    }

    #[test]
    fn generates_short_invoice_with_bottom_footer() {
        let output = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/report-smoke");
        fs::create_dir_all(&output).unwrap();
        let invoice = generate_invoice_pdf(
            &output,
            &InvoiceRecord {
                number: "INV-FOOTER-TEST".into(),
                sale_date: "2026-09-12".into(),
                customer_name: Some("Test Customer".into()),
                customer_phone: None,
                customer_address: None,
                notes: None,
                footer_text: Some("Thank you for your business!".into()),
                shop_name: "EagleNest Furniture".into(),
                shop_address: Some("Main Showroom, Lahore".into()),
                items: vec![InvoiceLine {
                    article: "FS-029".into(),
                    name: "5-Tier Open Bookshelf".into(),
                    quantity: 1,
                    price_minor: 1_400_000,
                    total_minor: 1_400_000,
                }],
                subtotal_minor: 1_400_000,
                discount_minor: 0,
                delivery_charge_minor: 0,
                tax_minor: 0,
                total_minor: 1_400_000,
                paid_minor: 1_400_000,
                advance_used_minor: 0,
                due_minor: 0,
            },
        )
        .unwrap();
        assert_eq!(invoice.pages, 1);
        assert!(invoice.bytes > 1_000);

        let document = printpdf::lopdf::Document::load(&invoice.path).unwrap();
        let page_id = *document.get_pages().values().next().unwrap();
        let content = document.get_page_content(page_id).unwrap();
        let operations = printpdf::lopdf::content::Content::decode(&content).unwrap();
        let mut page_text = String::new();
        for operation in operations.operations {
            if operation.operator == "Tj" {
                for operand in operation.operands {
                    if let Ok(bytes) = operand.as_str() {
                        page_text.push_str(&String::from_utf8_lossy(bytes));
                    }
                }
            }
        }
        assert!(page_text.contains("Software developed by EagleNest Creations (0346-4451505)"));
        println!("INVOICE_PDF={}", invoice.path);
    }
}
