use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use rsa::pkcs8::DecodePublicKey;
use rsa::pss::{Signature, VerifyingKey};
use rsa::signature::Verifier;
use rsa::RsaPublicKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::dto::LicenseStatusDto;
use crate::error::AppError;
use crate::infrastructure::{machine_id, protected_store, FilePaths};
use crate::state::AppState;

const FORMAT_PREFIX: &str = "FSLIC1";
const PUBLIC_KEY_PEM: &str = include_str!("../../keys/license_public.pem");
const CLOCK_TOLERANCE_MINUTES: i64 = 5;
const CLOCK_WRITE_INTERVAL_SECONDS: i64 = 30;
const RECOVERY_LICENSE_MAX_AGE_HOURS: i64 = 24;

static RUNTIME: OnceLock<LicenseRuntime> = OnceLock::new();

#[derive(Debug)]
struct LicenseRuntime {
    license_dir: PathBuf,
    hardware_id: String,
    gate_lock: Mutex<()>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LicensePayload {
    pub version: u32,
    pub license_id: String,
    pub customer: String,
    pub hardware_id: String,
    pub issue_date: String,
    pub expiry_date: String,
    pub granted_days: u32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ClockAnchor {
    last_seen_utc: String,
    license_digest: String,
}

struct VerifiedLicense {
    payload: LicensePayload,
    issue: DateTime<Utc>,
    expiry: DateTime<Utc>,
    token_digest: String,
    masked_key: String,
}

pub fn initialize(paths: &FilePaths) -> Result<(), AppError> {
    let runtime = LicenseRuntime {
        license_dir: paths.license_dir.clone(),
        hardware_id: machine_id::hardware_id()?,
        gate_lock: Mutex::new(()),
    };
    RUNTIME
        .set(runtime)
        .map_err(|_| AppError::Internal("license verifier was initialized twice".into()))
}

pub fn ensure_command_allowed(command: &str) -> Result<(), AppError> {
    if matches!(command, "license_status" | "license_activate") {
        return Ok(());
    }
    let runtime = RUNTIME
        .get()
        .ok_or_else(|| AppError::License("license verifier is not initialized".into()))?;
    let _guard = runtime
        .gate_lock
        .lock()
        .map_err(|_| AppError::Internal("license verifier lock is poisoned".into()))?;
    let current = evaluate(&runtime.license_dir, &runtime.hardware_id, Utc::now(), true)?;
    if current.is_activated {
        Ok(())
    } else {
        Err(AppError::License(current.label))
    }
}

pub fn status(state: &AppState) -> Result<LicenseStatusDto, AppError> {
    if let Some(runtime) = RUNTIME.get() {
        let _guard = runtime
            .gate_lock
            .lock()
            .map_err(|_| AppError::Internal("license verifier lock is poisoned".into()))?;
        evaluate(
            &state.paths.license_dir,
            &runtime.hardware_id,
            Utc::now(),
            true,
        )
    } else {
        evaluate(
            &state.paths.license_dir,
            &machine_id::hardware_id()?,
            Utc::now(),
            true,
        )
    }
}

pub fn activate(state: &AppState, license_key: &str) -> Result<LicenseStatusDto, AppError> {
    let key = license_key.trim();
    if key.len() > 16_384 {
        return Err(AppError::Validation("license key is too long".into()));
    }
    let runtime_guard = if let Some(runtime) = RUNTIME.get() {
        Some(
            runtime
                .gate_lock
                .lock()
                .map_err(|_| AppError::Internal("license verifier lock is poisoned".into()))?,
        )
    } else {
        None
    };
    let hardware_id = match RUNTIME.get() {
        Some(runtime) => runtime.hardware_id.clone(),
        None => machine_id::hardware_id()?,
    };
    let now = Utc::now();
    let verified = verify_token(key, &hardware_id).map_err(|failure| match failure {
        VerifyFailure::HardwareMismatch => {
            AppError::License("This license was issued for another computer.".into())
        }
        VerifyFailure::Invalid(message) => AppError::Validation(message),
        VerifyFailure::Tampered => AppError::License(
            "The license signature is invalid or the license was tampered with.".into(),
        ),
    })?;
    validate_dates(&verified, now)?;

    // A renewal cannot erase evidence that the local clock was moved back.
    match read_anchor(&state.paths.license_dir) {
        Ok(Some(anchor)) => {
            let last_seen = parse_utc(&anchor.last_seen_utc, "clock anchor")?;
            if now + Duration::minutes(CLOCK_TOLERANCE_MINUTES) < last_seen {
                return Err(AppError::License(
                    "Clock rollback detected. Correct the Windows date and time before renewing."
                        .into(),
                ));
            }
        }
        Ok(None) | Err(_) if license_path(&state.paths.license_dir).exists() => {
            let same_key = read_installed_key(&state.paths.license_dir)
                .is_some_and(|installed| installed.trim() == key);
            if recovery_requires_new_license(same_key, verified.issue, now) {
                return Err(AppError::License(
                    "The protected clock record is missing or damaged. A newly issued renewal license is required."
                        .into(),
                ));
            }
        }
        _ => {}
    }

    std::fs::create_dir_all(&state.paths.license_dir)?;
    let protected_key = protected_store::protect(key.as_bytes())?;
    atomic_write(&license_path(&state.paths.license_dir), &protected_key)?;
    write_anchor(
        &state.paths.license_dir,
        &ClockAnchor {
            last_seen_utc: now.to_rfc3339(),
            license_digest: verified.token_digest,
        },
    )?;
    let renewed = protected_store::protect(now.to_rfc3339().as_bytes())?;
    atomic_write(&renewed_path(&state.paths.license_dir), &renewed)?;

    let result = evaluate(&state.paths.license_dir, &hardware_id, now, false);
    drop(runtime_guard);
    result
}

fn evaluate(
    directory: &Path,
    hardware_id: &str,
    now: DateTime<Utc>,
    update_clock: bool,
) -> Result<LicenseStatusDto, AppError> {
    evaluate_with_public_key(directory, hardware_id, now, update_clock, PUBLIC_KEY_PEM)
}

fn evaluate_with_public_key(
    directory: &Path,
    hardware_id: &str,
    now: DateTime<Utc>,
    update_clock: bool,
    public_key_pem: &str,
) -> Result<LicenseStatusDto, AppError> {
    let path = license_path(directory);
    if !path.exists() {
        return Ok(inactive_status(
            "missing",
            "License required",
            "No license is installed on this computer.",
            hardware_id,
        ));
    }

    let protected = match std::fs::read(&path) {
        Ok(value) => value,
        Err(_) => {
            return Ok(inactive_status(
                "invalid",
                "Invalid license",
                "The installed license cannot be read.",
                hardware_id,
            ))
        }
    };
    let token = match protected_store::unprotect(&protected).and_then(|bytes| {
        String::from_utf8(bytes)
            .map_err(|_| AppError::Validation("stored license is not valid text".into()))
    }) {
        Ok(value) => value,
        Err(_) => {
            return Ok(inactive_status(
                "tampered",
                "Tampered license",
                "The installed license was modified or copied from another computer.",
                hardware_id,
            ))
        }
    };

    let verified = match verify_token_with_public_key(&token, hardware_id, public_key_pem) {
        Ok(value) => value,
        Err(VerifyFailure::HardwareMismatch) => {
            return Ok(inactive_status(
                "wrong_hardware",
                "License is for another computer",
                "Request a license for the Hardware ID shown below.",
                hardware_id,
            ))
        }
        Err(VerifyFailure::Invalid(message)) => {
            return Ok(inactive_status(
                "invalid",
                "Invalid license",
                &message,
                hardware_id,
            ))
        }
        Err(VerifyFailure::Tampered) => {
            return Ok(inactive_status(
                "tampered",
                "Tampered license",
                "The license signature is not valid.",
                hardware_id,
            ))
        }
    };

    if let Err(error) = validate_dates(&verified, now) {
        let message = error.to_string();
        let status = if now > verified.expiry {
            "expired"
        } else {
            "clock_rollback"
        };
        let label = if status == "expired" {
            "License expired"
        } else {
            "Clock rollback detected"
        };
        return Ok(payload_status(
            &verified,
            status,
            label,
            &message,
            hardware_id,
            directory,
            now,
        ));
    }

    let anchor = match read_anchor(directory) {
        Ok(Some(value)) => value,
        Ok(None) => {
            return Ok(payload_status(
                &verified,
                "tampered",
                "License state is incomplete",
                "The protected clock record is missing. Re-enter a newly issued license.",
                hardware_id,
                directory,
                now,
            ))
        }
        Err(_) => {
            return Ok(payload_status(
                &verified,
                "tampered",
                "License state is damaged",
                "The protected clock record could not be verified.",
                hardware_id,
                directory,
                now,
            ))
        }
    };
    let last_seen = match parse_utc(&anchor.last_seen_utc, "clock anchor") {
        Ok(value) => value,
        Err(_) => {
            return Ok(payload_status(
                &verified,
                "tampered",
                "License state is damaged",
                "The protected clock record contains invalid data.",
                hardware_id,
                directory,
                now,
            ))
        }
    };
    if anchor.license_digest != verified.token_digest {
        return Ok(payload_status(
            &verified,
            "tampered",
            "License state does not match",
            "The installed license and its protected clock record do not match.",
            hardware_id,
            directory,
            now,
        ));
    }
    if now + Duration::minutes(CLOCK_TOLERANCE_MINUTES) < last_seen {
        return Ok(payload_status(
            &verified,
            "clock_rollback",
            "Clock rollback detected",
            "Windows time is earlier than the last verified application time.",
            hardware_id,
            directory,
            now,
        ));
    }
    if update_clock && now - last_seen >= Duration::seconds(CLOCK_WRITE_INTERVAL_SECONDS) {
        write_anchor(
            directory,
            &ClockAnchor {
                last_seen_utc: now.to_rfc3339(),
                license_digest: verified.token_digest.clone(),
            },
        )?;
    }

    Ok(payload_status(
        &verified,
        "active",
        "Active",
        "This computer has a valid offline license.",
        hardware_id,
        directory,
        now,
    ))
}

#[derive(Debug)]
enum VerifyFailure {
    HardwareMismatch,
    Invalid(String),
    Tampered,
}

fn verify_token(token: &str, expected_hardware_id: &str) -> Result<VerifiedLicense, VerifyFailure> {
    verify_token_with_public_key(token, expected_hardware_id, PUBLIC_KEY_PEM)
}

fn verify_token_with_public_key(
    token: &str,
    expected_hardware_id: &str,
    public_key_pem: &str,
) -> Result<VerifiedLicense, VerifyFailure> {
    let mut parts = token.split('.');
    if parts.next() != Some(FORMAT_PREFIX) {
        return Err(VerifyFailure::Invalid(
            "License format is not supported.".into(),
        ));
    }
    let payload_part = parts
        .next()
        .ok_or_else(|| VerifyFailure::Invalid("License payload is missing.".into()))?;
    let signature_part = parts
        .next()
        .ok_or_else(|| VerifyFailure::Invalid("License signature is missing.".into()))?;
    if parts.next().is_some() {
        return Err(VerifyFailure::Invalid(
            "License format is not supported.".into(),
        ));
    }
    let payload_bytes = URL_SAFE_NO_PAD
        .decode(payload_part)
        .map_err(|_| VerifyFailure::Invalid("License payload encoding is invalid.".into()))?;
    let signature_bytes = URL_SAFE_NO_PAD
        .decode(signature_part)
        .map_err(|_| VerifyFailure::Invalid("License signature encoding is invalid.".into()))?;
    let public_key = RsaPublicKey::from_public_key_pem(public_key_pem)
        .map_err(|_| VerifyFailure::Invalid("Application verification key is invalid.".into()))?;
    let verifying_key = VerifyingKey::<Sha256>::new(public_key);
    let signature =
        Signature::try_from(signature_bytes.as_slice()).map_err(|_| VerifyFailure::Tampered)?;
    verifying_key
        .verify(&payload_bytes, &signature)
        .map_err(|_| VerifyFailure::Tampered)?;

    let payload: LicensePayload = serde_json::from_slice(&payload_bytes)
        .map_err(|_| VerifyFailure::Invalid("Signed license data is invalid.".into()))?;
    if payload.version != 1
        || payload.license_id.trim().is_empty()
        || payload.customer.trim().is_empty()
        || payload.granted_days == 0
    {
        return Err(VerifyFailure::Invalid(
            "Signed license fields are invalid.".into(),
        ));
    }
    if !payload
        .hardware_id
        .eq_ignore_ascii_case(expected_hardware_id)
    {
        return Err(VerifyFailure::HardwareMismatch);
    }
    let issue = parse_utc(&payload.issue_date, "issue date")
        .map_err(|error| VerifyFailure::Invalid(error.to_string()))?;
    let expiry = parse_utc(&payload.expiry_date, "expiry date")
        .map_err(|error| VerifyFailure::Invalid(error.to_string()))?;
    if expiry <= issue {
        return Err(VerifyFailure::Invalid(
            "License expiry must be after its issue date.".into(),
        ));
    }
    let digest = Sha256::digest(token.as_bytes());
    let token_digest = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    let suffix: String = token
        .chars()
        .rev()
        .take(8)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    Ok(VerifiedLicense {
        payload,
        issue,
        expiry,
        token_digest,
        masked_key: format!("FSLIC1-••••-••••-{suffix}"),
    })
}

fn validate_dates(license: &VerifiedLicense, now: DateTime<Utc>) -> Result<(), AppError> {
    if now + Duration::minutes(CLOCK_TOLERANCE_MINUTES) < license.issue {
        return Err(AppError::License(
            "The license issue date is in the future. Check the Windows date and time.".into(),
        ));
    }
    if now > license.expiry {
        return Err(AppError::License(
            "The license has expired. Enter a renewed license to continue.".into(),
        ));
    }
    Ok(())
}

fn recovery_requires_new_license(same_key: bool, issue: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    same_key || issue < now - Duration::hours(RECOVERY_LICENSE_MAX_AGE_HOURS)
}

fn parse_utc(value: &str, field: &str) -> Result<DateTime<Utc>, AppError> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| AppError::Validation(format!("license {field} is invalid")))
}

