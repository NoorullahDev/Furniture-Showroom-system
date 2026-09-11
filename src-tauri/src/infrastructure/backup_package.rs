use std::collections::HashSet;
use std::fs;
use std::io::{Read, Seek, Write};
use std::path::{Component, Path, PathBuf};

use chrono::Utc;
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::AppError;
use crate::infrastructure::backup::{create_sqlite_snapshot, integrity_check, sha256_file};
use crate::infrastructure::backup_preferences::validate_directory;
use crate::infrastructure::disk_size::free_disk_bytes;
use crate::infrastructure::FilePaths;

const MAGIC: &[u8] = b"FSHOP-BACKUP-1\n";
const FORMAT_VERSION: u32 = 1;
const MAX_MANIFEST_BYTES: u64 = 8 * 1024 * 1024;
const PACKAGE_EXTENSION: &str = "furniture-backup";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageEntry {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentMapping {
    pub expense_id: i64,
    pub archive_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageManifest {
    pub format_version: u32,
    pub app_version: String,
    pub schema_version: i64,
    pub created_at: String,
    pub kind: String,
    pub files: Vec<PackageEntry>,
    #[serde(default)]
    pub expense_attachments: Vec<AttachmentMapping>,
}

#[derive(Debug, Clone)]
pub struct CreatedPackage {
    pub name: String,
    pub path: PathBuf,
    pub sha256: String,
    pub bytes: u64,
    pub created_at: String,
    pub kind: String,
}

#[derive(Debug, Clone)]
pub struct PackageInspection {
    pub name: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub sha256: String,
    pub created_at: String,
    pub kind: String,
    pub app_version: String,
    pub schema_version: i64,
    pub file_count: usize,
    pub legacy_database_only: bool,
}

#[derive(Debug)]
struct SourceFile {
    archive_path: String,
    source_path: PathBuf,
}

struct WorkDir(PathBuf);

impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn create_package(
    paths: &FilePaths,
    destination_dir: &Path,
    requested_name: &str,
    kind: &str,
    schema_version: i64,
) -> Result<CreatedPackage, AppError> {
    let destination_dir = validate_directory(destination_dir)?;
    let work = WorkDir(
        paths
            .data_dir
            .join(format!(".backup-work-{}", uuid::Uuid::new_v4())),
    );
    fs::create_dir_all(&work.0)?;
    let snapshot = work.0.join("furniture_shop.db");
    create_sqlite_snapshot(&paths.db_path, &snapshot)?;
    if !integrity_check(&snapshot)? {
        return Err(AppError::Integrity(
            "SQLite snapshot failed integrity verification".into(),
        ));
    }

    let mut sources = vec![SourceFile {
        archive_path: "database/furniture_shop.db".into(),
        source_path: snapshot.clone(),
    }];
    gather_directory(&paths.images_dir, "images", &mut sources)?;
    gather_directory(&paths.branding_dir, "branding", &mut sources)?;
    gather_directory(&paths.fonts_dir, "fonts", &mut sources)?;
    gather_directory(
        &paths.data_dir.join("attachments"),
        "attachments",
        &mut sources,
    )?;
    gather_directory(&paths.data_dir.join("templates"), "templates", &mut sources)?;

    let mappings = gather_expense_attachments(&snapshot, &paths.data_dir, &mut sources)?;
    let mut seen = HashSet::new();
    sources.retain(|source| seen.insert(source.archive_path.clone()));
    sources.sort_by(|a, b| a.archive_path.cmp(&b.archive_path));

    let mut files = Vec::with_capacity(sources.len());
    let mut payload_size = 0u64;
    for source in &sources {
        validate_archive_path(&source.archive_path)?;
        let size = fs::metadata(&source.source_path)?.len();
        payload_size = payload_size
            .checked_add(size)
            .ok_or_else(|| AppError::Backup("backup package is too large".into()))?;
        files.push(PackageEntry {
            path: source.archive_path.clone(),
            size,
            sha256: sha256_file(&source.source_path)?,
        });
    }

    if let Some(free) = free_disk_bytes(&destination_dir)? {
        if free < payload_size.saturating_add(16 * 1024 * 1024) {
            return Err(AppError::Backup(format!(
                "not enough free space in {} to create this backup",
                destination_dir.display()
            )));
        }
    }

    let created_at = Utc::now().to_rfc3339();
    let manifest = PackageManifest {
        format_version: FORMAT_VERSION,
        app_version: env!("CARGO_PKG_VERSION").into(),
        schema_version,
        created_at: created_at.clone(),
        kind: kind.into(),
        files,
        expense_attachments: mappings,
    };
    let manifest_bytes = serde_json::to_vec(&manifest)
        .map_err(|e| AppError::Backup(format!("encode backup manifest: {e}")))?;
    if manifest_bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(AppError::Backup("backup manifest is too large".into()));
    }

    let name = unique_package_name(&destination_dir, requested_name);
    let final_path = destination_dir.join(&name);
    let partial_path = destination_dir.join(format!(".{name}.{}.partial", uuid::Uuid::new_v4()));
    let write_result = (|| -> Result<(), AppError> {
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial_path)?;
        output.write_all(MAGIC)?;
        output.write_all(&(manifest_bytes.len() as u64).to_le_bytes())?;
        output.write_all(&manifest_bytes)?;
        let mut buffer = vec![0u8; 1024 * 1024];
        for source in &sources {
            let mut input = fs::File::open(&source.source_path)?;
            loop {
                let read = input.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                output.write_all(&buffer[..read])?;
            }
        }
        output.sync_all()?;
        validate_package(&partial_path, schema_version, None)?;
        fs::rename(&partial_path, &final_path)?;
        Ok(())
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_file(&partial_path);
        return Err(error);
    }

    Ok(CreatedPackage {
        name,
        path: final_path.clone(),
        sha256: sha256_file(&final_path)?,
        bytes: fs::metadata(&final_path)?.len(),
        created_at,
        kind: kind.into(),
    })
}

pub fn inspect_backup(
    source: &Path,
    current_schema_version: i64,
    scratch_parent: &Path,
) -> Result<PackageInspection, AppError> {
    if !source.exists() || !source.is_file() {
        return Err(AppError::NotFound(format!(
            "backup file does not exist: {}",
            source.display()
        )));
    }
    match source
        .extension()
        .and_then(|v| v.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some(PACKAGE_EXTENSION) => {
            validate_package(source, current_schema_version, Some(scratch_parent))
        }
        Some("db") => inspect_legacy_database(source, current_schema_version),
        _ => Err(AppError::Validation(
            "select a .furniture-backup package or a legacy .db backup".into(),
        )),
    }
}

pub fn stage_backup(
    source: &Path,
    stage_dir: &Path,
    current_schema_version: i64,
    final_data_dir: &Path,
) -> Result<PackageInspection, AppError> {
    if stage_dir.exists() {
        return Err(AppError::Conflict(
            "restore staging directory already exists".into(),
        ));
    }
    fs::create_dir_all(stage_dir)?;
    for dir in [
        "database",
        "images",
        "branding",
        "fonts",
        "attachments",
        "templates",
    ] {
        fs::create_dir_all(stage_dir.join(dir))?;
    }

    let result = match source
        .extension()
        .and_then(|v| v.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some(PACKAGE_EXTENSION) => {
            extract_package(source, stage_dir, current_schema_version, final_data_dir)
        }
        Some("db") => {
            let inspection = inspect_legacy_database(source, current_schema_version)?;
            fs::copy(source, stage_dir.join("database/furniture_shop.db"))?;
            prepare_staged_database(
                &stage_dir.join("database/furniture_shop.db"),
                &[],
                final_data_dir,
            )?;
            Ok(inspection)
        }
        _ => Err(AppError::Validation("unsupported backup file type".into())),
    };
    if result.is_err() {
        let _ = fs::remove_dir_all(stage_dir);
    }
    result
}

fn validate_package(
    source: &Path,
    current_schema_version: i64,
    scratch_parent: Option<&Path>,
) -> Result<PackageInspection, AppError> {
    let scratch = scratch_parent
        .map(|parent| WorkDir(parent.join(format!(".backup-verify-{}", uuid::Uuid::new_v4()))));
    if let Some(work) = &scratch {
        fs::create_dir_all(&work.0)?;
    }
    let mut file = fs::File::open(source)?;
    let manifest = read_manifest(&mut file, current_schema_version)?;
    let mut db_copy = scratch.as_ref().map(|work| work.0.join("verify.db"));
    verify_or_extract_entries(&mut file, &manifest, db_copy.as_mut(), None)?;
    if let Some(db_path) = db_copy {
        if !integrity_check(&db_path)? {
            return Err(AppError::Integrity(
                "backup database failed integrity verification".into(),
            ));
        }
        validate_database_schema(&db_path, current_schema_version)?;
    } else {
        let db_entry = manifest
            .files
            .iter()
            .any(|entry| entry.path == "database/furniture_shop.db");
        if !db_entry {
            return Err(AppError::Integrity(
                "backup package does not contain a database".into(),
            ));
        }
    }
    inspection_from_manifest(source, &manifest)
}

fn extract_package(
    source: &Path,
    stage_dir: &Path,
    current_schema_version: i64,
    final_data_dir: &Path,
) -> Result<PackageInspection, AppError> {
    let mut file = fs::File::open(source)?;
    let manifest = read_manifest(&mut file, current_schema_version)?;
    let required: u64 = manifest
        .files
        .iter()
        .try_fold(0u64, |total, item| total.checked_add(item.size))
        .ok_or_else(|| AppError::Backup("backup size overflow".into()))?;
    if let Some(free) = free_disk_bytes(stage_dir)? {
        if free < required.saturating_add(32 * 1024 * 1024) {
            return Err(AppError::Backup(
                "not enough disk space to stage this restore".into(),
            ));
        }
    }
    verify_or_extract_entries(&mut file, &manifest, None, Some(stage_dir))?;
    let db_path = stage_dir.join("database/furniture_shop.db");
    if !db_path.exists() || !integrity_check(&db_path)? {
        return Err(AppError::Integrity(
            "backup database failed integrity verification".into(),
        ));
    }
    validate_database_schema(&db_path, current_schema_version)?;
    prepare_staged_database(&db_path, &manifest.expense_attachments, final_data_dir)?;
    inspection_from_manifest(source, &manifest)
}

fn read_manifest(
    file: &mut fs::File,
    current_schema_version: i64,
) -> Result<PackageManifest, AppError> {
    let mut magic = vec![0u8; MAGIC.len()];
    file.read_exact(&mut magic)
        .map_err(|_| AppError::Integrity("backup header is truncated".into()))?;
    if magic != MAGIC {
        return Err(AppError::Integrity(
            "backup header is not recognized".into(),
        ));
    }
    let mut length = [0u8; 8];
    file.read_exact(&mut length)
        .map_err(|_| AppError::Integrity("backup manifest length is missing".into()))?;
    let length = u64::from_le_bytes(length);
    if length == 0 || length > MAX_MANIFEST_BYTES {
        return Err(AppError::Integrity(
            "backup manifest length is invalid".into(),
        ));
    }
    let mut raw = vec![0u8; length as usize];
    file.read_exact(&mut raw)
        .map_err(|_| AppError::Integrity("backup manifest is truncated".into()))?;
    let manifest: PackageManifest = serde_json::from_slice(&raw)
        .map_err(|e| AppError::Integrity(format!("backup manifest is invalid: {e}")))?;
    if manifest.format_version != FORMAT_VERSION {
        return Err(AppError::Validation(format!(
            "backup format version {} is not supported by this application",
            manifest.format_version
        )));
    }
    if manifest.schema_version > current_schema_version {
        return Err(AppError::Validation(format!(
            "backup requires database schema {}, but this application supports up to {}",
            manifest.schema_version, current_schema_version
        )));
    }
    if manifest.files.is_empty() || manifest.files.len() > 100_000 {
        return Err(AppError::Integrity("backup file list is invalid".into()));
    }
    let mut paths = HashSet::new();
    for entry in &manifest.files {
        validate_archive_path(&entry.path)?;
        if !paths.insert(entry.path.as_str()) {
            return Err(AppError::Integrity(format!(
                "duplicate archive path: {}",
                entry.path
            )));
        }
    }
    Ok(manifest)
}

fn verify_or_extract_entries(
    file: &mut fs::File,
    manifest: &PackageManifest,
    mut database_copy: Option<&mut PathBuf>,
    extraction_root: Option<&Path>,
) -> Result<(), AppError> {
    let mut buffer = vec![0u8; 1024 * 1024];
    for entry in &manifest.files {
        let mut remaining = entry.size;
        let mut hasher = Sha256::new();
        let mut output = if let Some(root) = extraction_root {
            let target = safe_join(root, &entry.path)?;
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            Some(fs::File::create(target)?)
        } else if entry.path == "database/furniture_shop.db" {
            database_copy
                .as_deref_mut()
                .map(fs::File::create)
                .transpose()?
        } else {
            None
        };
        while remaining > 0 {
            let chunk = std::cmp::min(remaining, buffer.len() as u64) as usize;
            file.read_exact(&mut buffer[..chunk]).map_err(|_| {
                AppError::Integrity(format!("backup entry is truncated: {}", entry.path))
            })?;
            hasher.update(&buffer[..chunk]);
            if let Some(destination) = &mut output {
                destination.write_all(&buffer[..chunk])?;
            }
            remaining -= chunk as u64;
        }
        if hex(&hasher.finalize()) != entry.sha256 {
            return Err(AppError::Integrity(format!(
                "checksum mismatch for {}",
                entry.path
            )));
        }
        if let Some(destination) = &mut output {
            destination.sync_all()?;
        }
    }
    let position = file.stream_position()?;
    let total = file.metadata()?.len();
    if position != total {
        return Err(AppError::Integrity(
            "backup contains undeclared trailing data".into(),
        ));
    }
    Ok(())
}

fn prepare_staged_database(
    db_path: &Path,
    attachments: &[AttachmentMapping],
    final_data_dir: &Path,
) -> Result<(), AppError> {
    let conn = Connection::open(db_path)
        .map_err(|e| AppError::Backup(format!("open staged database: {e}")))?;
    conn.execute("UPDATE sessions SET active = 0, locked_at = NULL", [])
        .map_err(|e| AppError::Backup(format!("invalidate restored sessions: {e}")))?;
    for mapping in attachments {
        validate_archive_path(&mapping.archive_path)?;
        let relative = mapping
            .archive_path
            .strip_prefix("attachments/")
            .ok_or_else(|| {
                AppError::Integrity("expense attachment is outside the attachment area".into())
            })?;
        let restored = final_data_dir.join("attachments").join(relative);
        conn.execute(
            "UPDATE expenses SET attachment_path = ? WHERE id = ?",
            rusqlite::params![restored.to_string_lossy(), mapping.expense_id],
        )
        .map_err(|e| AppError::Backup(format!("prepare expense attachment path: {e}")))?;
    }
    Ok(())
}

/// Machine-scoped settings are deliberately kept from the current install.
/// In particular, a restored snapshot cannot act as license activation and it
/// cannot redirect the owner's configured backup destination.
pub fn preserve_machine_settings(
    db_path: &Path,
    settings: &[(String, String)],
) -> Result<(), AppError> {
    let mut conn = Connection::open(db_path)
        .map_err(|e| AppError::Backup(format!("open staged settings: {e}")))?;
    let transaction = conn
        .transaction()
        .map_err(|e| AppError::Backup(format!("begin staged settings update: {e}")))?;
    for (key, value) in settings {
        transaction
            .execute(
                "INSERT INTO settings (key, value_json, updated_by, updated_at)
             VALUES (?, ?, NULL, strftime('%Y-%m-%dT%H:%M:%fZ','now'))
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json,
               updated_by = NULL, updated_at = excluded.updated_at",
                rusqlite::params![key, value],
            )
            .map_err(|e| AppError::Backup(format!("preserve machine setting {key}: {e}")))?;
    }
    transaction
        .commit()
        .map_err(|e| AppError::Backup(format!("commit staged settings update: {e}")))?;
    Ok(())
}

fn inspect_legacy_database(
    source: &Path,
    current_schema_version: i64,
) -> Result<PackageInspection, AppError> {
    if !integrity_check(source)? {
        return Err(AppError::Integrity(
            "legacy database backup failed integrity verification".into(),
        ));
    }
    let schema_version = validate_database_schema(source, current_schema_version)?;
    let metadata = fs::metadata(source)?;
    Ok(PackageInspection {
        name: file_name(source)?,
        path: source.to_path_buf(),
        size_bytes: metadata.len(),
        sha256: sha256_file(source)?,
        created_at: modified_time(&metadata),
        kind: "legacy".into(),
        app_version: "Legacy database-only backup".into(),
        schema_version,
        file_count: 1,
        legacy_database_only: true,
    })
}

fn validate_database_schema(path: &Path, current: i64) -> Result<i64, AppError> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| AppError::Backup(format!("open backup database: {e}")))?;
    let has_migrations: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='_sqlx_migrations'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| AppError::Backup(format!("inspect backup schema: {e}")))?;
    let version = if has_migrations == 0 {
        0
    } else {
        conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations WHERE success = 1",
            [],
            |row| row.get(0),
        )
        .map_err(|e| AppError::Backup(format!("inspect backup schema version: {e}")))?
    };
    if version > current {
        return Err(AppError::Validation(format!(
            "backup schema {version} is newer than supported schema {current}"
        )));
    }
    Ok(version)
}

