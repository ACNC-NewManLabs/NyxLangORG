#[derive(Debug, Clone, Default)]
pub struct MemoryStats {
    pub runtime_objects: usize,
    pub glyph_cache_bytes: usize,
    pub image_cache_bytes: usize,
}
