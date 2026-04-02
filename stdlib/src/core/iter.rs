//! NYX Core Iterator Module
//!
//! Zero-allocation iterator traits and implementations for the core library.
//! This module provides the foundational iterator protocol.

// Redundant imports removed

// =============================================================================
// Iterator Trait
// =============================================================================

/// The core iterator trait.
///
/// An iterator has a source of values and a method to get the next value.
/// Iterators are lazy - they don't do anything until you call next().
pub trait Iterator {
    /// The type of the elements being iterated over.
    type Item;

    /// Advances the iterator and returns the next value.
    ///
    /// Returns None when iteration is finished.
    fn next(&mut self) -> Option<Self::Item>;

    /// Returns the bounds on the remaining length of the iterator.
    ///
    /// Specifically, size_hint() returns a tuple where the first element
    /// is the lower bound, and the second element is the upper bound.
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }

    /// Returns the exact number of items in the iterator, if known.
    /// Returns None if the length is not known.
    fn exact_size_hint(&self) -> Option<usize> {
        self.size_hint().1
    }

    /// Consumes the iterator, counting the number of items.
    fn count(mut self) -> usize
    where
        Self: Sized,
    {
        let mut count = 0;
        while let Some(_) = self.next() {
            count += 1;
        }
        count
    }

    /// Consumes the iterator, returning the last element.
    fn last(mut self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        let mut last = None;
        while let Some(item) = self.next() {
            last = Some(item);
        }
        last
    }

    /// Consumes the iterator, returning the nth element.
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let mut i = 0;
        while let Some(item) = self.next() {
            if i == n {
                return Some(item);
            }
            i += 1;
        }
        None
    }

    /// Consumes the iterator, collecting elements into a collection.
    fn collect<B>(self) -> B
    where
        Self: Sized,
        B: FromIterator<Self::Item>,
    {
        FromIterator::from_iter(self)
    }

    /// Applies a function to every element, producing nothing.
    fn for_each<F>(mut self, mut f: F)
    where
        Self: Sized,
        F: FnMut(Self::Item),
    {
        while let Some(item) = self.next() {
            f(item);
        }
    }

    /// Creates an iterator which uses a closure to determine if an element
    /// should be yielded.
    fn filter<P>(self, predicate: P) -> Filter<Self, P>
    where
        Self: Sized,
        P: FnMut(&Self::Item) -> bool,
    {
        Filter {
            iter: self,
            predicate,
        }
    }

    /// Creates an iterator which maps values using a closure.
    fn map<B, F>(self, f: F) -> Map<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Item) -> B,
    {
        Map { iter: self, f }
    }

    /// Creates an iterator which both maps and filters.
    fn filter_map<B, F>(self, f: F) -> FilterMap<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Item) -> Option<B>,
    {
        FilterMap { iter: self, f }
    }

    /// Creates an iterator which gives the current count and the value.
    fn enumerate(self) -> Enumerate<Self>
    where
        Self: Sized,
    {
        Enumerate {
            iter: self,
            index: 0,
        }
    }

    /// Creates an iterator which can peek at the next element without
    /// consuming it.
    fn peekable(self) -> Peekable<Self>
    where
        Self: Sized,
    {
        Peekable {
            iter: self,
            cache: None,
        }
    }

    /// Creates an iterator which skips the first n elements.
    fn skip(self, n: usize) -> Skip<Self>
    where
        Self: Sized,
    {
        Skip { iter: self, n }
    }

    /// Creates an iterator which takes the first n elements.
    fn take(self, n: usize) -> Take<Self>
    where
        Self: Sized,
    {
        Take { iter: self, n }
    }

    /// Creates an iterator which zips two iterators together.
    fn zip<B>(self, other: B) -> Zip<Self, B::IntoIter>
    where
        Self: Sized,
        B: IntoIterator,
    {
        Zip {
            a: self,
            b: other.into_iter(),
        }
    }

    /// Creates an iterator that chains two iterators.
    fn chain(
        self,
        other: impl IntoIterator<Item = Self::Item>,
    ) -> Chain<Self, impl Iterator<Item = Self::Item>>
    where
        Self: Sized,
    {
        Chain {
            a: self,
            b: other.into_iter(),
        }
    }

    /// Returns true if the iterator contains an element.
    fn contains(mut self, x: &Self::Item) -> bool
    where
        Self: Sized,
        Self::Item: PartialEq + core::fmt::Debug + Copy + Clone + Eq + core::hash::Hash,
    {
        while let Some(item) = self.next() {
            if item == *x {
                return true;
            }
        }
        false
    }

    /// Returns true if any element satisfies the predicate.
    fn any<F>(mut self, mut f: F) -> bool
    where
        Self: Sized,
        F: FnMut(Self::Item) -> bool,
    {
        while let Some(item) = self.next() {
            if f(item) {
                return true;
            }
        }
        false
    }

    /// Returns true if all elements satisfy the predicate.
    fn all<F>(mut self, mut f: F) -> bool
    where
        Self: Sized,
        F: FnMut(Self::Item) -> bool,
    {
        while let Some(item) = self.next() {
            if !f(item) {
                return false;
            }
        }
        true
    }

    /// Finds the first element satisfying the predicate.
    fn find<P>(mut self, mut predicate: P) -> Option<Self::Item>
    where
        Self: Sized,
        P: FnMut(&Self::Item) -> bool,
    {
        while let Some(item) = self.next() {
            if predicate(&item) {
                return Some(item);
            }
        }
        None
    }

    /// Reduces the iterator to a single value.
    fn fold<B, F>(mut self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        let mut acc = init;
        while let Some(item) = self.next() {
            acc = f(acc, item);
        }
        acc
    }

    /// Reduces the iterator to a single value, returning the first non-None result.
    fn filter_map_try<B, R, F>(mut self, mut f: F) -> Option<B>
    where
        Self: Sized,
        F: FnMut(Self::Item) -> Option<B>,
    {
        while let Some(item) = self.next() {
            if let Some(b) = f(item) {
                return Some(b);
            }
        }
        None
    }
}

