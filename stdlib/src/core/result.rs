//! Result type implementation
//!
//! A type that represents either success (Ok(T)) or failure (Err(E)).

// Internal traits already in scope

/// Result type - represents success or failure
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Result<T, E> {
    /// Success value
    Ok(T),
    /// Error value
    Err(E),
}

impl<T, E> Result<T, E> {
    /// Returns true if the result is Ok
    #[inline]
    pub fn is_ok(&self) -> bool {
        match self {
            Result::Ok(_) => true,
            Result::Err(_) => false,
        }
    }

    /// Returns true if the result is Err
    #[inline]
    pub fn is_err(&self) -> bool {
        !self.is_ok()
    }

    /// Returns the contained Ok value
    ///
    /// # Panics
    /// Panics if the value is Err
    #[inline]
    pub fn expect(self, msg: &str) -> T
    where
        E: core::fmt::Debug,
    {
        match self {
            Result::Ok(v) => v,
            Result::Err(e) => panic!("{}: {:?}", msg, e),
        }
    }

    pub fn expect_err(self, msg: &str) -> E
    where
        T: core::fmt::Debug,
    {
        match self {
            Result::Ok(v) => panic!("{}: {:?}", msg, v),
            Result::Err(e) => e,
        }
    }

    pub fn unwrap(self) -> T
    where
        E: core::fmt::Debug,
    {
        match self {
            Result::Ok(v) => v,
            Result::Err(e) => panic!("called `Result::unwrap()` on an `Err` value: {:?}", e),
        }
    }

    pub fn unwrap_err(self) -> E
    where
        T: core::fmt::Debug,
    {
        match self {
            Result::Ok(v) => panic!("called `Result::unwrap_err()` on an `Ok` value: {:?}", v),
            Result::Err(e) => e,
        }
    }

    /// Returns the contained Ok value or a provided default
    #[inline]
    pub fn unwrap_or(self, default: T) -> T {
        match self {
            Result::Ok(v) => v,
            Result::Err(_) => default,
        }
    }

    /// Returns the contained Ok value or computes it from a closure
    #[inline]
    pub fn unwrap_or_else<F: FnOnce(E) -> T>(self, op: F) -> T {
        match self {
            Result::Ok(v) => v,
            Result::Err(e) => op(e),
        }
    }

    /// Maps a Result<T, E> to Result<U, E> by applying a function to a contained Ok value
    #[inline]
    pub fn map<U, F: FnOnce(T) -> U>(self, op: F) -> Result<U, E> {
        match self {
            Result::Ok(v) => Result::Ok(op(v)),
            Result::Err(e) => Result::Err(e),
        }
    }

    /// Maps a Result<T, E> to Result<T, F> by applying a function to a contained Err value
    #[inline]
    pub fn map_err<F, O: FnOnce(E) -> F>(self, op: O) -> Result<T, F> {
        match self {
            Result::Ok(v) => Result::Ok(v),
            Result::Err(e) => Result::Err(op(e)),
        }
    }

    /// Returns the contained Ok value or None if the result is Err
    #[inline]
    pub fn ok(self) -> Option<T> {
        match self {
            Result::Ok(v) => Option::Some(v),
            Result::Err(_) => Option::None,
        }
    }

    /// Returns the contained Err value or None if the result is Ok
    #[inline]
    pub fn err(self) -> Option<E> {
        match self {
            Result::Ok(_) => Option::None,
            Result::Err(e) => crate::core::option::Option::Some(e),
        }
    }

    /// Returns res if the result is Ok, otherwise returns the Err value of self
    #[inline]
    pub fn and<U>(self, res: Result<U, E>) -> Result<U, E> {
        match self {
            Result::Ok(_) => res,
            Result::Err(e) => Result::Err(e),
        }
    }

    /// Calls op if the result is Ok, otherwise returns the Err value of self
    #[inline]
    pub fn and_then<U, F: FnOnce(T) -> Result<U, E>>(self, op: F) -> Result<U, E> {
        match self {
            Result::Ok(v) => op(v),
            Result::Err(e) => Result::Err(e),
        }
    }

    /// Returns res if the result is Err, otherwise returns the Ok value of self
    #[inline]
    pub fn or<F>(self, res: Result<T, F>) -> Result<T, F> {
        match self {
            Result::Ok(v) => Result::Ok(v),
            Result::Err(_) => res,
        }
    }

    /// Calls op if the result is Err, otherwise returns the Ok value of self
    #[inline]
    pub fn or_else<F, O: FnOnce(E) -> Result<T, F>>(self, op: O) -> Result<T, F> {
        match self {
            Result::Ok(v) => Result::Ok(v),
            Result::Err(e) => op(e),
        }
    }

