#[derive(Debug, Clone)]
pub enum DrawOp {
    SolidQuad {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: [f32; 4],
    },
    GlyphRun {
        x: f32,
        y: f32,
        glyphs: usize,
        color: [f32; 4],
    },
    Image {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        image_id: String,
    },
    ClipRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    Transform([f32; 9]),
}

#[derive(Debug, Clone, Default)]
pub struct DisplayList {
    pub ops: Vec<DrawOp>,
}

impl DisplayList {
    pub fn push(&mut self, op: DrawOp) {
        self.ops.push(op);
    }
}
