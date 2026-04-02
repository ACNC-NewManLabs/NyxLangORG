//! NYX Core Traits Module
//!
//! Core traits that form the foundation of the NYX type system.
//! These traits work without an OS and provide the basis for polymorphism.

use core::fmt;

// =============================================================================
// Marker Traits
// =============================================================================

/// Types whose values can be copied by simply copying bits.
///
/// This trait is automatically implemented for primitive types and
/// types that contain only Copy types. Unlike Clone, Copy is implicit
/// and doesn't require any explicit method calls.
pub trait Copy: Clone + 'static {}

/// A trait for types that can be duplicated by copying their data.
///
/// The Clone trait is used to create a deep copy of a value. It's
/// not implicit like Copy and requires an explicit method call.
pub trait Clone {
    /// Returns a copy of the value.
    fn clone(&self) -> Self;
}

/// A trait for types that can be dropped.
///
/// Drop is called when a value goes out of scope. It's primarily
/// used to release resources like memory, file handles, or network
/// connections.
pub trait Drop {
    /// Performs the destructor for this type.
    ///
    /// This method is called automatically when a value goes out of scope.
    fn drop(&mut self);
}

// =============================================================================
// Comparison Traits
// =============================================================================

/// Trait for equality comparison.
///
/// Types that implement Eq guarantee total equality, meaning:
/// - Reflexivity: a == a is always true
/// - Symmetry: a == b implies b == a
/// - Transitivity: a == b && b == c implies a == c
pub trait Eq: PartialEq<Self> {}

/// Trait for types that can be compared for equality.
///
/// This is a subtrait of Eq that provides additional guarantees:
/// - Non-reflexivity is possible (a != a is false)
pub trait PartialEq<Rhs: ?Sized = Self> {
    /// Returns true if the two values are equal.
    fn eq(&self, other: &Rhs) -> bool;

    /// Returns true if the two values are not equal.
    #[inline]
    fn ne(&self, other: &Rhs) -> bool {
        !self.eq(other)
    }
}

/// Trait for types that can be totally ordered.
///
/// Types that implement Ord have a total ordering, meaning:
/// - Transitivity: a < b && b < c implies a < c
/// - Trichotomy: exactly one of a < b, a == b, or a > b is true
pub trait Ord: Eq + PartialOrd<Self> {
    /// Returns the ordering between self and other.
    fn cmp(&self, other: &Self) -> Ordering;
}

/// Trait for types that can be partially ordered.
///
/// This is a subtrait of Eq for comparison operations that may not
/// be defined for all pairs.
pub trait PartialOrd<Rhs: ?Sized = Self>: PartialEq<Rhs> {
    /// Returns the ordering between self and other, if defined.
    fn partial_cmp(&self, other: &Rhs) -> Option<Ordering>;

    /// Returns true if self < other.
    #[inline]
    fn lt(&self, other: &Rhs) -> bool {
        matches!(self.partial_cmp(other), Option::Some(Ordering::Less))
    }

    /// Returns true if self > other.
    #[inline]
    fn gt(&self, other: &Rhs) -> bool {
        matches!(self.partial_cmp(other), Option::Some(Ordering::Greater))
    }

    /// Returns true if self <= other.
    #[inline]
    fn le(&self, other: &Rhs) -> bool {
        matches!(
            self.partial_cmp(other),
            Option::Some(Ordering::Less | Ordering::Equal)
        )
    }

    /// Returns true if self >= other.
    #[inline]
    fn ge(&self, other: &Rhs) -> bool {
        matches!(
            self.partial_cmp(other),
            Option::Some(Ordering::Greater | Ordering::Equal)
        )
    }
}

/// Represents the result of a comparison operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ordering {
    /// Less than.
    Less = -1,
    /// Equal.
    Equal = 0,
    /// Greater than.
    Greater = 1,
}

impl Ordering {
    /// Reverses the ordering.
    #[inline]
    pub fn reverse(self) -> Ordering {
        match self {
            Ordering::Less => Ordering::Greater,
            Ordering::Equal => Ordering::Equal,
            Ordering::Greater => Ordering::Less,
        }
    }

    /// Returns the lesser of two orderings.
    #[inline]
    pub fn min(a: Ordering, b: Ordering) -> Ordering {
        if a < b {
            a
        } else {
            b
        }
    }

    /// Returns the greater of two orderings.
    #[inline]
    pub fn max(a: Ordering, b: Ordering) -> Ordering {
        if a > b {
            a
        } else {
            b
        }
    }
}

// =============================================================================
// Formatting Traits
// =============================================================================

/// Trait for debug formatting.
///
/// Implementors can provide custom debug formatting for their types.
/// This is used by the {:?} format specifier.
pub trait Debug {
    /// Formats the value using the given formatter.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result;
}

/// Trait for user-facing display formatting.
///
/// Implementors can provide custom display formatting for their types.
/// This is used by the {} format specifier.
pub trait Display {
    /// Formats the value using the given formatter.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result;
}

// =============================================================================
// Hashing Traits
// =============================================================================