fn inactive_status(
    status: &str,
    label: &str,
    message: &str,
    hardware_id: &str,
) -> LicenseStatusDto {
    LicenseStatusDto {
        status: status.into(),
        label: label.into(),
        message: message.into(),
        is_activated: false,
        hardware_id: hardware_id.into(),
        license_id: None,
        customer: None,
        issue_date: None,
        last_renewed: None,
        granted_days: None,
        expires_at: None,
        days_remaining: 0,
        validity_percent: 0,
        masked_key: None,
    }
}

fn payload_status(
    license: &VerifiedLicense,
    status: &str,
    label: &str,
    message: &str,
    hardware_id: &str,
    directory: &Path,
    now: DateTime<Utc>,
) -> LicenseStatusDto {
    let seconds_remaining = (license.expiry - now).num_seconds().max(0);
    let days_remaining = if seconds_remaining == 0 {
        0
    } else {
        ((seconds_remaining + 86_399) / 86_400) as u32
    };
    let validity_percent =
        ((seconds_remaining as f64 / (license.payload.granted_days as f64 * 86_400.0)) * 100.0)
            .clamp(0.0, 100.0)
            .round() as u8;
    LicenseStatusDto {
        status: status.into(),
        label: label.into(),
        message: message.into(),
        is_activated: status == "active",
        hardware_id: hardware_id.into(),
        license_id: Some(license.payload.license_id.clone()),
        customer: Some(license.payload.customer.clone()),
        issue_date: Some(license.payload.issue_date.clone()),
        last_renewed: read_renewed(directory),
        granted_days: Some(license.payload.granted_days),
        expires_at: Some(license.payload.expiry_date.clone()),
        days_remaining,
        validity_percent,
        masked_key: Some(license.masked_key.clone()),
    }
}

