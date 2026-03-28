use crate::graphics::renderer::pipelines::PipelineKind;

#[derive(Debug, Clone)]
pub struct WgpuPipelineSet {
    pub supported: Vec<PipelineKind>,
}

impl Default for WgpuPipelineSet {
    fn default() -> Self {
        Self {
            supported: vec![PipelineKind::Solid, PipelineKind::Text, PipelineKind::Image],
        }
    }
}
