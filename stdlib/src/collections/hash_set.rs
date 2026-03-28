//! NYX HashSet Module

use std::collections::HashSet as StdHashSet;
use std::hash::Hash;

/// A hash set
pub struct HashSet<T> {
    inner: StdHashSet<T>,
}

impl<T: Eq + Hash> HashSet<T> {
    /// Create new hash set
    pub fn new() -> HashSet<T> {
        HashSet { inner: StdHashSet::new() }
    }

    /// With capacity
    pub fn with_capacity(cap: usize) -> HashSet<T> {
        HashSet { inner: StdHashSet::with_capacity(cap) }
    }

    /// Insert
    pub fn insert(&mut self, value: T) -> bool {
        self.inner.insert(value)
    }

    /// Contains
    pub fn contains(&self, value: &T) -> bool {
        self.inner.contains(value)
    }

    /// Remove
    pub fn remove(&mut self, value: &T) -> bool {
        self.inner.remove(value)
    }

    /// Length
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Is empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl<T: Eq + Hash> Default for HashSet<T> {
    fn default() -> HashSet<T> {
        HashSet::new()
    }
}

#[cfg(test)]
mod tests {
    use super::HashSet;

    #[test]
    fn hash_set_basic_ops() {
        let mut s = HashSet::new();
        assert!(s.is_empty());
        assert!(s.insert(1));
        assert!(!s.insert(1));
        assert!(s.contains(&1));
        assert!(s.remove(&1));
        assert!(!s.contains(&1));
        assert!(s.is_empty());
    }
}
