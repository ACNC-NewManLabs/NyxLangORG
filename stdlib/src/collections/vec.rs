//! NYX Vec Module

use std::vec::Vec as StdVec;

/// A growable array type
pub struct Vec<T> {
    inner: StdVec<T>,
}

impl<T> Vec<T> {
    /// Create a new empty vector
    pub fn new() -> Vec<T> {
        Vec {
            inner: StdVec::new(),
        }
    }

    /// Create vector with capacity
    pub fn with_capacity(cap: usize) -> Vec<T> {
        Vec {
            inner: StdVec::with_capacity(cap),
        }
    }

    /// Push element to end
    pub fn push(&mut self, value: T) {
        self.inner.push(value);
    }

    /// Pop element from end
    pub fn pop(&mut self) -> Option<T> {
        self.inner.pop()
    }

    /// Get length
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get element at index
    pub fn get(&self, index: usize) -> Option<&T> {
        self.inner.get(index)
    }

    /// Industrial Safe Get: Returns Result with NyxError on OOB
    pub fn safe_get(&self, index: usize) -> Result<&T, crate::error::NyxError> {
        self.inner.get(index).ok_or_else(|| {
            crate::error::NyxError::new(
                "STD001",
                format!(
                    "Index out of bounds: index {} is >= length {}",
                    index,
                    self.inner.len()
                ),
                crate::error::ErrorCategory::Runtime,
            )
            .with_suggestion("Check index bounds before access or use safe_get().")
        })
    }

    /// Industrial Expect: Panics with formatted NyxError report
    pub fn expect_at(&self, index: usize) -> &T {
        match self.safe_get(index) {
            Ok(v) => v,
            Err(e) => {
                panic!("\n{}", e);
            }
        }
    }

    /// Get mutable element
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.inner.get_mut(index)
    }

    /// Clear vector
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Append another vector
    pub fn append(&mut self, other: &mut Vec<T>) {
        self.inner.append(&mut other.inner);
    }

    /// Get first element
    pub fn first(&self) -> Option<&T> {
        self.inner.first()
    }

    /// Get last element
    pub fn last(&self) -> Option<&T> {
        self.inner.last()
    }

    /// Iterator
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.inner.iter()
    }

    /// Mutable iterator
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        self.inner.iter_mut()
    }

    /// Get as slice
    pub fn as_slice(&self) -> &[T] {
        self.inner.as_slice()
    }
}

impl<T> Default for Vec<T> {
    fn default() -> Vec<T> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::Vec;

    #[test]
    fn vec_basic_ops() {
        let mut v = Vec::new();
        assert!(v.is_empty());
        v.push(1);
        v.push(2);
        assert_eq!(v.len(), 2);
        assert_eq!(v.first(), Some(&1));
        assert_eq!(v.last(), Some(&2));
        assert_eq!(v.get(0), Some(&1));
        assert_eq!(v.pop(), Some(2));
        assert_eq!(v.len(), 1);
        v.clear();
        assert!(v.is_empty());
    }

    #[test]
    fn vec_append() {
        let mut a = Vec::new();
        let mut b = Vec::new();
        a.push(1);
        b.push(2);
        b.push(3);
        a.append(&mut b);
        assert_eq!(a.len(), 3);
        assert!(b.is_empty());
    }
}
