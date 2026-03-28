#[derive(Debug, Clone, Default)]
pub struct WgpuFrame {
    pub command_count: usize,
    pub uploaded_bytes: usize,
}
