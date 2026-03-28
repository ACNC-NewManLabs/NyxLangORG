use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DevtoolsStream {
    Timeline,
    Snapshot,
    Memory,
    Gpu,
    Reload,
    Inspector,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DevtoolsPayload {
    FrameStarted { frame_id: u64 },
    FrameEnded { frame_id: u64, total_micros: u64 },
    LayoutStats { nodes: usize, micros: u64 },
    PaintStats { ops: usize, micros: u64 },
    DrawCalls { count: usize },
    GpuSubmission { frame_id: u64, batches: usize },
    HotReloadPatched { modules: Vec<String> },
    SnapshotRef { kind: String, generation: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevtoolsEnvelope {
    pub seq: u64,
    pub ts_micros: u64,
    pub stream: DevtoolsStream,
    pub payload: DevtoolsPayload,
}
