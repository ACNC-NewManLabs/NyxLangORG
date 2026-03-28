#[derive(Debug, Clone, Default)]
pub struct GpuStats {
    pub draw_calls: usize,
    pub batches: usize,
    pub uploaded_bytes: usize,
}
