use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct FontDb {
    pub families: BTreeMap<String, Vec<String>>,
}

impl FontDb {
    pub fn register(&mut self, family: impl Into<String>, face: impl Into<String>) {
        self.families.entry(family.into()).or_default().push(face.into());
    }
}
