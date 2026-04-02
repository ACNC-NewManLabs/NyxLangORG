use std::time::{Duration, Instant};

use super::protocol::{DevtoolsEnvelope, DevtoolsPayload, DevtoolsStream};

#[derive(Debug, Clone)]
pub struct Timeline {
    seq: u64,
    started_at: Instant,
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}

impl Timeline {
    pub fn new() -> Self {
        Self {
            seq: 0,
            started_at: Instant::now(),
        }
    }

    pub fn next(&mut self, stream: DevtoolsStream, payload: DevtoolsPayload) -> DevtoolsEnvelope {
        self.seq += 1;
        DevtoolsEnvelope {
            seq: self.seq,
            ts_micros: self.started_at.elapsed().as_micros() as u64,
            stream,
            payload,
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }
}