fn inspection_from_manifest(
    source: &Path,
    manifest: &PackageManifest,
) -> Result<PackageInspection, AppError> {
    Ok(PackageInspection {
        name: file_name(source)?,
        path: source.to_path_buf(),
        size_bytes: fs::metadata(source)?.len(),
        sha256: sha256_file(source)?,
        created_at: manifest.created_at.clone(),
        kind: manifest.kind.clone(),
        app_version: manifest.app_version.clone(),
        schema_version: manifest.schema_version,
        file_count: manifest.files.len(),
        legacy_database_only: false,
    })
}

fn gather_directory(
    root: &Path,
    prefix: &str,
    output: &mut Vec<SourceFile>,
) -> Result<(), AppError> {
    if !root.exists() {
        return Ok(());
    }
    gather_directory_inner(root, root, prefix, output)
}

fn gather_directory_inner(
    root: &Path,
    current: &Path,
    prefix: &str,
    output: &mut Vec<SourceFile>,
) -> Result<(), AppError> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(AppError::Backup(format!(
                "symbolic links are not supported in backups: {}",
                path.display()
            )));
        }
        if metadata.is_dir() {
            gather_directory_inner(root, &path, prefix, output)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| AppError::Internal("asset path escaped its root".into()))?;
            let relative = path_to_archive(relative)?;
            output.push(SourceFile {
                archive_path: format!("{prefix}/{relative}"),
                source_path: path,
            });
        }
    }
    Ok(())
}

