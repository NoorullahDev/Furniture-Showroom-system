# Backup, Restore, and Data Lifecycle

## Backup contents

A backup package contains a consistent SQLite snapshot, product images, attachments, report templates, shop settings, schema/app version manifest, checksums, and optional audit/log subset. Generated PDFs can be excluded because they are reproducible unless legal policy requires retaining them.

## Backup schedule

- Automatic local backup daily on application close or scheduled idle period.
- Keep a configurable rotation, recommended 7 daily, 4 weekly, and 6 monthly.
- Prompt for a second destination such as an external drive.
- Manual `Back Up Now` available to authorized users.

## Backup algorithm

1. Resolve an approved destination with adequate space.
2. Use SQLite backup API/checkpoint-aware snapshot; never copy a live database blindly.
3. Copy referenced files into a staging package.
4. Write manifest and SHA-256 checksums.
5. Verify completeness and database integrity.
6. Finalize via atomic rename.
7. Record success/failure in backup history and audit.

## Restore workflow

1. Owner selects package.
2. Application validates format, checksum, database integrity, app compatibility, and available disk.
3. Show backup date, shop, record counts, version, and overwrite warning.
4. Create a safety backup of current data.
5. Close database, restore to staging, migrate if supported, verify, then atomically switch.
6. Restart application and run post-restore checks.

Restore failure must leave the existing installation usable.

## Encryption

Offer password-encrypted exported backup packages. Explain that forgotten backup passwords cannot be recovered. Do not store the export password in plaintext.

## Recovery cases

- Accidental deletion: restore full backup or use reversal/archive where possible.
- Failed migration: automatically return to pre-migration backup.
- Corrupt database: stop writes, copy evidence, run integrity checks, restore last verified backup.
- Missing image: flag the record without breaking products or invoices.

## Data lifecycle

- Master data: archive when historically referenced.
- Drafts with no side effects: deletable with permission.
- Posted finance/stock records: reverse, never delete.
- Logs: rotate by size/time and retain a configurable period.
- Temporary thumbnails/exports: clean after safe expiration.
- Orphan file cleanup runs as a previewable maintenance action.

## Restore drill

Before launch and quarterly thereafter, restore a production-like backup to a clean test profile and verify counts, balances, images, invoices, login, and reports.

