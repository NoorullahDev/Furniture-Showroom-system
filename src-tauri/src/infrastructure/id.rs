pub trait IdGenerator: Send + Sync {
    fn new_id(&self) -> String;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct UuidIdGenerator;

impl IdGenerator for UuidIdGenerator {
    fn new_id(&self) -> String {
        uuid::Uuid::now_v7().to_string()
    }
}