fn gather_expense_attachments(
    snapshot: &Path,
    data_dir: &Path,
    output: &mut Vec<SourceFile>,
) -> Result<Vec<AttachmentMapping>, AppError> {
    let conn = Connection::open_with_flags(snapshot, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| AppError::Backup(format!("read snapshot attachments: {e}")))?;
    let table_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='expenses'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if table_exists == 0 {
        return Ok(Vec::new());
    }
    let mut statement = conn.prepare("SELECT id, attachment_path FROM expenses WHERE attachment_path IS NOT NULL AND trim(attachment_path) <> ''")
        .map_err(|e| AppError::Backup(format!("query snapshot attachments: {e}")))?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| AppError::Backup(format!("query snapshot attachments: {e}")))?;
    let managed_root = data_dir.join("attachments");
    let canonical_managed = fs::canonicalize(&managed_root).ok();
    let mut mappings = Vec::new();
    for row in rows {
        let (expense_id, stored_path) =
            row.map_err(|e| AppError::Backup(format!("read attachment row: {e}")))?;
        let source = PathBuf::from(&stored_path);
        if !source.exists() || !source.is_file() {
            return Err(AppError::Backup(format!(
                "expense {expense_id} references a missing attachment: {stored_path}"
            )));
        }
        let canonical = fs::canonicalize(&source)?;
        let archive_path = if let Some(root) = &canonical_managed {
            if let Ok(relative) = canonical.strip_prefix(root) {
                format!("attachments/{}", path_to_archive(relative)?)
            } else {
                external_attachment_path(expense_id, &canonical)
            }
        } else {
            external_attachment_path(expense_id, &canonical)
        };
        output.push(SourceFile {
            archive_path: archive_path.clone(),
            source_path: canonical,
        });
        mappings.push(AttachmentMapping {
            expense_id,
            archive_path,
        });
    }
    Ok(mappings)
}

