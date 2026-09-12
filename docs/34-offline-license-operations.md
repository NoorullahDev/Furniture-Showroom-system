# Offline license operations

The installed Furniture Shop application contains only `src-tauri/keys/license_public.pem` and RSA-PSS/SHA-256 verification logic. The private key is not compiled or bundled.

## Private-key custody

Keep the issuer private key on an operator-only, encrypted machine. Never copy it into `src-tauri`, an installer, a customer computer, source control, backups, or support attachments. The repository ignores `license-issuer-private.pem` and `*.license-private.pem`, but access controls and offline backups remain the operator's responsibility.

To replace the signing identity, create a key pair outside the repository, replace the public PEM before building the customer application, and archive the previous issuer securely. The issuer utility refuses to overwrite existing keys:

```powershell
cd src-tauri
cargo run --example license_issuer -- generate-key --private D:\Secure\furniture-shop.license-private.pem --public keys\license_public.pem
```

Before issuing or building, confirm that the private key matches the public key embedded by the application:

```powershell
cd src-tauri
cargo run --example license_issuer -- check-key --private ..\license-issuer-private.pem --public keys\license_public.pem
```

## Issue a license

Ask the customer to copy the Hardware ID shown on the activation screen. No command prompt is required on the customer computer.

```powershell
cd src-tauri
cargo run --example license_issuer -- issue --private ..\license-issuer-private.pem --customer "Example Furniture Showroom" --hardware "C1FA-3534-1C7C-1FE9" --days 30
```

Use `--days 90`, `--days 365`, or any custom value from 1 through 3650. Send the single `FSLIC1...` output line to the customer. The customer pastes it into Activate/Renew License.

## Enforcement and storage

- Every normal backend command is denied until the signed license is valid; UI navigation is not a security boundary.
- The key is bound to the SMBIOS-derived Hardware ID and cannot be reused on another computer.
- License and clock-anchor files are protected by Windows DPAPI in the app data `license` directory.
- Every backend command compares the current UTC time with the protected last-seen time. A rollback beyond five minutes blocks the app until Windows time is corrected and a valid state is restored.
- Backups do not activate another installation because license state is machine-specific and excluded from the database backup payload.
