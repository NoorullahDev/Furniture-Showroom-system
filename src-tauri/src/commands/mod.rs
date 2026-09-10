pub mod audit;
pub mod auth;
pub mod catalogue;
pub mod dashboard;
pub mod expenses;
pub mod first_run;
pub mod fulfilment;
pub mod inventory;
pub mod products;
pub mod proof;
pub mod purchases;
pub mod roles;
pub mod sales;
pub mod search;
pub mod settings;
pub mod suppliers;
pub mod users;
pub mod wrapper;

use crate::application::auth::Principal;
use crate::error::AppError;
use crate::state::AppState;

/// Resolve an active, unlocked session without requiring a permission. Used by
/// read-only flows that every authenticated role may call (e.g. reading shop
/// settings).
pub async fn authenticated(state: &AppState, session: &str) -> Result<Principal, AppError> {
    crate::application::auth::resolve_session(state, session).await
}

/// Resolve an active, unlocked session and require a permission. Every
/// protected command starts here; the UI never replaces this enforcement.
pub async fn authed(
    state: &AppState,
    session: &str,
    permission: &str,
) -> Result<Principal, AppError> {
    let principal = authenticated(state, session).await?;
    principal.require(permission)?;
    Ok(principal)
}
