# Packaging, Deployment, and Updates

## Target

Ship a signed 64-bit Windows installer for supported Windows 10 and 11 editions. Decide MSI or NSIS based on installation/update needs, then test that exact format.

## Build pipeline

1. Install locked Node and Rust dependencies.
2. Run formatting, lint, type check, audits, tests, and migration verification.
3. Build static Next.js frontend.
4. Build Tauri release binary.
5. Package installer, templates, embedded fonts, and required runtime assets.
6. Sign executable/installer when certificate is available.
7. Generate SHA-256 checksums and release notes.
8. Test installation and upgrade on clean Windows VMs.

## Installation behavior

- Install application binaries outside business data directory.
- Store database/images/settings in the platform app-data directory.
- Upgrade must not overwrite user data.
- Uninstall should clearly ask whether to retain business data; default to retention.
- Create owner-friendly shortcut and application identity.

## Release channels

- Stable: shop production systems.
- Pilot: internal/test installation only.
- No production user receives an untested development build.

## Update policy

Offline manual update is mandatory: user imports or runs a verified installer. Optional signed Tauri updater may be added later. Before schema-changing upgrade, create and verify a backup. Failed migration restores the pre-upgrade state.

## Versioning

Use semantic application versions and monotonic database migration numbers. Store app/database version in backups and diagnostic information. Release notes list user changes, schema implications, known issues, and backup recommendation.

## Configuration

Business settings live in the database and backup. Build-time configuration contains only safe product identity and feature defaults. Secrets and private signing keys never enter the repository or installed frontend.

## Support package

Authorized users can generate a diagnostic package containing app version, schema version, OS version, sanitized logs, migration status, and integrity results. It excludes passwords, license secrets, and customer data by default.

## Rollback

Binary rollback is allowed only when compatible with the current schema. Otherwise restore the pre-upgrade backup and matching app version. Never open a newer database with an older incompatible binary.

## Release checklist

Installer signature/checksum, clean install, upgrade from previous supported version, first run, login, primary workflows, print/PDF, backup/restore, 125% scaling, antivirus false-positive check, and release archive.