fn external_attachment_path(expense_id: i64, source: &Path) -> String {
    let raw_name = source
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("bill");
    let name = sanitize_component(raw_name);
    format!("attachments/expenses/{expense_id}-{name}")
}

fn path_to_archive(path: &Path) -> Result<String, AppError> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => parts.push(value.to_string_lossy().into_owned()),
            _ => return Err(AppError::Validation("asset path is not portable".into())),
        }
    }
    if parts.is_empty() {
        return Err(AppError::Validation("asset path is empty".into()));
    }
    Ok(parts.join("/"))
}

fn validate_archive_path(path: &str) -> Result<(), AppError> {
    if path.is_empty() || path.contains('\\') || path.starts_with('/') || path.contains('\0') {
        return Err(AppError::Integrity(format!("unsafe archive path: {path}")));
    }
    let parsed = Path::new(path);
    if parsed
        .components()
        .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(AppError::Integrity(format!("unsafe archive path: {path}")));
    }
    Ok(())
}

fn safe_join(root: &Path, archive_path: &str) -> Result<PathBuf, AppError> {
    validate_archive_path(archive_path)?;
    Ok(root.join(archive_path.replace('/', std::path::MAIN_SEPARATOR_STR)))
}

fn unique_package_name(destination: &Path, requested: &str) -> String {
    let requested_had_extension = requested
        .trim()
        .to_ascii_lowercase()
        .ends_with(".furniture-backup");
    let base = requested
        .trim()
        .trim_end_matches(".furniture-backup")
        .trim_end_matches(".db");
    let base = sanitize_component(if base.is_empty() {
        "Furniture-Shop"
    } else {
        base
    });
    let timestamp = Utc::now().format("%Y%m%d-%H%M%S");
    let first = if requested_had_extension && has_timestamp_suffix(&base) {
        format!("{base}.{PACKAGE_EXTENSION}")
    } else {
        format!("{base}-{timestamp}.{PACKAGE_EXTENSION}")
    };
    if !destination.join(&first).exists() {
        return first;
    }
    format!(
        "{base}-{timestamp}-{}.{}",
        &uuid::Uuid::new_v4().simple().to_string()[..8],
        PACKAGE_EXTENSION
    )
}

