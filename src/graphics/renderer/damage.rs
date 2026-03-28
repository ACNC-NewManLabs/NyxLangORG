#[derive(Debug, Clone, Copy, Default)]
pub struct DamageRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Default)]
pub struct DamageTracker {
    pub rects: Vec<DamageRect>,
}

impl DamageTracker {
    pub fn record(&mut self, rect: DamageRect) {
        self.rects.push(rect);
    }
}
