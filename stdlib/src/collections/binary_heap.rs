//! NYX BinaryHeap Module

use std::collections::BinaryHeap as StdBinaryHeap;

pub struct BinaryHeap<T> {
    inner: StdBinaryHeap<T>,
}

impl<T: Ord> BinaryHeap<T> {
    pub fn new() -> BinaryHeap<T> {
        BinaryHeap {
            inner: StdBinaryHeap::new(),
        }
    }
    pub fn push(&mut self, value: T) {
        self.inner.push(value);
    }
    pub fn pop(&mut self) -> Option<T> {
        self.inner.pop()
    }
    pub fn peek(&self) -> Option<&T> {
        self.inner.peek()
    }
    pub fn len(&self) -> usize {
        self.inner.len()
    }
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl<T: Ord> Default for BinaryHeap<T> {
    fn default() -> BinaryHeap<T> {
        BinaryHeap::new()
    }
}

#[cfg(test)]
mod tests {
    use super::BinaryHeap;

    #[test]
    fn binary_heap_basic_ops() {
        let mut h = BinaryHeap::new();
        assert!(h.is_empty());
        h.push(2);
        h.push(1);
        assert_eq!(h.peek(), Some(&2));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(1));
        assert!(h.is_empty());
    }
}
