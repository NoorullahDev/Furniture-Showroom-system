use crate::application::auth::Principal;
use crate::error::AppError;
use crate::infrastructure::clock::Clock;
use crate::infrastructure::AuditInput;
use crate::repositories::SettingsRepository;
use crate::state::AppState;

const LOGO_FILE: &str = "shop-logo.webp";

#[derive(Debug, Clone)]
pub struct GeneralSettingsInput {
    pub shop_name: String,
    pub owner_name: String,
    pub address: String,
    pub phone: String,
    pub currency: String,
}

#[derive(Debug, Clone)]
pub struct PrintSettingsInput {
    pub paper_size: String,
    pub orientation: String,
    pub margin_mm: i64,
    pub font_size: String,
    pub copies: i64,
    pub show_logo: bool,
    pub show_address: bool,
    pub show_phone: bool,
    pub show_payment_details: bool,
    pub footer_text: String,
    pub printer_destination: String,
}

/// Read one settings value as raw JSON text.
pub async fn get(state: &AppState, key: &str) -> Result<Option<String>, AppError> {
    SettingsRepository::find(&state.pool, key).await
}

/// Authorized settings write used by commands: requires `settings.manage` and
/// records the change in the audit trail.
#[allow(clippy::too_many_arguments)]
pub async fn set_authorized(
    state: &AppState,
    principal: &Principal,
    key: &str,
    value_json: &str,
    correlation_id: &str,
) -> Result<(), AppError> {
    principal.require("settings.manage")?;
    let before = SettingsRepository::find(&state.pool, key).await?;
    let key_owned = key.to_owned();
    let value_owned = value_json.to_owned();
    let now = state.clock.now_iso();
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                SettingsRepository::upsert(
                    &mut *tx,
                    &key_owned,
                    &value_owned,
                    Some(actor_id),
                    &now,
                )
                .await?;
                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session),
                            action: "settings.update".into(),
                            entity_type: Some("setting".into()),
                            entity_id: Some(key_owned.clone()),
                            before_json: before,
                            after_json: Some(value_owned.clone()),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;
                Ok(())
            })
        })
        .await
}

pub async fn update_general(
    state: &AppState,
    principal: &Principal,
    input: GeneralSettingsInput,
    correlation_id: &str,
) -> Result<(), AppError> {
    principal.require("settings.manage")?;
    let shop_name = input.shop_name.trim().to_string();
    let owner_name = input.owner_name.trim().to_string();
    let address = input.address.trim().to_string();
    let phone = input.phone.trim().to_string();
    let currency = if input.currency.trim().is_empty() {
        "PKR".to_string()
    } else {
        input.currency.trim().to_uppercase()
    };
    if shop_name.is_empty() || shop_name.chars().count() > 100 {
        return Err(AppError::Validation(
            "showroom name is required and must be 100 characters or fewer".into(),
        ));
    }
    if owner_name.chars().count() > 200 {
        return Err(AppError::Validation(
            "owner name must be 200 characters or fewer".into(),
        ));
    }
    if address.chars().count() > 500 || phone.chars().count() > 50 {
        return Err(AppError::Validation(
            "address or phone number is too long".into(),
        ));
    }
    if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_uppercase()) {
        return Err(AppError::Validation(
            "currency must be a three-letter code such as PKR".into(),
        ));
    }

    let current_currency = get(state, "shop.currency")
        .await?
        .and_then(|value| serde_json::from_str::<String>(&value).ok())
        .unwrap_or_else(|| "PKR".into());
    if current_currency != currency {
        let financial_rows: i64 = sqlx::query_scalar(
            "SELECT
                (SELECT COUNT(*) FROM sales WHERE status IN ('confirmed', 'cancelled')) +
                (SELECT COUNT(*) FROM purchases WHERE status IN ('posted', 'reversed')) +
                (SELECT COUNT(*) FROM cash_entries)",
        )
        .fetch_one(&state.pool)
        .await?;
        if financial_rows > 0 {
            return Err(AppError::Conflict(
                "currency cannot be changed after financial activity has been recorded".into(),
            ));
        }
    }

    let showroom_name = shop_name.clone();
    let values = vec![
        ("shop.name", serde_json::json!(shop_name)),
        ("shop.owner_name", serde_json::json!(owner_name)),
        ("shop.address", serde_json::json!(address)),
        ("shop.phone", serde_json::json!(phone)),
        ("shop.currency", serde_json::json!(currency)),
    ];
    update_group(
        state,
        principal,
        values,
        "settings.general_update",
        correlation_id,
        Some(showroom_name),
    )
    .await
}

