use sha2::{Digest, Sha256};

use crate::error::AppError;

/// Stable, privacy-preserving machine fingerprint derived from SMBIOS data.
/// Raw firmware identifiers never leave this module or get persisted.
pub fn hardware_id() -> Result<String, AppError> {
    let identifiers = firmware_identifiers()?;
    if identifiers.is_empty() {
        return Err(AppError::Internal(
            "Windows did not expose stable hardware identifiers".into(),
        ));
    }
    Ok(hardware_id_from(&identifiers))
}

fn hardware_id_from(identifiers: &[String]) -> String {
    let mut normalized: Vec<String> = identifiers
        .iter()
        .map(|value| value.trim().to_ascii_uppercase())
        .filter(|value| !value.is_empty())
        .collect();
    normalized.sort();
    normalized.dedup();
    let digest = Sha256::digest(normalized.join("|").as_bytes());
    let hex = digest[..8]
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<String>();
    format!(
        "{}-{}-{}-{}",
        &hex[0..4],
        &hex[4..8],
        &hex[8..12],
        &hex[12..16]
    )
}

#[cfg(windows)]
fn firmware_identifiers() -> Result<Vec<String>, AppError> {
    use windows_sys::Win32::System::SystemInformation::GetSystemFirmwareTable;

    // Win32 provider signatures use the numeric value of the C multi-character
    // constant 'RSMB' (0x52534D42), not the in-memory little-endian byte order.
    const RSMB: u32 = u32::from_be_bytes(*b"RSMB");
    let needed = unsafe { GetSystemFirmwareTable(RSMB, 0, std::ptr::null_mut(), 0) };
    if needed < 8 {
        return Err(AppError::Internal(
            "unable to read Windows SMBIOS firmware table".into(),
        ));
    }
    let mut raw = vec![0u8; needed as usize];
    let received = unsafe { GetSystemFirmwareTable(RSMB, 0, raw.as_mut_ptr(), needed) };
    if received != needed {
        return Err(AppError::Internal(
            "Windows returned an incomplete SMBIOS firmware table".into(),
        ));
    }

    let table_len = u32::from_le_bytes(raw[4..8].try_into().unwrap()) as usize;
    let end = 8usize.saturating_add(table_len).min(raw.len());
    let table = &raw[8..end];
    let mut result = Vec::new();
    let mut offset = 0usize;
    while offset + 4 <= table.len() {
        let kind = table[offset];
        let formatted_len = table[offset + 1] as usize;
        if formatted_len < 4 || offset + formatted_len > table.len() {
            break;
        }
        let strings_start = offset + formatted_len;
        let mut structure_end = strings_start;
        while structure_end + 1 < table.len()
            && !(table[structure_end] == 0 && table[structure_end + 1] == 0)
        {
            structure_end += 1;
        }
        let strings_end = structure_end.min(table.len());
        let strings = &table[strings_start..strings_end];
        let formatted = &table[offset..offset + formatted_len];

        match kind {
            0 if formatted_len > 7 => {
                push_smbios_string(&mut result, "BIOS", strings, formatted[7])
            }
            1 => {
                if formatted_len >= 24 {
                    let uuid = &formatted[8..24];
                    if uuid.iter().any(|byte| *byte != 0) && uuid.iter().any(|byte| *byte != 0xff) {
                        result.push(format!("UUID:{}", hex_bytes(uuid)));
                    }
                }
                if formatted_len > 7 {
                    push_smbios_string(&mut result, "SYSTEM", strings, formatted[7]);
                }
            }
            2 if formatted_len > 7 => {
                push_smbios_string(&mut result, "BOARD", strings, formatted[7])
            }
            127 => break,
            _ => {}
        }

        offset = (structure_end + 2).min(table.len());
    }
    Ok(result)
}

#[cfg(not(windows))]
fn firmware_identifiers() -> Result<Vec<String>, AppError> {
    for path in ["/etc/machine-id", "/var/lib/dbus/machine-id"] {
        if let Ok(value) = std::fs::read_to_string(path) {
            if !value.trim().is_empty() {
                return Ok(vec![format!("MACHINE:{}", value.trim())]);
            }
        }
    }
    Err(AppError::Internal(
        "stable machine identifiers are unavailable on this platform".into(),
    ))
}

#[cfg(windows)]
fn push_smbios_string(result: &mut Vec<String>, label: &str, strings: &[u8], index: u8) {
    if index == 0 {
        return;
    }
    if let Some(value) = strings.split(|byte| *byte == 0).nth((index - 1) as usize) {
        let value = String::from_utf8_lossy(value).trim().to_string();
        let upper = value.to_ascii_uppercase();
        let placeholder = value.is_empty()
            || upper.contains("TO BE FILLED")
            || upper.contains("DEFAULT STRING")
            || upper == "NONE"
            || upper == "UNKNOWN";
        if !placeholder {
            result.push(format!("{label}:{value}"));
        }
    }
}

#[cfg(windows)]
fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02X}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_stable_and_readable() {
        let a = hardware_id_from(&["BIOS:ABC".into(), "BOARD:XYZ".into()]);
        let b = hardware_id_from(&[" board:xyz ".into(), "bios:abc".into()]);
        assert_eq!(a, b);
        assert_eq!(a.len(), 19);
        assert_eq!(a.chars().filter(|c| *c == '-').count(), 3);
    }

    #[cfg(windows)]
    #[test]
    fn current_windows_machine_exposes_a_hardware_id() {
        let value = hardware_id().unwrap();
        assert_eq!(value.len(), 19);
    }
}
