use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct GlyphCache {
    pub entries: BTreeMap<String, (u32, u32)>,
}

impl GlyphCache {
    pub fn cache_key(face: &str, glyph_id: u32, size: u32) -> String {
        format!("{face}:{glyph_id}:{size}")
    }
}