fn license_path(directory: &Path) -> PathBuf {
    directory.join("license.dat")
}
fn anchor_path(directory: &Path) -> PathBuf {
    directory.join("clock.dat")
}
fn renewed_path(directory: &Path) -> PathBuf {
    directory.join("renewed.dat")
}

fn read_anchor(directory: &Path) -> Result<Option<ClockAnchor>, AppError> {
    let path = anchor_path(directory);
    if !path.exists() {
        return Ok(None);
    }
    let plain = protected_store::unprotect(&std::fs::read(path)?)?;
    serde_json::from_slice(&plain)
        .map(Some)
        .map_err(|_| AppError::Validation("protected clock record is invalid".into()))
}

fn write_anchor(directory: &Path, anchor: &ClockAnchor) -> Result<(), AppError> {
    let json = serde_json::to_vec(anchor)
        .map_err(|error| AppError::Internal(format!("encode clock record: {error}")))?;
    let protected = protected_store::protect(&json)?;
    atomic_write(&anchor_path(directory), &protected)
}

fn read_renewed(directory: &Path) -> Option<String> {
    let bytes = std::fs::read(renewed_path(directory)).ok()?;
    let plain = protected_store::unprotect(&bytes).ok()?;
    String::from_utf8(plain).ok()
}

fn read_installed_key(directory: &Path) -> Option<String> {
    let bytes = std::fs::read(license_path(directory)).ok()?;
    let plain = protected_store::unprotect(&bytes).ok()?;
    String::from_utf8(plain).ok()
}

