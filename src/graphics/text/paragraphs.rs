use super::shaping::{shape_text, ShapedRun};

#[derive(Debug, Clone)]
pub struct ParagraphRequest {
    pub text: String,
    pub max_width: f32,
    pub font_size: f32,
}

#[derive(Debug, Clone, Default)]
pub struct ParagraphLayout {
    pub runs: Vec<ShapedRun>,
    pub width: f32,
    pub height: f32,
}

pub fn layout_paragraph(request: &ParagraphRequest) -> ParagraphLayout {
    let run = shape_text(&request.text, request.font_size);
    ParagraphLayout {
        width: run.width.min(request.max_width),
        height: request.font_size * 1.2,
        runs: vec![run],
    }
}