pub async fn update_print(
    state: &AppState,
    principal: &Principal,
    input: PrintSettingsInput,
    correlation_id: &str,
) -> Result<(), AppError> {
    principal.require("settings.manage")?;
    if input.paper_size != "a4" {
        return Err(AppError::Validation(
            "paper size must be A4 (210 x 297 mm)".into(),
        ));
    }
    if !matches!(input.orientation.as_str(), "portrait" | "landscape") {
        return Err(AppError::Validation("unsupported page orientation".into()));
    }
    if !matches!(input.font_size.as_str(), "small" | "normal" | "large") {
        return Err(AppError::Validation("unsupported print font size".into()));
    }
    if !(5..=40).contains(&input.margin_mm) || !(1..=10).contains(&input.copies) {
        return Err(AppError::Validation(
            "margins must be 5-40 mm and copies must be 1-10".into(),
        ));
    }
    if input.footer_text.chars().count() > 300 {
        return Err(AppError::Validation(
            "footer message must be 300 characters or fewer".into(),
        ));
    }
    let destination_ok = matches!(
        input.printer_destination.as_str(),
        "print-window" | "system-default"
    ) || input
        .printer_destination
        .strip_prefix("printer:")
        .is_some_and(|name| !name.trim().is_empty() && name.chars().count() <= 200);
    if !destination_ok {
        return Err(AppError::Validation("invalid printer destination".into()));
    }

    let values = vec![
        ("print.paper_size", serde_json::json!(input.paper_size)),
        ("print.orientation", serde_json::json!(input.orientation)),
        ("print.margin_mm", serde_json::json!(input.margin_mm)),
        ("print.font_size", serde_json::json!(input.font_size)),
        ("print.copies", serde_json::json!(input.copies)),
        ("print.show_logo", serde_json::json!(input.show_logo)),
        ("print.show_address", serde_json::json!(input.show_address)),
        ("print.show_phone", serde_json::json!(input.show_phone)),
        (
            "print.show_payment_details",
            serde_json::json!(input.show_payment_details),
        ),
        ("print.footer_text", serde_json::json!(input.footer_text)),
        (
            "print.printer_destination",
            serde_json::json!(input.printer_destination),
        ),
    ];
    update_group(
        state,
        principal,
        values,
        "settings.print_update",
        correlation_id,
        None,
    )
    .await
}

async fn update_group(
    state: &AppState,
    principal: &Principal,
    values: Vec<(&'static str, serde_json::Value)>,
    action: &'static str,
    correlation_id: &str,
    showroom_name: Option<String>,
) -> Result<(), AppError> {
    let now = state.clock.now_iso();
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let audits = state.audits.clone();
    let correlation = correlation_id.to_string();
    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                if let Some(name) = showroom_name {
                    let active_locations: i64 =
                        sqlx::query_scalar("SELECT COUNT(*) FROM locations WHERE is_active = 1")
                            .fetch_one(&mut *tx)
                            .await?;
                    if active_locations != 1 {
                        return Err(AppError::Conflict(
                            "the showroom location consolidation has not completed".into(),
                        ));
                    }
                    let location_id: i64 = sqlx::query_scalar(
                        "SELECT id FROM locations WHERE is_active = 1 ORDER BY id LIMIT 1",
                    )
                    .fetch_one(&mut *tx)
                    .await?;
                    sqlx::query(
                        "UPDATE locations
                            SET name = '__retired_location_' || id, updated_at = ?
                          WHERE id <> ? AND name = ?",
                    )
                    .bind(&now)
                    .bind(location_id)
                    .bind(&name)
                    .execute(&mut *tx)
                    .await?;
                    sqlx::query(
                        "UPDATE locations
                            SET name = ?, type = 'showroom', updated_at = ?
                          WHERE id = ?",
                    )
                    .bind(&name)
                    .bind(&now)
                    .bind(location_id)
                    .execute(&mut *tx)
                    .await?;
                }
                let changed_keys: Vec<&str> = values.iter().map(|(key, _)| *key).collect();
                for (key, value) in &values {
                    SettingsRepository::upsert(
                        &mut *tx,
                        key,
                        &value.to_string(),
                        Some(actor_id),
                        &now,
                    )
                    .await?;
                }
                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session),
                            action: action.into(),
                            entity_type: Some("settings".into()),
                            after_json: Some(
                                serde_json::json!({ "keys": changed_keys }).to_string(),
                            ),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;
                Ok(())
            })
        })
        .await
}