// =============================================================================
// IntoIterator Trait
// =============================================================================

/// Trait for types that can be converted to iterators.
pub trait IntoIterator {
    /// The type of the elements being iterated over.
    type Item;

    /// The type of the iterator.
    type IntoIter: Iterator<Item = Self::Item>;

    /// Creates an iterator from a value.
    fn into_iter(self) -> Self::IntoIter;
}

impl<I: Iterator> IntoIterator for I {
    type Item = I::Item;
    type IntoIter = I;

    #[inline]
    fn into_iter(self) -> I {
        self
    }
}

// =============================================================================
// FromIterator Trait
// =============================================================================

/// Trait for types that can be created from an iterator.
pub trait FromIterator<A> {
    /// Creates a value from an iterator.
    fn from_iter<T: IntoIterator<Item = A>>(iter: T) -> Self;
}

// =============================================================================
// Iterator Adapters
// =============================================================================

/// Iterator that maps values using a function.
#[derive(Debug)]
pub struct Map<I, F> {
    iter: I,
    f: F,
}

impl<I, F, B> Iterator for Map<I, F>
where
    I: Iterator,
    F: FnMut(I::Item) -> B,
{
    type Item = B;

    #[inline]
    fn next(&mut self) -> Option<B> {
        self.iter.next().map(|item| (self.f)(item))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

/// Iterator that filters elements using a predicate.
#[derive(Debug)]
pub struct Filter<I, P> {
    iter: I,
    predicate: P,
}

impl<I, P> Iterator for Filter<I, P>
where
    I: Iterator,
    P: FnMut(&I::Item) -> bool,
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<I::Item> {
        while let Some(item) = self.iter.next() {
            if (self.predicate)(&item) {
                return Some(item);
            }
        }
        None
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_, upper) = self.iter.size_hint();
        (0, upper)
    }
}

/// Iterator that both maps and filters.
#[derive(Debug)]
pub struct FilterMap<I, F> {
    iter: I,
    f: F,
}

impl<I, F, B> Iterator for FilterMap<I, F>
where
    I: Iterator,
    F: FnMut(I::Item) -> Option<B>,
{
    type Item = B;

    #[inline]
    fn next(&mut self) -> Option<B> {
        while let Some(item) = self.iter.next() {
            if let Some(b) = (self.f)(item) {
                return Some(b);
            }
        }
        None
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_, upper) = self.iter.size_hint();
        (0, upper)
    }
}

/// Iterator that enumerates elements with their index.
#[derive(Debug)]
pub struct Enumerate<I> {
    iter: I,
    index: usize,
}

