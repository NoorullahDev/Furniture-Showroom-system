use chrono::{DateTime, SecondsFormat, Utc};

pub trait Clock: Send + Sync {
    fn now_utc(&self) -> DateTime<Utc>;

    fn now_iso(&self) -> String {
        self.now_utc().to_rfc3339_opts(SecondsFormat::Micros, true)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_utc(&self) -> DateTime<Utc> {
        Utc::now()
    }
}
