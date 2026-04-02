//! NYX LinkedList Module

use std::collections::LinkedList as StdLinkedList;

pub struct LinkedList<T> {
    inner: StdLinkedList<T>,
}

impl<T> LinkedList<T> {
    pub fn new() -> LinkedList<T> {
        LinkedList {
            inner: StdLinkedList::new(),
        }
    }
    pub fn push_back(&mut self, value: T) {
        self.inner.push_back(value);
    }
    pub fn push_front(&mut self, value: T) {
        self.inner.push_front(value);
    }
    pub fn pop_back(&mut self) -> Option<T> {
        self.inner.pop_back()
    }
    pub fn pop_front(&mut self) -> Option<T> {
        self.inner.pop_front()
    }
    pub fn len(&self) -> usize {
        self.inner.len()
    }
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl<T> Default for LinkedList<T> {
    fn default() -> LinkedList<T> {
        LinkedList::new()
    }
}

#[cfg(test)]
mod tests {
    use super::LinkedList;

    #[test]
    fn linked_list_basic_ops() {
        let mut l = LinkedList::new();
        assert!(l.is_empty());
        l.push_back(1);
        l.push_front(0);
        assert_eq!(l.len(), 2);
        assert_eq!(l.pop_front(), Some(0));
        assert_eq!(l.pop_back(), Some(1));
        assert!(l.is_empty());
    }
}
