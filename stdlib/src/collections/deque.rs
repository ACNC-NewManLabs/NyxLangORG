//! NYX Deque Module

use std::collections::VecDeque as StdVecDeque;

pub struct Deque<T> {
    inner: StdVecDeque<T>,
}

impl<T> Deque<T> {
    pub fn new() -> Deque<T> {
        Deque { inner: StdVecDeque::new() }
    }
    pub fn push_back(&mut self, value: T) { self.inner.push_back(value); }
    pub fn push_front(&mut self, value: T) { self.inner.push_front(value); }
    pub fn pop_back(&mut self) -> Option<T> { self.inner.pop_back() }
    pub fn pop_front(&mut self) -> Option<T> { self.inner.pop_front() }
    pub fn len(&self) -> usize { self.inner.len() }
    pub fn is_empty(&self) -> bool { self.inner.is_empty() }
}

impl<T> Default for Deque<T> {
    fn default() -> Deque<T> { Deque::new() }
}

#[cfg(test)]
mod tests {
    use super::Deque;

    #[test]
    fn deque_basic_ops() {
        let mut d = Deque::new();
        assert!(d.is_empty());
        d.push_back(1);
        d.push_front(0);
        assert_eq!(d.len(), 2);
        assert_eq!(d.pop_front(), Some(0));
        assert_eq!(d.pop_back(), Some(1));
        assert!(d.is_empty());
    }
}
