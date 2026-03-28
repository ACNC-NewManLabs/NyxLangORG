use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct SnapshotStore {
    pub widget_trees: BTreeMap<u64, String>,
    pub render_trees: BTreeMap<u64, String>,
    pub semantics_trees: BTreeMap<u64, String>,
}

impl SnapshotStore {
    pub fn put_widget(&mut self, generation: u64, snapshot: String) {
        self.widget_trees.insert(generation, snapshot);
    }

    pub fn put_render(&mut self, generation: u64, snapshot: String) {
        self.render_trees.insert(generation, snapshot);
    }

    pub fn put_semantics(&mut self, generation: u64, snapshot: String) {
        self.semantics_trees.insert(generation, snapshot);
    }
}
