use std::collections::BTreeMap;

use super::decode::DecodedImage;

#[derive(Debug, Clone, Default)]
pub struct ImageCache {
    pub entries: BTreeMap<String, DecodedImage>,
}

impl ImageCache {
    pub fn insert(&mut self, key: impl Into<String>, image: DecodedImage) {
        self.entries.insert(key.into(), image);
    }
}
