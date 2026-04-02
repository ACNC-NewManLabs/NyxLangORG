//! Option type implementation
//! 
//! A type that represents an optional value: either Some(T) or None.

use crate::core::{Iterator, IntoIterator};

/// Option type - represents an optional value
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Default)]
pub enum Option<T> {
    /// Some value
    Some(T),
    /// No value
    #[default]
    None,
}

impl<T> Option<T> {
    /// Returns true if the option is a Some value
    #[inline]
    pub fn is_some(&self) -> bool {
        match self {
            Option::Some(_) => true,
            Option::None => false,
        }
    }

    /// Returns true if the option is a None value
    #[inline]
    pub fn is_none(&self) -> bool {
        !self.is_some()
    }

    /// Returns the contained Some value
    /// 
    /// # Panics
    /// Panics if the value is None
    #[inline]
    pub fn unwrap(self) -> T {
        match self {
            Option::Some(v) => v,
            Option::None => panic!("called Option::unwrap() on a None value"),
        }
    }

    /// Returns the contained Some value or a provided default
    #[inline]
    pub fn expect(self, msg: &str) -> T
    where
        T: Debug,
    {
        match self {
            Option::Some(v) => v,
            Option::None => panic!("{}", msg),
        }
    }

    /// Returns the contained Some value or a provided default
    #[inline]
    pub fn unwrap_or(self, default: T) -> T {
        match self {
            Option::Some(v) => v,
            Option::None => default,
        }
    }

    /// Returns the contained Some value or computes it from a closure
    #[inline]
    pub fn unwrap_or_else<F: FnOnce() -> T>(self, f: F) -> T {
        match self {
            Option::Some(v) => v,
            Option::None => f(),
        }
    }

    /// Maps an Option<T> to Option<U> by applying a function to a contained value
    #[inline]
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Option<U> {
        match self {
            Option::Some(v) => Option::Some(f(v)),
            Option::None => Option::None,
        }
    }

    /// Maps an Option<T> to Option<U> by applying a function to a contained value,
    /// returning None if the option is None
    #[inline]
    pub fn and_then<U, F: FnOnce(T) -> Option<U>>(self, f: F) -> Option<U> {
        match self {
            Option::Some(v) => f(v),
            Option::None => Option::None,
        }
    }

    /// Takes the value out of the option, leaving a None in its place
    #[inline]
    pub fn take(&mut self) -> Option<T> {
        match core::mem::replace(self, Option::None) {
            Option::Some(v) => Option::Some(v),
            Option::None => Option::None,
        }
    }

    /// Returns None if the option is None, otherwise returns the default
    #[inline]
    pub fn and<U>(self, optb: Option<U>) -> Option<U> {
        match self {
            Option::Some(_) => optb,
            Option::None => Option::None,
        }
    }

    /// Returns the option if it contains a value, otherwise returns the provided option
    #[inline]
    pub fn or(self, optb: Option<T>) -> Option<T> {
        match self {
            Option::Some(_) => self,
            Option::None => optb,
        }
    }

    /// Returns the option if it contains a value, otherwise calls f and returns the result
    #[inline]
    pub fn or_else<F: FnOnce() -> Option<T>>(self, f: F) -> Option<T> {
        match self {
            Option::Some(_) => self,
            Option::None => f(),
        }
    }

    /// Converts from Option<Option<T>> to Option<T>
    #[inline]
    pub fn flatten(self) -> Option<T>
    where
        T: IntoOption<Inner = T>,
    {
        match self {
            Option::Some(v) => v.into_option(),
            Option::None => Option::None,
        }
    }

    /// Converts from &Option<T> to Option<&T>
    #[inline]
    pub fn as_ref(&self) -> Option<&T> {
        match self {
            Option::Some(v) => Option::Some(v),
            Option::None => Option::None,
        }
    }

    /// Converts from &mut Option<T> to Option<&mut T>
    #[inline]
    pub fn as_mut(&mut self) -> Option<&mut T> {
        match self {
            Option::Some(v) => Option::Some(v),
            Option::None => Option::None,
        }
    }

    /// Returns the inner T if Some, or a default value if None
    #[inline]
    pub fn inner_or_default(self) -> T
    where
        T: Default,
    {
        match self {
            Option::Some(v) => v,
            Option::None => T::default(),
        }
    }
}

// Manual Clone impl removed

impl<T: Copy> Option<T> {
    /// Copies the contained value if Some, or returns None
    #[inline]
    pub fn copied(&self) -> Option<T> {
        match self {
            Option::Some(v) => Option::Some(*v),
            Option::None => Option::None,
        }
    }
}

impl<T> IntoIterator for Option<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Option::Some(v) => crate::core::option::IntoIter { inner: Option::Some(v) },
            Option::None => crate::core::option::IntoIter { inner: Option::None },
        }
    }
}

/// IntoIterator for Option
pub struct IntoIter<T> {
    inner: Option<T>,
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> std::option::Option<Self::Item> {
        match self.inner.take() {
            crate::core::option::Option::Some(v) => std::option::Option::Some(v),
            crate::core::option::Option::None => std::option::Option::None,
        }
    }

    fn size_hint(&self) -> (usize, std::option::Option<usize>) {
        match self.inner {
            Option::Some(_) => (1, std::option::Option::Some(1)),
            Option::None => (0, std::option::Option::Some(0)),
        }
    }
}

impl<T: Clone> Clone for IntoIter<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> crate::core::traits::Extend<T> for Option<T> {
    fn extend<I: crate::core::iter::IntoIterator<Item = T>>(&mut self, iter: I) {
        if self.is_none() {
            let mut iter = iter.into_iter();
            if let std::option::Option::Some(item) = iter.next() {
                *self = Option::Some(item);
            }
        }
    }
}


/// Trait for types that can be converted to Option
pub trait IntoOption {
    type Inner;
    fn into_option(self) -> Option<Self::Inner>;
}

impl<T> IntoOption for Option<T> {
    type Inner = T;
    fn into_option(self) -> Option<T> {
        self
    }
}

// Implement Debug for Option
impl<T: Debug> Debug for Option<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Option::Some(v) => write!(f, "Some({:?})", v),
            Option::None => write!(f, "None"),
        }
    }
}

// Implement Display for Option
impl<T: Display> Display for Option<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Option::Some(v) => write!(f, "Some({})", v),
            Option::None => write!(f, "None"),
        }
    }
}

// Implement PartialOrd for Option
impl<T: PartialOrd> PartialOrd for Option<T> {
    fn partial_cmp(&self, other: &Self) -> std::option::Option<Ordering> {
        match (self, other) {
            (Option::Some(a), Option::Some(b)) => a.partial_cmp(b),
            (Option::Some(_), Option::None) => std::option::Option::Some(Ordering::Greater),
            (Option::None, Option::Some(_)) => std::option::Option::Some(Ordering::Less),
            (Option::None, Option::None) => std::option::Option::Some(Ordering::Equal),
        }
    }
}

// Implement Ord for Option
impl<T: Ord> Ord for Option<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Option::Some(a), Option::Some(b)) => a.cmp(b),
            (Option::Some(_), Option::None) => Ordering::Greater,
            (Option::None, Option::Some(_)) => Ordering::Less,
            (Option::None, Option::None) => Ordering::Equal,
        }
    }
}

use core::cmp::Ordering;
use core::fmt::{Debug, Display};