/// Trait for types that can be hashed.
///
/// This trait is used to compute a hash value for a type. The hash
/// should be consistent across executions but doesn't need to be
/// cryptographically secure.
pub trait Hash {
    /// Feeds the value into the state given, updating the hash.
    fn hash<H: Hasher>(&self, state: &mut H);
}

/// A trait for types that can be used in hashing.
///
/// Hasher provides the mechanism for computing the final hash value.
pub trait Hasher {
    /// Writes some bytes into the hasher.
    fn write(&mut self, bytes: &[u8]);

    /// Writes a single byte into the hasher.
    #[inline]
    fn write_u8(&mut self, byte: u8) {
        self.write(&[byte]);
    }

    /// Writes a 16-bit integer into the hasher.
    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.write(&i.to_le_bytes());
    }

    /// Writes a 32-bit integer into the hasher.
    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.write(&i.to_le_bytes());
    }

    /// Writes a 64-bit integer into the hasher.
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.write(&i.to_le_bytes());
    }

    /// Writes an isize into the hasher.
    #[inline]
    fn write_isize(&mut self, i: isize) {
        self.write_usize(i as usize);
    }

    /// Writes a usize into the hasher.
    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.write(&i.to_le_bytes());
    }

    /// Returns the hash result.
    fn finish(&self) -> u64;
}

/// A simple, fast, non-cryptographic hasher.
///
/// This is a simple implementation of Hasher suitable for non-security
/// critical hashing use cases.
pub struct SimpleHasher {
    state: u64,
}

impl SimpleHasher {
    /// Creates a new SimpleHasher.
    #[inline]
    pub fn new() -> SimpleHasher {
        SimpleHasher {
            state: 0xcbf29ce484222325,
        }
    }
}

impl Default for SimpleHasher {
    #[inline]
    fn default() -> SimpleHasher {
        SimpleHasher::new()
    }
}

impl Hasher for SimpleHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.state ^= byte as u64;
            self.state = self.state.wrapping_mul(0x100000001b3);
        }
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.state
    }
}

// =============================================================================
// Default and From/Into Traits
// =============================================================================

/// Trait for types with a default value.
///
/// The default value is typically some sensible value for the type.
pub trait Default {
    /// Returns the default value.
    fn default() -> Self;
}

/// Trait for converting a value to another type.
///
/// This is the reciprocal of From.
pub trait Into<T> {
    /// Converts the value to T.
    fn into(self) -> T;
}

/// Trait for converting from another type.
///
/// This is the reciprocal of Into.
pub trait From<T> {
    /// Converts the value to Self.
    fn from(value: T) -> Self;
}

// =============================================================================
// AsRef and AsMut Traits
// =============================================================================

/// Trait for converting to a reference.
pub trait AsRef<T: ?Sized> {
    /// Converts the value to a reference.
    fn as_ref(&self) -> &T;
}

/// Trait for converting to a mutable reference.
pub trait AsMut<T: ?Sized> {
    /// Converts the value to a mutable reference.
    fn as_mut(&mut self) -> &mut T;
}

// =============================================================================
// Borrow and BorrowMut Traits
// =============================================================================

/// Trait for borrowing immutably.
///
/// This trait is used for comparisons and to allow flexible borrowing
/// without taking ownership.
pub trait Borrow<Borrowed: ?Sized> {
    /// Borrows the value immutably.
    fn borrow(&self) -> &Borrowed;
}

/// Trait for borrowing mutably.
///
/// This trait is used to allow mutable borrows while still allowing
/// immutable borrows through the Borrow trait.
pub trait BorrowMut<Borrowed: ?Sized> {
    /// Borrows the value mutably.
    fn borrow_mut(&mut self) -> &mut Borrowed;
}

// =============================================================================
// Extend Trait
// =============================================================================

/// Trait for types that can be extended by an iterator.
pub trait Extend<A> {
    /// Extends a collection with the contents of an iterator.
    fn extend<I: crate::core::iter::IntoIterator<Item = A>>(&mut self, iter: I);
}

// =============================================================================
// Default Implementations
// =============================================================================

// =============================================================================
// Ordering Implementation
// =============================================================================

impl Eq for Ordering {}

impl PartialEq for Ordering {
    #[inline]
    fn eq(&self, other: &Ordering) -> bool {
        (*self as i32) == (*other as i32)
    }
}

impl Ord for Ordering {
    #[inline]
    fn cmp(&self, other: &Ordering) -> Ordering {
        // Directly compare the underlying i32 values
        if (*self as i32) < (*other as i32) {
            Ordering::Less
        } else if (*self as i32) > (*other as i32) {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}

impl PartialOrd for Ordering {
    #[inline]
    fn partial_cmp(&self, other: &Ordering) -> std::option::Option<Ordering> {
        std::option::Option::Some(crate::core::traits::Ord::cmp(self, other))
    }
}

impl Debug for Ordering {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ordering::Less => write!(f, "Less"),
            Ordering::Equal => write!(f, "Equal"),
            Ordering::Greater => write!(f, "Greater"),
        }
    }
}

// [T] does not implement Clone returning [T] because [T] is not Sized.

