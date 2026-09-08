pub struct User {
    pub id: i64,
    pub username: String,
    /// Argon2id hash. Never leaves this module — commands must build safe DTOs.
    pub password_hash: String,
    pub full_name: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

pub struct RoleTemplate {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
}

pub struct Permission {
    pub id: i64,
    pub code: String,
    pub description: Option<String>,
}

pub struct AuditEvent {
    pub id: i64,
    pub user_id: Option<i64>,
    pub action: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
    pub reason: Option<String>,
    pub before_json: Option<String>,
    pub after_json: Option<String>,
    pub approval_user_id: Option<i64>,
    pub session_id: Option<String>,
    pub app_version: Option<String>,
    pub correlation_id: Option<String>,
    pub prev_hash: Option<String>,
    pub created_at: String,
}