    /// Converts from Result<Result<T, E>, E> to Result<T, E>
    #[inline]
    pub fn flatten(self) -> Result<T, E>
    where
        T: IntoResult<Ok = T, Err = E>,
    {
        match self {
            Result::Ok(v) => v.into_result(),
            Result::Err(e) => Result::Err(e),
        }
    }

    /// Converts from &Result<T, E> to Result<&T, &E>
    #[inline]
    pub fn as_ref(&self) -> Result<&T, &E> {
        match self {
            Result::Ok(v) => Result::Ok(v),
            Result::Err(e) => Result::Err(e),
        }
    }

    /// Converts from &mut Result<T, E> to Result<&mut T, &mut E>
    #[inline]
    pub fn as_mut(&mut self) -> Result<&mut T, &mut E> {
        match self {
            Result::Ok(v) => Result::Ok(v),
            Result::Err(e) => Result::Err(e),
        }
    }

    /// Transforms the Result<T, E>'s Ok value by applying a function to a contained Ok value
    /// while preserving the Err value
    #[inline]
    pub fn inspect<F: FnOnce(&T)>(self, f: F) -> Self {
        if let Result::Ok(ref v) = self {
            f(v);
        }
        self
    }

    /// Transforms the Result<T, E>'s Err value by applying a function to a contained Err value
    /// while preserving the Ok value
    #[inline]
    pub fn inspect_err<F: FnOnce(&E)>(self, f: F) -> Self {
        if let Result::Err(ref e) = self {
            f(e);
        }
        self
    }

    /// Returns the contained Ok value, without checking if the result is Ok
    ///
    /// # Safety
    /// Calling this method on Err is undefined behavior
    #[inline]
    pub unsafe fn unwrap_unchecked(self) -> T {
        debug_assert!(self.is_ok());
        match self {
            Result::Ok(v) => v,
            Result::Err(_) => core::hint::unreachable_unchecked(),
        }
    }

    /// Returns the contained Err value, without checking if the result is Err
    ///
    /// # Safety
    /// Calling this method on Ok is undefined behavior
    #[inline]
    pub unsafe fn unwrap_err_unchecked(self) -> E {
        debug_assert!(self.is_err());
        match self {
            Result::Ok(_) => core::hint::unreachable_unchecked(),
            Result::Err(e) => e,
        }
    }
}

// Manual Clone impl removed (replaced by derive)

impl<T, E> Result<T, E> {
    /// Maps a Result<T, E> to Result<U, E> by cloning the Ok value and applying the function
    #[inline]
    pub fn map_or<U, F: FnOnce(T) -> U>(self, default: U, f: F) -> U {
        match self {
            Result::Ok(v) => f(v),
            Result::Err(_) => default,
        }
    }
}

// Orphan Debug impl removed

impl<T: Display, E: Display> Display for Result<T, E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Result::Ok(v) => write!(f, "Ok({})", v),
            Result::Err(e) => write!(f, "Err({})", e),
        }
    }
}

impl<T> IntoResult for T {
    type Ok = T;
    type Err = ();
    fn into_result(self) -> Result<T, ()> {
        Result::Ok(self)
    }
}

impl<T, E> IntoIterator for Result<T, E> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Result::Ok(v) => crate::core::result::IntoIter {
                inner: Option::Some(v),
            },
            Result::Err(_) => crate::core::result::IntoIter {
                inner: Option::None,
            },
        }
    }
}

/// IntoIterator for Result
pub struct IntoIter<T> {
    inner: Option<T>,
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> std::option::Option<Self::Item> {
        match self.inner.take() {
            crate::core::option::Option::Some(v) => std::option::Option::Some(v),
            _ => std::option::Option::None,
        }
    }

    fn size_hint(&self) -> (usize, std::option::Option<usize>) {
        match self.inner {
            crate::core::option::Option::Some(_) => (1, std::option::Option::Some(1)),
            _ => (0, std::option::Option::Some(0)),
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

impl<T> Extend<T> for IntoIter<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        if self.inner.is_none() {
            let mut iter = iter.into_iter();
            if let std::option::Option::Some(item) = iter.next() {
                self.inner = crate::core::option::Option::Some(item);
            }
        }
    }
}

impl<T: crate::core::traits::Default> Default for Result<T, ()> {
    fn default() -> Self {
        Result::Ok(T::default())
    }
}

/// Trait for types that can be converted to Result
pub trait IntoResult {
    type Ok;
    type Err;
    fn into_result(self) -> Result<Self::Ok, Self::Err>;
}

// Redundant impl removed

use crate::core::option::Option;

use core::fmt::Display;
