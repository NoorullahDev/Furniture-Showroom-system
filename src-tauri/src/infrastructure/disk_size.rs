use std::fs;
use std::path::Path;

use crate::error::AppError;

/// Recursive size of a directory in bytes (including the immediate entry list).
pub fn dir_size(path: &Path) -> Result<u64, AppError> {
    let mut total = 0u64;
    visit(path, &mut total)?;
    Ok(total)
}

fn visit(path: &Path, total: &mut u64) -> Result<(), AppError> {
    if path.is_file() {
        *total += fs::metadata(path)?.len();
        return Ok(());
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            visit(&entry.path(), total)?;
        } else if file_type.is_file() {
            *total += entry.metadata()?.len();
        }
    }
    Ok(())
}

/// Free bytes on the volume containing `path`. Returns `None` on platforms
/// without a supported probe so callers can keep showing the rest of the
/// maintenance status.
pub fn free_disk_bytes(path: &Path) -> Result<Option<u64>, AppError> {
    let dir = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    free_disk_bytes_for_dir(dir)
}

#[cfg(windows)]
fn free_disk_bytes_for_dir(dir: &Path) -> Result<Option<u64>, AppError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let wide: Vec<u16> = dir
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut free_available_to_caller: u64 = 0;
    let mut total: u64 = 0;
    let mut total_free: u64 = 0;

    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free_available_to_caller,
            &mut total,
            &mut total_free,
        )
    };

    if ok == 0 {
        return Ok(None);
    }
    Ok(Some(free_available_to_caller))
}

#[cfg(not(windows))]
fn free_disk_bytes_for_dir(_dir: &Path) -> Result<Option<u64>, AppError> {
    Ok(None)
}
