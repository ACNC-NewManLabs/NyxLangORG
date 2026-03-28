use super::batcher::{build_batches, Batch};
use super::scene::{Layer, Scene};

pub fn flatten_scene(scene: &Scene) -> Vec<Batch> {
    let mut out = Vec::new();
    visit_layer(&scene.root, &mut out);
    out
}

fn visit_layer(layer: &Layer, out: &mut Vec<Batch>) {
    out.extend(build_batches(&layer.display_list));
    for child in &layer.children {
        visit_layer(child, out);
    }
}