pub async fn logo_data(state: &AppState) -> Result<Option<String>, AppError> {
    let path = state.paths.branding_dir.join(LOGO_FILE);
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = std::fs::read(path)?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(AppError::Image("stored shop logo is too large".into()));
    }
    Ok(Some(format!(
        "data:image/webp;base64,{}",
        crate::infrastructure::base64_encode(&bytes)
    )))
}

pub async fn replace_logo(
    state: &AppState,
    principal: &Principal,
    source_path: &str,
    correlation_id: &str,
) -> Result<String, AppError> {
    principal.require("settings.manage")?;
    let imported = crate::infrastructure::import_image(
        std::path::Path::new(source_path),
        &state.paths.branding_dir,
    )?;
    let staged = state.paths.branding_dir.join(&imported.stored_name);
    let thumbnail = state.paths.branding_dir.join(&imported.thumbnail_name);
    let destination = state.paths.branding_dir.join(LOGO_FILE);
    let _ = std::fs::remove_file(&thumbnail);
    if destination.exists() {
        std::fs::remove_file(&destination)?;
    }
    std::fs::rename(&staged, &destination)?;

    state
        .audits
        .record_pool(AuditInput {
            user_id: Some(principal.user_id),
            session_id: Some(principal.session_id.clone()),
            action: "settings.logo_replace".into(),
            entity_type: Some("settings".into()),
            entity_id: Some("shop.logo".into()),
            after_json: Some(
                serde_json::json!({
                    "width": imported.width,
                    "height": imported.height,
                    "sha256": imported.sha256
                })
                .to_string(),
            ),
            correlation_id: Some(correlation_id.to_string()),
            ..Default::default()
        })
        .await?;
    logo_data(state)
        .await?
        .ok_or_else(|| AppError::Internal("saved logo could not be read".into()))
}

