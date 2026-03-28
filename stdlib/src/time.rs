//! NYX Time Layer

pub mod time {
    pub struct Instant(std::time::Instant);
    pub struct Duration(std::time::Duration);

    impl Instant {
        pub fn now() -> Instant { Instant(std::time::Instant::now()) }
        pub fn duration_since(&self, other: &Instant) -> Duration { Duration(self.0.duration_since(other.0)) }
    }

    impl Duration {
        pub fn from_secs(s: u64) -> Duration { Duration(std::time::Duration::from_secs(s)) }
        pub fn from_millis(ms: u64) -> Duration { Duration(std::time::Duration::from_millis(ms)) }
        pub fn as_secs(&self) -> u64 { self.0.as_secs() }
        pub fn as_millis(&self) -> u128 { self.0.as_millis() }
    }
}

pub use time::*;

