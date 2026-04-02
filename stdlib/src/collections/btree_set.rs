//! NYX BTreeSet Module

use std::collections::BTreeSet as StdBTreeSet;

pub struct BTreeSet<T> {
    inner: StdBTreeSet<T>,
}

impl<T: Ord> BTreeSet<T> {
    pub fn new() -> BTreeSet<T> {
        BTreeSet {
            inner: StdBTreeSet::new(),
        }
    }
    pub fn insert(&mut self, value: T) -> bool {
        self.inner.insert(value)
    }
    pub fn contains(&self, value: &T) -> bool {
        self.inner.contains(value)
    }
    pub fn remove(&mut self, value: &T) -> bool {
        self.inner.remove(value)
    }
    pub fn len(&self) -> usize {
        self.inner.len()
    }
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl<T: Ord> Default for BTreeSet<T> {
    fn default() -> BTreeSet<T> {
        BTreeSet::new()
    }
}

#[cfg(test)]
mod tests {
    use super::BTreeSet;

    #[test]
    fn btree_set_basic_ops() {
        let mut s = BTreeSet::new();
        assert!(s.is_empty());
        assert!(s.insert(2));
        assert!(s.insert(1));
        assert!(s.contains(&1));
        assert!(s.remove(&2));
        assert_eq!(s.len(), 1);
    }
}
