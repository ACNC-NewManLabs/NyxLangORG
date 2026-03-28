#[derive(Debug, Clone)]
pub struct ShapedGlyph {
    pub glyph_id: u32,
    pub advance: f32,
}

#[derive(Debug, Clone, Default)]
pub struct ShapedRun {
    pub glyphs: Vec<ShapedGlyph>,
    pub width: f32,
}

pub fn shape_text(text: &str, font_size: f32) -> ShapedRun {
    let mut glyphs = Vec::new();
    let advance = font_size * 0.6;
    for (index, _) in text.chars().enumerate() {
        glyphs.push(ShapedGlyph {
            glyph_id: index as u32,
            advance,
        });
    }
    ShapedRun {
        width: glyphs.len() as f32 * advance,
        glyphs,
    }
}