impl<I> Iterator for Enumerate<I>
where
    I: Iterator,
{
    type Item = (usize, I::Item);

    #[inline]
    fn next(&mut self) -> Option<(usize, I::Item)> {
        self.iter.next().map(|item| {
            let index = self.index;
            self.index += 1;
            (index, item)
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

/// Iterator that can peek at the next element.
#[derive(Debug)]
pub struct Peekable<I: Iterator> {
    iter: I,
    cache: Option<I::Item>,
}

impl<I> Iterator for Peekable<I>
where
    I: Iterator,
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<I::Item> {
        if let Some(item) = self.cache.take() {
            Some(item)
        } else {
            self.iter.next()
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let cache_len = if self.cache.is_some() { 1 } else { 0 };
        let (lo, hi_val) = self.iter.size_hint();
        (lo + cache_len, hi_val.map(|h| h + cache_len))
    }
}

impl<I> Peekable<I>
where
    I: Iterator,
{
    /// Peeks at the next element without consuming it.
    pub fn peek(&mut self) -> Option<&I::Item> {
        if self.cache.is_none() {
            self.cache = self.iter.next();
        }
        self.cache.as_ref()
    }
}

/// Iterator that skips the first n elements.
#[derive(Debug)]
pub struct Skip<I> {
    iter: I,
    n: usize,
}

impl<I> Iterator for Skip<I>
where
    I: Iterator,
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<I::Item> {
        if self.n > 0 {
            self.iter.nth(self.n - 1);
            self.n = 0;
        }
        self.iter.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lo, hi_val) = self.iter.size_hint();
        let lo = lo.saturating_sub(self.n);
        let hi = hi_val.map(|h| h.saturating_sub(self.n));
        (lo, hi)
    }
}

/// Iterator that takes the first n elements.
#[derive(Debug)]
pub struct Take<I> {
    iter: I,
    n: usize,
}

impl<I> Iterator for Take<I>
where
    I: Iterator,
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<I::Item> {
        if self.n > 0 {
            self.n -= 1;
            self.iter.next()
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lo, _) = self.iter.size_hint();
        (0, Some(self.n.min(lo)))
    }
}

/// Iterator that zips two iterators together.
#[derive(Debug)]
pub struct Zip<A, B> {
    a: A,
    b: B,
}

impl<A, B> Iterator for Zip<A, B>
where
    A: Iterator,
    B: Iterator,
{
    type Item = (A::Item, B::Item);

    #[inline]
    fn next(&mut self) -> Option<(A::Item, B::Item)> {
        match (self.a.next(), self.b.next()) {
            (Some(a), Some(b)) => Some((a, b)),
            _ => None,
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let a_len = self.a.size_hint();
        let b_len = self.b.size_hint();
        let min = a_len.0.min(b_len.0);
        let max = match (a_len.1, b_len.1) {
            (Option::Some(a), Option::Some(b)) => Option::Some(a.min(b)),
            _ => Option::None,
        };
        (min, max)
    }
}

/// Iterator that chains two iterators.
#[derive(Debug)]
pub struct Chain<A, B> {
    a: A,
    b: B,
}

impl<A, B> Iterator for Chain<A, B>
where
    A: Iterator,
    B: Iterator<Item = A::Item>,
{
    type Item = A::Item;

    #[inline]
    fn next(&mut self) -> Option<A::Item> {
        self.a.next().or_else(|| self.b.next())
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let a_len = self.a.size_hint();
        let b_len = self.b.size_hint();
        let min = a_len.0.saturating_add(b_len.0);
        let max = match (a_len.1, b_len.1) {
            (Option::Some(a), Option::Some(b)) => Option::Some(a.saturating_add(b)),
            _ => Option::None,
        };
        (min, max)
    }
}

// =============================================================================
// Double-Ended Iterator
// =============================================================================

/// An iterator able to yield elements from both ends.
pub trait DoubleEndedIterator: Iterator {
    /// Yields the next element from the end of the iterator.
    fn next_back(&mut self) -> Option<Self::Item>;
}

// =============================================================================
// Exact Size Iterator
// =============================================================================

/// An iterator that knows its exact length.
pub trait ExactSizeIterator: Iterator {
    /// Returns the exact length of the iterator.
    fn len(&self) -> usize;

    /// Returns true if the iterator is empty.
    #[inline]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// =============================================================================
// Default Implementations
// =============================================================================

impl<I: Iterator + ?Sized> Iterator for &mut I {
    type Item = I::Item;

    fn next(&mut self) -> Option<I::Item> {
        (**self).next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (**self).size_hint()
    }
}

// Removed problematic blanket IntoIterator impls for &I and &mut I

// =============================================================================
// Standard Implementations for Slices
// =============================================================================

/// Iterator over a slice.
#[derive(Debug, Clone)]
pub struct SliceIter<'a, T: 'a> {
    slice: &'a [T],
    index: usize,
}

impl<'a, T> Iterator for SliceIter<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<&'a T> {
        if self.index < self.slice.len() {
            let item = &self.slice[self.index];
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.slice.len() - self.index;
        (len, Some(len))
    }
}

impl<'a, T> IntoIterator for &'a [T] {
    type Item = &'a T;
    type IntoIter = SliceIter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        SliceIter {
            slice: self,
            index: 0,
        }
    }
}

// =============================================================================
// Range Types
// =============================================================================

/// An iterator over the integers from start to end.
#[derive(Debug, Clone)]
pub struct Range {
    start: usize,
    end: usize,
}

impl Range {
    /// Creates a new range.
    #[inline]
    pub const fn new(start: usize, end: usize) -> Range {
        Range { start, end }
    }
}

impl Iterator for Range {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<usize> {
        if self.start < self.end {
            let result = self.start;
            self.start += 1;
            Some(result)
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.end.saturating_sub(self.start);
        (len, Some(len))
    }
}

impl ExactSizeIterator for Range {
    #[inline]
    fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }
}

impl DoubleEndedIterator for Range {
    #[inline]
    fn next_back(&mut self) -> Option<usize> {
        if self.start < self.end {
            self.end -= 1;
            Some(self.end)
        } else {
            None
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_range() {
        let mut r = Range::new(0, 5);
        assert_eq!(r.next(), Some(0));
        assert_eq!(r.next(), Some(1));
        assert_eq!(r.next(), Some(2));
        assert_eq!(r.next(), Some(3));
        assert_eq!(r.next(), Some(4));
        assert_eq!(r.next(), None);
    }

    #[test]
    fn test_range_size_hint() {
        let r = Range::new(0, 10);
        assert_eq!(r.size_hint(), (10, Some(10)));
    }

    #[test]
    fn test_map() {
        let v = vec![1, 2, 3];
        let doubled: Vec<_> = v.iter().map(|x| *x * 2).collect();
        assert_eq!(doubled, vec![2, 4, 6]);
    }

    #[test]
    fn test_filter() {
        let v = vec![1, 2, 3, 4, 5];
        let evens: Vec<_> = v.iter().filter(|x| *x % 2 == 0).copied().collect();
        assert_eq!(evens, vec![2, 4]);
    }

    #[test]
    fn test_enumerate() {
        let v = vec!['a', 'b', 'c'];
        let indexed: Vec<_> = v.iter().copied().enumerate().collect();
        assert_eq!(indexed, vec![(0, 'a'), (1, 'b'), (2, 'c')]);
    }

    #[test]
    fn test_zip() {
        let a = vec![1, 2, 3];
        let b = vec![4, 5, 6];
        let zipped: Vec<_> = a.iter().zip(b.iter()).collect();
        assert_eq!(zipped, vec![(&1, &4), (&2, &5), (&3, &6)]);
    }

    #[test]
    fn test_take() {
        let v = vec![1, 2, 3, 4, 5];
        let taken: Vec<_> = v.iter().take(3).collect();
        assert_eq!(taken, vec![&1, &2, &3]);
    }

    #[test]
    fn test_skip() {
        let v = vec![1, 2, 3, 4, 5];
        let skipped: Vec<_> = v.iter().skip(2).collect();
        assert_eq!(skipped, vec![&3, &4, &5]);
    }

    #[test]
    fn test_peekable() {
        let v = vec![1, 2, 3];
        let mut peekable = v.iter().peekable();
        assert_eq!(peekable.peek(), Some(&&1));
        assert_eq!(peekable.next(), Some(&1));
    }

    #[test]
    fn test_find() {
        let v = vec![1, 2, 3, 4, 5];
        let result = v.iter().find(|x| **x > 3);
        assert_eq!(result, Some(&4));
    }

    #[test]
    fn test_count() {
        let v = vec![1, 2, 3, 4, 5];
        assert_eq!(v.iter().count(), 5);
    }

    #[test]
    fn test_any_all() {
        let v = vec![1, 2, 3, 4, 5];
        assert!(v.iter().any(|x| *x > 3));
        assert!(v.iter().all(|x| *x > 0));
        assert!(!v.iter().all(|x| *x > 3));
    }
}