impl<T: ?Sized> AsRef<T> for T {
    #[inline]
    fn as_ref(&self) -> &T {
        self
    }
}

impl<T: ?Sized> AsMut<T> for T {
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        self
    }
}

impl<T> From<T> for T {
    #[inline]
    fn from(value: T) -> T {
        value
    }
}

impl<T> Into<T> for T {
    #[inline]
    fn into(self) -> T {
        self
    }
}

// =============================================================================
// Standard implementations for primitives
// =============================================================================

macro_rules! impl_traits_for_primitives {
    ($($ty:ty = $default:expr),*) => {
        $(
            impl Clone for $ty {
                #[inline]
                fn clone(&self) -> $ty {
                    *self
                }
            }

            impl Copy for $ty {}

            impl PartialEq for $ty {
                #[inline]
                fn eq(&self, other: &$ty) -> bool {
                    *self == *other
                }
            }

            impl Eq for $ty {}

            impl PartialOrd for $ty {
                #[inline]
                fn partial_cmp(&self, other: &$ty) -> Option<Ordering> {
                    if *self < *other {
                        Some(Ordering::Less)
                    } else if *self > *other {
                        Some(Ordering::Greater)
                    } else {
                        Some(Ordering::Equal)
                    }
                }
            }

            impl Ord for $ty {
                #[inline]
                fn cmp(&self, other: &$ty) -> Ordering {
                    if *self < *other {
                        Ordering::Less
                    } else if *self > *other {
                        Ordering::Greater
                    } else {
                        Ordering::Equal
                    }
                }
            }

            impl Default for $ty {
                #[inline]
                fn default() -> $ty {
                    $default
                }
            }
        )*
    };
}

impl_traits_for_primitives!(
    u8 = 0,
    u16 = 0,
    u32 = 0,
    u64 = 0,
    u128 = 0,
    usize = 0,
    i8 = 0,
    i16 = 0,
    i32 = 0,
    i64 = 0,
    i128 = 0,
    isize = 0,
    f32 = 0.0,
    f64 = 0.0,
    bool = false,
    char = '\0'
);

// =============================================================================
// Standard implementations for slices
// =============================================================================

// =============================================================================
// Vec type for internal use
// =============================================================================

/// Simple Vec implementation for core library use.
pub struct Vec<T> {
    ptr: *mut T,
    len: usize,
    cap: usize,
}

// Cleanup remaining orphan impls if any

impl<T> Vec<T> {
    /// Creates a new Vec with the given capacity.
    #[inline]
    pub fn with_capacity(cap: usize) -> Vec<T> {
        if cap == 0 {
            Vec {
                ptr: core::ptr::NonNull::dangling().as_ptr(),
                len: 0,
                cap: 0,
            }
        } else {
            let layout = core::alloc::Layout::array::<T>(cap).unwrap();
            let ptr = unsafe { std::alloc::alloc(layout) } as *mut T;
            Vec { ptr, len: 0, cap }
        }
    }

    /// Returns the length of the Vec.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the Vec is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Pushes a value onto the Vec.
    #[inline]
    pub fn push(&mut self, value: T) {
        if self.len >= self.cap {
            let new_cap = if self.cap == 0 { 1 } else { self.cap * 2 };
            let new_layout = core::alloc::Layout::array::<T>(new_cap).unwrap();
            let new_ptr = unsafe {
                std::alloc::realloc(
                    self.ptr as *mut u8,
                    core::alloc::Layout::array::<T>(self.cap).unwrap(),
                    new_layout.size(),
                )
            } as *mut T;
            self.ptr = new_ptr;
            self.cap = new_cap;
        }
        unsafe {
            core::ptr::write(self.ptr.add(self.len), value);
        }
        self.len += 1;
    }

    /// Converts the Vec into a raw pointer and length.
    #[inline]
    pub fn into_raw_parts(self) -> (*mut T, usize, usize) {
        let ptr = self.ptr;
        let len = self.len;
        let cap = self.cap;
        core::mem::forget(self);
        (ptr, len, cap)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ordering_cmp() {
        assert_eq!(
            crate::core::traits::Ord::cmp(&Ordering::Less, &Ordering::Less),
            Ordering::Equal
        );
        assert_eq!(
            crate::core::traits::Ord::cmp(&Ordering::Less, &Ordering::Greater),
            Ordering::Less
        );
        assert_eq!(
            crate::core::traits::Ord::cmp(&Ordering::Greater, &Ordering::Less),
            Ordering::Greater
        );
    }

    #[test]
    fn test_simple_hasher() {
        let mut hasher = SimpleHasher::new();
        hasher.write(b"test");
        let result = hasher.finish();
        assert_ne!(result, 0);
    }

    #[test]
    fn test_primitives_implement_traits() {
        let a: i32 = 5;
        let b = crate::core::traits::Clone::clone(&a);
        assert_eq!(a, b);

        let c: i32 = Default::default();
        assert_eq!(c, 0);
    }

    #[test]
    fn test_vec() {
        let mut v: Vec<i32> = Vec::with_capacity(4);
        v.push(1);
        v.push(2);
        v.push(3);
        assert_eq!(v.len(), 3);
    }
}