pub async fn remove_logo(
    state: &AppState,
    principal: &Principal,
    correlation_id: &str,
) -> Result<(), AppError> {
    principal.require("settings.manage")?;
    let path = state.paths.branding_dir.join(LOGO_FILE);
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    state
        .audits
        .record_pool(AuditInput {
            user_id: Some(principal.user_id),
            session_id: Some(principal.session_id.clone()),
            action: "settings.logo_remove".into(),
            entity_type: Some("settings".into()),
            entity_id: Some("shop.logo".into()),
            correlation_id: Some(correlation_id.to_string()),
            ..Default::default()
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::password::hash_password;

    async fn setup() -> (std::path::PathBuf, AppState, Principal) {
        let dir =
            std::env::temp_dir().join(format!("furniture-shop-settings-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let paths = crate::infrastructure::FilePaths::init(&dir).unwrap();
        let (pool, _) = crate::infrastructure::db::open(&paths).await.unwrap();
        let state = AppState::new(pool, paths);
        let hash = hash_password("Owner Pass 123").unwrap();
        let user_id = sqlx::query(
            "INSERT INTO users (username, password_hash, full_name) VALUES ('owner', ?, 'Owner')",
        )
        .bind(hash)
        .execute(&state.pool)
        .await
        .unwrap()
        .last_insert_rowid();
        let principal = Principal {
            session_id: "settings-test-session".into(),
            user_id,
            username: "owner".into(),
            full_name: "Owner".into(),
            roles: vec!["owner".into()],
            permissions: vec!["settings.manage".into()],
        };
        (dir, state, principal)
    }

    #[tokio::test]
    async fn general_and_print_settings_survive_database_reopen() {
        let (dir, state, principal) = setup().await;
        update_general(
            &state,
            &principal,
            GeneralSettingsInput {
                shop_name: "Oak & Pine".into(),
                owner_name: "A. Owner".into(),
                address: "Main Road".into(),
                phone: "+92 300 1234567".into(),
                currency: "PKR".into(),
            },
            "general-test",
        )
        .await
        .unwrap();
        update_print(
            &state,
            &principal,
            PrintSettingsInput {
                paper_size: "a4".into(),
                orientation: "landscape".into(),
                margin_mm: 12,
                font_size: "large".into(),
                copies: 2,
                show_logo: true,
                show_address: false,
                show_phone: true,
                show_payment_details: true,
                footer_text: "Thank you".into(),
                printer_destination: "print-window".into(),
            },
            "print-test",
        )
        .await
        .unwrap();
        state.pool.close().await;

        let paths = crate::infrastructure::FilePaths::init(&dir).unwrap();
        let (pool, _) = crate::infrastructure::db::open(&paths).await.unwrap();
        let reopened = AppState::new(pool, paths);
        assert_eq!(
            get(&reopened, "shop.name").await.unwrap().as_deref(),
            Some("\"Oak & Pine\"")
        );
        assert_eq!(
            get(&reopened, "shop.owner_name").await.unwrap().as_deref(),
            Some("\"A. Owner\"")
        );
        assert_eq!(
            get(&reopened, "print.paper_size").await.unwrap().as_deref(),
            Some("\"a4\"")
        );
        assert_eq!(
            get(&reopened, "print.copies").await.unwrap().as_deref(),
            Some("2")
        );
        assert_eq!(
            get(&reopened, "print.footer_text")
                .await
                .unwrap()
                .as_deref(),
            Some("\"Thank you\"")
        );
        assert!(reopened.audits.verify_chain().await.unwrap().is_none());
        reopened.pool.close().await;
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn currency_history_rule_and_logo_lifecycle_are_enforced() {
        let (dir, state, principal) = setup().await;
        update_general(
            &state,
            &principal,
            GeneralSettingsInput {
                shop_name: "Furniture Shop".into(),
                owner_name: String::new(),
                address: String::new(),
                phone: String::new(),
                currency: "PKR".into(),
            },
            "defaults",
        )
        .await
        .unwrap();
        let account_id: i64 = sqlx::query_scalar("SELECT id FROM cash_accounts LIMIT 1")
            .fetch_one(&state.pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO cash_entries (cash_account_id, entry_type, amount_minor, created_by) VALUES (?, 'owner_capital', 100, ?)",
        )
        .bind(account_id)
        .bind(principal.user_id)
        .execute(&state.pool)
        .await
        .unwrap();
        let currency_error = update_general(
            &state,
            &principal,
            GeneralSettingsInput {
                shop_name: "Furniture Shop".into(),
                owner_name: String::new(),
                address: String::new(),
                phone: String::new(),
                currency: "USD".into(),
            },
            "currency-change",
        )
        .await
        .unwrap_err();
        assert!(matches!(currency_error, AppError::Conflict(_)));
        assert_eq!(
            get(&state, "shop.currency").await.unwrap().as_deref(),
            Some("\"PKR\"")
        );

        let source = dir.join("test-logo.png");
        image::DynamicImage::new_rgba8(8, 8)
            .save_with_format(&source, image::ImageFormat::Png)
            .unwrap();
        let data = replace_logo(&state, &principal, source.to_str().unwrap(), "logo-replace")
            .await
            .unwrap();
        assert!(data.starts_with("data:image/webp;base64,"));
        assert!(logo_data(&state).await.unwrap().is_some());
        remove_logo(&state, &principal, "logo-remove")
            .await
            .unwrap();
        assert!(logo_data(&state).await.unwrap().is_none());

        state.pool.close().await;
        let _ = std::fs::remove_dir_all(dir);
    }
}
