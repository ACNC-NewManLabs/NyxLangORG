use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct FrameClock {
    started_at: Instant,
    frame_id: u64,
    target_frame_time: Duration,
}

impl FrameClock {
    pub fn new(target_fps: u32) -> Self {
        let fps = target_fps.max(1);
        Self {
            started_at: Instant::now(),
            frame_id: 0,
            target_frame_time: Duration::from_secs_f64(1.0 / fps as f64),
        }
    }

    pub fn begin_frame(&mut self) -> u64 {
        self.frame_id += 1;
        self.frame_id
    }

    pub fn elapsed_micros(&self) -> u64 {
        self.started_at.elapsed().as_micros() as u64
    }

    pub fn target_frame_time(&self) -> Duration {
        self.target_frame_time
    }
}
