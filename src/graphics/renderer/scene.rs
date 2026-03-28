use super::display_list::DisplayList;

#[derive(Debug, Clone, Default)]
pub struct Layer {
    pub id: u64,
    pub opacity: f32,
    pub display_list: DisplayList,
    pub children: Vec<Layer>,
}

#[derive(Debug, Clone, Default)]
pub struct Scene {
    pub root: Layer,
    pub generation: u64,
}
