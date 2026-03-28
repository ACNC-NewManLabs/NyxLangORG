//! NYX HashMap Module

use std::collections::HashMap as StdHashMap;
use std::hash::Hash;

/// A hash map
pub struct HashMap<K, V> {
    inner: StdHashMap<K, V>,
}

impl<K: Eq + Hash, V> HashMap<K, V> {
    /// Create new hash map
    pub fn new() -> HashMap<K, V> {
        HashMap { inner: StdHashMap::new() }
    }

    /// With capacity
    pub fn with_capacity(cap: usize) -> HashMap<K, V> {
        HashMap { inner: StdHashMap::with_capacity(cap) }
    }

    /// Insert
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.inner.insert(key, value)
    }

    /// Get
    pub fn get(&self, key: &K) -> Option<&V> {
        self.inner.get(key)
    }

    /// Contains key
    pub fn contains_key(&self, key: &K) -> bool {
        self.inner.contains_key(key)
    }

    /// Remove
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.inner.remove(key)
    }

    /// Length
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Is empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Clear
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Iterator
    pub fn iter(&self) -> std::collections::hash_map::Iter<'_, K, V> {
        self.inner.iter()
    }
}

impl<K: Eq + Hash, V> Default for HashMap<K, V> {
    fn default() -> HashMap<K, V> {
        HashMap::new()
    }
}

#[cfg(test)]
mod tests {
    use super::HashMap;

    #[test]
    fn hash_map_basic_ops() {
        let mut m = HashMap::new();
        assert!(m.is_empty());
        assert_eq!(m.insert("a", 1), None);
        assert!(m.contains_key(&"a"));
        assert_eq!(m.get(&"a"), Some(&1));
        assert_eq!(m.insert("a", 2), Some(1));
        assert_eq!(m.len(), 1);
        assert_eq!(m.remove(&"a"), Some(2));
        assert!(m.is_empty());
    }
}