fn atomic_write(path: &Path, data: &[u8]) -> Result<(), AppError> {
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::write(&temporary, data)?;
    replace_file(&temporary, path)?;
    Ok(())
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> Result<(), AppError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let source_wide: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination_wide: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let moved = unsafe {
        MoveFileExW(
            source_wide.as_ptr(),
            destination_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        let error = std::io::Error::last_os_error();
        let _ = std::fs::remove_file(source);
        return Err(AppError::Io(error));
    }
    Ok(())
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> Result<(), AppError> {
    if destination.exists() {
        std::fs::remove_file(destination)?;
    }
    std::fs::rename(source, destination)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;
    use rsa::pkcs8::{EncodePublicKey, LineEnding};
    use rsa::pss::SigningKey;
    use rsa::signature::{RandomizedSigner, SignatureEncoding};
    use rsa::{RsaPrivateKey, RsaPublicKey};

    fn signed(
        private: &RsaPrivateKey,
        hardware: &str,
        issue: DateTime<Utc>,
        expiry: DateTime<Utc>,
    ) -> String {
        let payload = LicensePayload {
            version: 1,
            license_id: "LIC-TEST-1".into(),
            customer: "Test Showroom".into(),
            hardware_id: hardware.into(),
            issue_date: issue.to_rfc3339(),
            expiry_date: expiry.to_rfc3339(),
            granted_days: 30,
        };
        let bytes = serde_json::to_vec(&payload).unwrap();
        let key = SigningKey::<Sha256>::new(private.clone());
        let signature = key.sign_with_rng(&mut OsRng, &bytes);
        format!(
            "{FORMAT_PREFIX}.{}.{}",
            URL_SAFE_NO_PAD.encode(bytes),
            URL_SAFE_NO_PAD.encode(signature.to_bytes())
        )
    }

    #[test]
    fn invalid_and_tampered_tokens_fail_closed() {
        assert!(matches!(
            verify_token("bad", "AAAA"),
            Err(VerifyFailure::Invalid(_))
        ));
        assert!(matches!(
            verify_token("FSLIC1.e30.invalid", "AAAA"),
            Err(VerifyFailure::Invalid(_)) | Err(VerifyFailure::Tampered)
        ));
    }

    #[test]
    fn date_rules_detect_expiry_and_future_issue() {
        let verified = VerifiedLicense {
            payload: LicensePayload {
                version: 1,
                license_id: "x".into(),
                customer: "x".into(),
                hardware_id: "x".into(),
                issue_date: "x".into(),
                expiry_date: "x".into(),
                granted_days: 30,
            },
            issue: Utc::now() - Duration::days(31),
            expiry: Utc::now() - Duration::days(1),
            token_digest: "x".into(),
            masked_key: "x".into(),
        };
        assert!(validate_dates(&verified, Utc::now()).is_err());
        let future = VerifiedLicense {
            issue: Utc::now() + Duration::days(1),
            expiry: Utc::now() + Duration::days(31),
            ..verified
        };
        assert!(validate_dates(&future, Utc::now()).is_err());
    }

    #[test]
    fn valid_signature_hardware_binding_and_tamper_are_enforced() {
        let private = RsaPrivateKey::new(&mut OsRng, 2048).unwrap();
        let public = RsaPublicKey::from(&private)
            .to_public_key_pem(LineEnding::LF)
            .unwrap();
        let now = Utc::now();
        let token = signed(
            &private,
            "AAAA-BBBB-CCCC-DDDD",
            now,
            now + Duration::days(30),
        );
        let valid = verify_token_with_public_key(&token, "AAAA-BBBB-CCCC-DDDD", &public).unwrap();
        assert_eq!(valid.payload.customer, "Test Showroom");
        assert!(matches!(
            verify_token_with_public_key(&token, "1111-2222-3333-4444", &public),
            Err(VerifyFailure::HardwareMismatch)
        ));
        let mut tampered = token.into_bytes();
        tampered[8] = if tampered[8] == b'A' { b'B' } else { b'A' };
        assert!(matches!(
            verify_token_with_public_key(
                &String::from_utf8(tampered).unwrap(),
                "AAAA-BBBB-CCCC-DDDD",
                &public
            ),
            Err(VerifyFailure::Tampered)
        ));
    }

    #[test]
    fn rollback_comparison_allows_small_clock_correction_only() {
        let last_seen = Utc::now();
        assert!(
            last_seen - Duration::minutes(4) + Duration::minutes(CLOCK_TOLERANCE_MINUTES)
                >= last_seen
        );
        assert!(
            last_seen - Duration::minutes(6) + Duration::minutes(CLOCK_TOLERANCE_MINUTES)
                < last_seen
        );
    }

    #[test]
    fn damaged_clock_state_cannot_be_reset_with_the_same_or_an_old_key() {
        let now = Utc::now();
        assert!(recovery_requires_new_license(
            false,
            now - Duration::days(2),
            now
        ));
        assert!(recovery_requires_new_license(true, now, now));
        assert!(!recovery_requires_new_license(
            false,
            now - Duration::minutes(5),
            now
        ));
    }

    fn test_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "furniture-license-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn install_for_test(
        dir: &Path,
        token: &str,
        public: &str,
        hardware: &str,
        seen: DateTime<Utc>,
    ) {
        let verified = verify_token_with_public_key(token, hardware, public).unwrap();
        atomic_write(
            &license_path(dir),
            &protected_store::protect(token.as_bytes()).unwrap(),
        )
        .unwrap();
        write_anchor(
            dir,
            &ClockAnchor {
                last_seen_utc: seen.to_rfc3339(),
                license_digest: verified.token_digest,
            },
        )
        .unwrap();
        atomic_write(
            &renewed_path(dir),
            &protected_store::protect(seen.to_rfc3339().as_bytes()).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn lifecycle_scenarios_fail_closed_and_survive_restart() {
        let dir = test_dir("lifecycle");
        let hardware = "AAAA-BBBB-CCCC-DDDD";
        let now = Utc::now();
        let private = RsaPrivateKey::new(&mut OsRng, 2048).unwrap();
        let public = RsaPublicKey::from(&private)
            .to_public_key_pem(LineEnding::LF)
            .unwrap();

        let missing = evaluate_with_public_key(&dir, hardware, now, true, &public).unwrap();
        assert_eq!(missing.status, "missing");
        assert!(!missing.is_activated);

        let token = signed(
            &private,
            hardware,
            now - Duration::minutes(1),
            now + Duration::days(30),
        );
        install_for_test(&dir, &token, &public, hardware, now);
        let active = evaluate_with_public_key(&dir, hardware, now, true, &public).unwrap();
        assert_eq!(active.status, "active");
        assert!(active.is_activated);
        let restarted =
            evaluate_with_public_key(&dir, hardware, now + Duration::minutes(1), true, &public)
                .unwrap();
        assert_eq!(restarted.status, "active");

        let copied =
            evaluate_with_public_key(&dir, "1111-2222-3333-4444", now, true, &public).unwrap();
        assert_eq!(copied.status, "wrong_hardware");

        write_anchor(
            &dir,
            &ClockAnchor {
                last_seen_utc: (now + Duration::hours(1)).to_rfc3339(),
                license_digest: verify_token_with_public_key(&token, hardware, &public)
                    .unwrap()
                    .token_digest,
            },
        )
        .unwrap();
        let rollback = evaluate_with_public_key(&dir, hardware, now, true, &public).unwrap();
        assert_eq!(rollback.status, "clock_rollback");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn expired_tampered_and_valid_renewal_scenarios() {
        let dir = test_dir("renewal");
        let hardware = "AAAA-BBBB-CCCC-DDDD";
        let now = Utc::now();
        let private = RsaPrivateKey::new(&mut OsRng, 2048).unwrap();
        let public = RsaPublicKey::from(&private)
            .to_public_key_pem(LineEnding::LF)
            .unwrap();

        let expired_token = signed(
            &private,
            hardware,
            now - Duration::days(31),
            now - Duration::days(1),
        );
        install_for_test(
            &dir,
            &expired_token,
            &public,
            hardware,
            now - Duration::days(2),
        );
        assert_eq!(
            evaluate_with_public_key(&dir, hardware, now, true, &public)
                .unwrap()
                .status,
            "expired"
        );

        let renewal = signed(
            &private,
            hardware,
            now - Duration::minutes(1),
            now + Duration::days(90),
        );
        install_for_test(&dir, &renewal, &public, hardware, now);
        assert_eq!(
            evaluate_with_public_key(&dir, hardware, now, true, &public)
                .unwrap()
                .status,
            "active"
        );

        let mut changed = renewal.into_bytes();
        changed[8] = if changed[8] == b'A' { b'B' } else { b'A' };
        atomic_write(
            &license_path(&dir),
            &protected_store::protect(&changed).unwrap(),
        )
        .unwrap();
        assert_eq!(
            evaluate_with_public_key(&dir, hardware, now, true, &public)
                .unwrap()
                .status,
            "tampered"
        );

        let _ = std::fs::remove_dir_all(dir);
    }
}
