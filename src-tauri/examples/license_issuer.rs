//! Offline license issuer. This utility is intentionally separate from the
//! Tauri application and reads the private signing key from a caller-supplied
//! file. Never distribute the private PEM with an installer or customer PC.

use std::collections::HashMap;
use std::path::Path;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{Duration, Utc};
use rand::rngs::OsRng;
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs8::{
    DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey, LineEnding,
};
use rsa::pss::SigningKey;
use rsa::signature::{RandomizedSigner, SignatureEncoding};
use rsa::traits::PublicKeyParts;
use rsa::{RsaPrivateKey, RsaPublicKey};
use serde::Serialize;
use sha2::Sha256;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LicensePayload {
    version: u32,
    license_id: String,
    customer: String,
    hardware_id: String,
    issue_date: String,
    expiry_date: String,
    granted_days: u32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut raw = std::env::args().skip(1);
    let command = raw.next().unwrap_or_default();
    let args = parse_args(raw.collect())?;
    match command.as_str() {
        "generate-key" => generate_key(&args),
        "check-key" => check_key(&args),
        "issue" => issue(&args),
        _ => {
            eprintln!("Usage:\n  license_issuer generate-key --private <private.pem> --public <license_public.pem>\n  license_issuer check-key --private <private.pem> --public <license_public.pem>\n  license_issuer issue --private <private.pem> --customer <name> --hardware <ID> --days <30|90|365|custom> [--license-id <id>]");
            std::process::exit(2);
        }
    }
}

fn read_private(path: &str) -> Result<RsaPrivateKey, Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(path)?;
    Ok(RsaPrivateKey::from_pkcs8_pem(&text).or_else(|_| RsaPrivateKey::from_pkcs1_pem(&text))?)
}

fn check_key(args: &HashMap<String, String>) -> Result<(), Box<dyn std::error::Error>> {
    let private = read_private(required(args, "private")?)?;
    let public =
        RsaPublicKey::from_public_key_pem(&std::fs::read_to_string(required(args, "public")?)?)?;
    let derived = RsaPublicKey::from(&private);
    if derived.n() != public.n() || derived.e() != public.e() {
        return Err("private key does not match the application's public key".into());
    }
    println!(
        "OK: private key matches the application public key ({} bits)",
        public.size() * 8
    );
    Ok(())
}

fn parse_args(values: Vec<String>) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    if values.len() % 2 != 0 {
        return Err("every option must have a value".into());
    }
    let mut result = HashMap::new();
    for pair in values.chunks_exact(2) {
        let name = pair[0]
            .strip_prefix("--")
            .ok_or("options must start with --")?;
        result.insert(name.to_string(), pair[1].clone());
    }
    Ok(result)
}

fn required<'a>(
    args: &'a HashMap<String, String>,
    name: &str,
) -> Result<&'a str, Box<dyn std::error::Error>> {
    args.get(name)
        .map(String::as_str)
        .ok_or_else(|| format!("--{name} is required").into())
}

fn generate_key(args: &HashMap<String, String>) -> Result<(), Box<dyn std::error::Error>> {
    let private_path = Path::new(required(args, "private")?);
    let public_path = Path::new(required(args, "public")?);
    if private_path.exists() || public_path.exists() {
        return Err("refusing to overwrite an existing key file".into());
    }
    let private = RsaPrivateKey::new(&mut OsRng, 3072)?;
    let public = RsaPublicKey::from(&private);
    std::fs::write(
        private_path,
        private.to_pkcs8_pem(LineEnding::LF)?.as_bytes(),
    )?;
    std::fs::write(public_path, public.to_public_key_pem(LineEnding::LF)?)?;
    eprintln!(
        "Key pair created. Keep {} offline and rebuild the app after replacing {}.",
        private_path.display(),
        public_path.display()
    );
    Ok(())
}

fn issue(args: &HashMap<String, String>) -> Result<(), Box<dyn std::error::Error>> {
    let private = read_private(required(args, "private")?)?;
    let customer = required(args, "customer")?.trim();
    let hardware = required(args, "hardware")?.trim().to_ascii_uppercase();
    let days: u32 = required(args, "days")?.parse()?;
    if customer.is_empty() || customer.chars().count() > 120 {
        return Err("customer must be 1-120 characters".into());
    }
    if days == 0 || days > 3650 {
        return Err("days must be between 1 and 3650".into());
    }
    if hardware.len() != 19 || hardware.chars().filter(|c| *c == '-').count() != 3 {
        return Err("hardware must look like C1FA-3534-1C7C-1FE9".into());
    }
    let issue = Utc::now();
    let expiry = issue + Duration::days(days.into());
    let payload = LicensePayload {
        version: 1,
        license_id: args
            .get("license-id")
            .cloned()
            .unwrap_or_else(|| format!("LIC-{}", uuid::Uuid::now_v7())),
        customer: customer.to_string(),
        hardware_id: hardware,
        issue_date: issue.to_rfc3339(),
        expiry_date: expiry.to_rfc3339(),
        granted_days: days,
    };
    let payload_bytes = serde_json::to_vec(&payload)?;
    let signing_key = SigningKey::<Sha256>::new(private);
    let signature = signing_key.sign_with_rng(&mut OsRng, &payload_bytes);
    println!(
        "FSLIC1.{}.{}",
        URL_SAFE_NO_PAD.encode(payload_bytes),
        URL_SAFE_NO_PAD.encode(signature.to_bytes())
    );
    Ok(())
}
