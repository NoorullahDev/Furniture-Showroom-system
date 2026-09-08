use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use uuid::Uuid;

use crate::error::AppError;

/// Argon2id memory-cost variant used for all password hashing.
const ARGON2_MEM_KIB: u32 = 19_456;
const ARGON2_TIME: u32 = 2;
const ARGON2_PAR: u32 = 1;
const ARGON2_OUTPUT_BYTES: Option<usize> = Some(32);

pub fn hash_password(password: &str) -> Result<String, AppError> {
    let params = argon2::Params::new(ARGON2_MEM_KIB, ARGON2_TIME, ARGON2_PAR, ARGON2_OUTPUT_BYTES)
        .map_err(|e| AppError::Internal(format!("argon2 params: {e}")))?;
    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);

    // 16 random bytes from the OS CSPRNG (via UUIDv4) encoded as a B64 salt.
    let salt = SaltString::encode_b64(&Uuid::new_v4().into_bytes())
        .map_err(|e| AppError::Internal(format!("salt generation failed: {e}")))?;
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| AppError::Internal(format!("password hash failed: {e}")))
}

pub fn verify_password(password: &str, stored_hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored_hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

/// Owner-account password rule from the Phase 0 decision record: minimum 8
/// characters containing at least one letter and one digit.
pub fn validate_strength(password: &str) -> Result<(), AppError> {
    if password.len() < 8 {
        return Err(AppError::Validation(
            "password must be at least 8 characters long".into(),
        ));
    }
    let has_letter = password.chars().any(|c| c.is_alphabetic());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    if !has_letter || !has_digit {
        return Err(AppError::Validation(
            "password must contain at least one letter and one digit".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_and_verifies_correctly() {
        let hash = hash_password("Correct Horse 123").unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(verify_password("Correct Horse 123", &hash));
        assert!(!verify_password("Wrong Horse 123", &hash));
    }

    #[test]
    fn distinct_salts_produce_distinct_hashes() {
        let a = hash_password("Same Pass 123").unwrap();
        let b = hash_password("Same Pass 123").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn malformed_hash_is_rejected() {
        assert!(!verify_password("anything", "not-a-valid-hash"));
    }

    #[test]
    fn strength_rules_enforced() {
        assert!(validate_strength("short1").is_err()); // 6 chars
        assert!(validate_strength("onlyletters").is_err());
        assert!(validate_strength("12345678").is_err());
        assert!(validate_strength("lettersand1").is_ok());
    }
}
