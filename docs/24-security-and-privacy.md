# Security and Privacy

## Security goals

Protect business records from unauthorized access, accidental corruption, unsafe filesystem operations, dependency risk, and incomplete transactions.

## Tauri controls

- Use least-privilege Tauri capabilities per window.
- Do not enable shell execution, arbitrary URL opening, or unrestricted filesystem scope.
- Allow file pickers only for explicit imports, exports, images, and backups.
- Use a strict Content Security Policy compatible with local assets.
- Block remote scripts and untrusted web content in the application window.
- Validate every IPC request and authorize it in Rust.

## Data protection

- Passwords: Argon2id only.
- License secrets/keys: OS credential store where available.
- Database: stored in user app-data with restrictive permissions.
- Backups: optional strong encryption plus integrity checks.
- Logs/audit: redact passwords, tokens, full backup keys, and unnecessary personal data.

SQLite file encryption is not automatic. If theft-at-rest protection is required, use Windows device encryption/BitLocker or evaluate a supported encrypted SQLite build with licensing and recovery implications.

## Input and file safety

- Parameterized SQL only.
- Validate bounds, enum states, text length, money, quantity, dates, and IDs.
- Decode images and verify MIME/size; do not trust extension.
- Generate filenames and prevent traversal/symlink escape.
- Sanitize CSV cells and PDF text/layout.
- Do not deserialize untrusted backup data before integrity and format checks.

## Financial and inventory integrity

- Database constraints plus application validation.
- Atomic transactions for multi-record workflows.
- Idempotency keys for posting commands.
- Immutable posted records and reversal trails.
- Permission thresholds and audit logs.

## Dependency and build security

- Commit lockfiles.
- Run Rust and npm vulnerability audits in CI.
- Review Tauri plugin capabilities on every addition.
- Sign release installers when a code-signing certificate is available.
- Produce checksums for release artifacts.
- Never ship development tools, test credentials, source maps with secrets, or database samples containing real customers.

## Privacy

Collect only necessary customer and supplier contact data. Limit exports and reports by permission. Provide a controlled data-export process and legal deletion/anonymization policy that preserves required financial records.

## Incident response

On suspected compromise: stop affected installation, preserve logs and a read-only backup, change owner credentials, validate binaries and database integrity, restore if necessary, review audit activity, and document corrective action.

