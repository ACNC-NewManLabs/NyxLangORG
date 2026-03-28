use super::display_list::{DisplayList, DrawOp};

#[derive(Debug, Clone, Default)]
pub struct Batch {
    pub ops: Vec<DrawOp>,
}

pub fn build_batches(display_list: &DisplayList) -> Vec<Batch> {
    let mut batches = Vec::new();
    let mut current = Batch::default();
    for op in &display_list.ops {
        current.ops.push(op.clone());
        if current.ops.len() >= 64 {
            batches.push(std::mem::take(&mut current));
        }
    }
    if !current.ops.is_empty() {
        batches.push(current);
    }
    batches
}
