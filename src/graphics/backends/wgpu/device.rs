#[derive(Debug, Clone)]
pub struct WgpuDevice {
    pub label: String,
    pub max_texture_dimension_2d: u32,
}

impl WgpuDevice {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            max_texture_dimension_2d: 8192,
        }
    }
}
