use std::sync::atomic::AtomicU8;
use std::sync::Arc;

use sqlx::SqlitePool;

use crate::infrastructure::audit::AuditService;
use crate::infrastructure::clock::SystemClock;
use crate::infrastructure::id::UuidIdGenerator;
use crate::infrastructure::session::SessionManager;
use crate::infrastructure::throttle::LoginThrottle;
use crate::infrastructure::write_coordinator::WriteCoordinator;
use crate::infrastructure::FilePaths;

pub struct AppState {
    pub pool: SqlitePool,
    pub paths: FilePaths,
    pub clock: SystemClock,
    pub ids: UuidIdGenerator,
    pub write_coordinator: WriteCoordinator,
    pub sessions: SessionManager,
    pub audits: AuditService,
    pub throttle: LoginThrottle,
    /// 0 = idle, 1 = close backup running, 2 = close backup failed.
    pub backup_close_state: AtomicU8,
}

impl AppState {
    pub fn new(pool: SqlitePool, paths: FilePaths) -> Self {
        let clock = SystemClock;
        let ids = UuidIdGenerator;
        let write_coordinator = WriteCoordinator::new();
        let sessions = SessionManager::new(pool.clone(), Arc::new(clock), Arc::new(ids));
        let app_version = env!("CARGO_PKG_VERSION").to_string();
        let audits = AuditService::new(pool.clone(), Arc::new(clock), app_version);
        let throttle = LoginThrottle::new();
        Self {
            pool,
            paths,
            clock,
            ids,
            write_coordinator,
            sessions,
            audits,
            throttle,
            backup_close_state: AtomicU8::new(0),
        }
    }
}
