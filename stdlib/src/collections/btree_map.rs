//! NYX BTreeMap Module

use std::collections::BTreeMap as StdBTreeMap;

/// A B-tree map
pub struct BTreeMap<K, V> {
    inner: StdBTreeMap<K, V>,
}

impl<K: Ord, V> BTreeMap<K, V> {
    pub fn new() -> BTreeMap<K, V> {
        BTreeMap { inner: StdBTreeMap::new() }
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.inner.insert(key, value)
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.inner.get(key)
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.inner.remove(key)
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl<K: Ord, V> Default for BTreeMap<K, V> {
    fn default() -> BTreeMap<K, V> {
        BTreeMap::new()
    }
}

#[cfg(test)]
mod tests {
    use super::BTreeMap;

    #[test]
    fn btree_map_basic_ops() {
        let mut m = BTreeMap::new();
        assert!(m.is_empty());
        assert_eq!(m.insert(2, "b"), None);
        assert_eq!(m.insert(1, "a"), None);
        assert_eq!(m.get(&1), Some(&"a"));
        assert_eq!(m.remove(&2), Some("b"));
        assert_eq!(m.len(), 1);
    }
}
