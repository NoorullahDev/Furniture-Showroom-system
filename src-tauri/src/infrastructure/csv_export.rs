use std::fs;
use std::io::BufWriter;
use std::path::Path;

pub struct CsvTable {
    pub title: String,
    pub generated_at: String,
    pub filter_summary: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub totals: Option<Vec<String>>,
}

fn sanitize_cell(value: &str) -> String {
    let trimmed = value.trim();
    if matches!(
        trimmed.chars().next(),
        Some('=' | '+' | '-' | '@' | '\t' | '\r')
    ) {
        format!("'{trimmed}")
    } else {
        trimmed.to_string()
    }
}

pub fn write_csv(table: &CsvTable, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let file = fs::File::create(path)?;
    let mut wtr = csv::Writer::from_writer(BufWriter::new(file));
    let cols = table.columns.len().max(2);

    let meta_row = |label: &str, value: &str| -> Vec<String> {
        let mut r = vec![label.to_string(), value.to_string()];
        r.resize(cols, String::new());
        r
    };

    wtr.write_record(meta_row("Report", &table.title))?;
    wtr.write_record(meta_row("Generated", &table.generated_at))?;
    wtr.write_record(meta_row("Filters", &table.filter_summary))?;

    wtr.write_record(table.columns.iter().map(|c| sanitize_cell(c)))?;

    for row in &table.rows {
        wtr.write_record(row.iter().map(|c| sanitize_cell(c)))?;
    }

    if let Some(totals) = &table.totals {
        let blank: Vec<String> = (0..cols).map(|_| String::new()).collect();
        wtr.write_record(blank.iter().map(|c| sanitize_cell(c)))?;
        let padded: Vec<String> = {
            let mut t = totals.clone();
            t.resize(cols, String::new());
            t
        };
        wtr.write_record(padded.iter().map(|c| sanitize_cell(c)))?;
    }

    wtr.flush()?;
    Ok(())
}
