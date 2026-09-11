use crate::dto::PrinterDto;
use crate::error::AppError;

#[cfg(windows)]
fn wide_string(pointer: *const u16) -> String {
    if pointer.is_null() {
        return String::new();
    }
    let mut length = 0usize;
    unsafe {
        while *pointer.add(length) != 0 {
            length += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(pointer, length))
    }
}

#[cfg(windows)]
fn default_printer() -> Option<String> {
    use windows_sys::Win32::Graphics::Printing::GetDefaultPrinterW;

    let mut length = 0u32;
    unsafe {
        GetDefaultPrinterW(std::ptr::null_mut(), &mut length);
    }
    if length == 0 {
        return None;
    }
    let mut buffer = vec![0u16; length as usize];
    let ok = unsafe { GetDefaultPrinterW(buffer.as_mut_ptr(), &mut length) };
    (ok != 0).then(|| {
        let used = buffer
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(buffer.len());
        String::from_utf16_lossy(&buffer[..used])
    })
}

#[cfg(windows)]
pub fn list() -> Result<Vec<PrinterDto>, AppError> {
    use windows_sys::Win32::Graphics::Printing::{
        EnumPrintersW, PRINTER_ENUM_CONNECTIONS, PRINTER_ENUM_LOCAL, PRINTER_INFO_4W,
    };

    let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
    let mut needed = 0u32;
    let mut returned = 0u32;
    unsafe {
        EnumPrintersW(
            flags,
            std::ptr::null(),
            4,
            std::ptr::null_mut(),
            0,
            &mut needed,
            &mut returned,
        );
    }
    if needed == 0 {
        return Ok(Vec::new());
    }

    let mut buffer = vec![0u8; needed as usize];
    let ok = unsafe {
        EnumPrintersW(
            flags,
            std::ptr::null(),
            4,
            buffer.as_mut_ptr(),
            needed,
            &mut needed,
            &mut returned,
        )
    };
    if ok == 0 {
        return Err(AppError::Internal(
            "Windows could not enumerate installed printers".into(),
        ));
    }

    let default = default_printer();
    let entries = unsafe {
        std::slice::from_raw_parts(buffer.as_ptr().cast::<PRINTER_INFO_4W>(), returned as usize)
    };
    let mut printers: Vec<PrinterDto> = entries
        .iter()
        .map(|entry| wide_string(entry.pPrinterName))
        .filter(|name| !name.is_empty())
        .map(|name| PrinterDto {
            is_default: default.as_deref() == Some(name.as_str()),
            name,
        })
        .collect();
    printers.sort_by_key(|left| left.name.to_lowercase());
    printers.dedup_by(|left, right| left.name.eq_ignore_ascii_case(&right.name));
    Ok(printers)
}

#[cfg(not(windows))]
pub fn list() -> Result<Vec<PrinterDto>, AppError> {
    Ok(Vec::new())
}
