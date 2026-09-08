use std::sync::Arc;

use sqlx::SqlitePool;

use crate::infrastructure::clock::SystemClock;
use crate::infrastructure::id::UuidIdGenerator;
use crate::infrastructure::session::SessionManager;
use crate::infrastructure::write_coordinator::WriteCoordinator;
use crate::infrastructure::FilePaths;

pub struct AppState {
    pub pool: SqlitePool,
    pub paths: FilePaths,
    pub clock: SystemClock,
    pub ids: UuidIdGenerator,
    pub write_coordinator: WriteCoordinator,
    pub sessions: SessionManager,
}

impl AppState {
    pub fn new(pool: SqlitePool, paths: FilePaths) -> Self {
        let clock = SystemClock;
        let ids = UuidIdGenerator;
        let write_coordinator = WriteCoordinator::new();
        let sessions = SessionManager::new(pool.clone(), Arc::new(clock), Arc::new(ids));
        Self {
            pool,
            paths,
            clock,
            ids,
            write_coordinator,
            sessions,
        }
    }
}
