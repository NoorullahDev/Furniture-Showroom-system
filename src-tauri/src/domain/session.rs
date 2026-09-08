pub struct Session {
    pub id: String,
    pub user_id: i64,
    pub active: bool,
    pub locked_at: Option<String>,
    pub created_at: String,
    pub last_used_at: String,
}