fn has_timestamp_suffix(value: &str) -> bool {
    let Some(suffix) = value.get(value.len().saturating_sub(15)..) else {
        return false;
    };
    suffix.as_bytes().iter().enumerate().all(|(index, byte)| {
        if index == 8 {
            *byte == b'-'
        } else {
            byte.is_ascii_digit()
        }
    })
}

fn sanitize_component(value: &str) -> String {
    let mut result = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ' ' | '.') {
            result.push(ch);
        } else {
            result.push('-');
        }
    }
    let trimmed = result.trim_matches([' ', '.', '-']).trim();
    if trimmed.is_empty() {
        "backup".into()
    } else {
        trimmed.chars().take(80).collect()
    }
}

fn file_name(path: &Path) -> Result<String, AppError> {
    path.file_name()
        .map(|v| v.to_string_lossy().into_owned())
        .ok_or_else(|| AppError::Validation("backup path has no file name".into()))
}

fn modified_time(metadata: &fs::Metadata) -> String {
    metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|value| chrono::DateTime::from_timestamp(value.as_secs() as i64, 0))
        .map(|value| value.to_rfc3339())
        .unwrap_or_default()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn list_backup_files(
    directory: &Path,
    current_schema: i64,
    scratch_parent: &Path,
) -> Result<Vec<PackageInspection>, AppError> {
    let directory = validate_directory(directory)?;
    let mut results = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let extension = path
            .extension()
            .and_then(|v| v.to_str())
            .map(str::to_ascii_lowercase);
        if !matches!(extension.as_deref(), Some(PACKAGE_EXTENSION) | Some("db")) {
            continue;
        }
        if let Ok(item) = inspect_backup(&path, current_schema, scratch_parent) {
            results.push(item);
        } else {
            let metadata = entry.metadata()?;
            results.push(PackageInspection {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: path.clone(),
                size_bytes: metadata.len(),
                sha256: sha256_file(&path).unwrap_or_default(),
                created_at: modified_time(&metadata),
                kind: "unknown".into(),
                app_version: String::new(),
                schema_version: 0,
                file_count: 0,
                legacy_database_only: extension.as_deref() == Some("db"),
            });
        }
    }
    results.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(results)
}

pub fn delete_backup_file(directory: &Path, name: &str) -> Result<(), AppError> {
    if name.is_empty() || name.contains(['/', '\\']) {
        return Err(AppError::Validation("invalid backup name".into()));
    }
    let directory = validate_directory(directory)?;
    let target = directory.join(name);
    let canonical = fs::canonicalize(&target)
        .map_err(|_| AppError::NotFound(format!("backup `{name}` does not exist")))?;
    if canonical.parent() != Some(directory.as_path()) {
        return Err(AppError::Validation(
            "backup path escaped selected folder".into(),
        ));
    }
    fs::remove_file(canonical)?;
    Ok(())
}
