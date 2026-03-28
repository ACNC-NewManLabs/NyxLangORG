#[derive(Debug, Clone)]
pub struct WgpuSurface {
    pub width: u32,
    pub height: u32,
    pub frame_index: u64,
}

impl WgpuSurface {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            frame_index: 0,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }
}
