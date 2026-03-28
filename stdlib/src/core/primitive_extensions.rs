//! NYX Core Primitive Extensions Module
//! 
//! Extension methods for primitive types that work without an OS.



// =============================================================================
// Integer Extensions
// =============================================================================

/// Extension methods for unsigned integers.
pub trait UIntExt: Sized + Copy {
    /// Returns the number of ones in the binary representation.
    fn popcount(self) -> usize;

    /// Returns the number of leading zeros.
    fn leading_zeros(self) -> usize;

    /// Returns the number of trailing zeros.
    fn trailing_zeros(self) -> usize;

    /// Returns true if the number is a power of two.
    fn is_power_of_two(self) -> bool;

    /// Returns the next power of two, or None if it would overflow.
    fn next_power_of_two(self) -> crate::core::option::Option<Self>;

    /// Returns the number of bytes needed to represent the number.
    fn bytes_needed(self) -> usize;

    /// Wrapping addition.
    fn wrapping_add(self, rhs: Self) -> Self;

    /// Wrapping subtraction.
    fn wrapping_sub(self, rhs: Self) -> Self;

    /// Wrapping multiplication.
    fn wrapping_mul(self, rhs: Self) -> Self;

    /// Saturating addition.
    fn saturating_add(self, rhs: Self) -> Self;

    /// Saturating subtraction.
    fn saturating_sub(self, rhs: Self) -> Self;
}

/// Extension methods for signed integers.
pub trait SIntExt: Sized + Copy + PartialOrd {
    /// Returns the number of ones in the binary representation.
    fn popcount(self) -> usize;

    /// Returns the number of leading zeros.
    fn leading_zeros(self) -> usize;

    /// Returns the number of trailing zeros.
    fn trailing_zeros(self) -> usize;

    /// Returns the absolute value, saturating at Overflow.
    fn abs(self) -> Self;

    /// Returns true if the number is positive.
    fn is_positive(self) -> bool;

    /// Returns true if the number is negative.
    fn is_negative(self) -> bool;

    /// Returns the sign of the number: -1, 0, or 1.
    fn signum(self) -> Self;

    /// Wrapping addition.
    fn wrapping_add(self, rhs: Self) -> Self;

    /// Wrapping subtraction.
    fn wrapping_sub(self, rhs: Self) -> Self;

    /// Wrapping multiplication.
    fn wrapping_mul(self, rhs: Self) -> Self;

    /// Saturating addition.
    fn saturating_add(self, rhs: Self) -> Self;

    /// Saturating subtraction.
    fn saturating_sub(self, rhs: Self) -> Self;

    /// Returns self/2 rounded toward negative infinity.
    fn div_floor(self, rhs: Self) -> Self;
}

// =============================================================================
// U8 Extensions
// =============================================================================

impl UIntExt for u8 {
    #[inline]
    fn popcount(self) -> usize {
        (self as usize).count_ones() as usize
    }

    #[inline]
    fn leading_zeros(self) -> usize {
        (self as usize).leading_zeros() as usize
    }

    #[inline]
    fn trailing_zeros(self) -> usize {
        (self as usize).trailing_zeros() as usize
    }

    #[inline]
    fn is_power_of_two(self) -> bool {
        self != 0 && (self & (self - 1)) == 0
    }

    #[inline]
    fn next_power_of_two(self) -> crate::core::option::Option<u8> {
        if self == 0 {
            return crate::core::option::Option::Some(1);
        }
        if self > u8::MAX / 2 + 1 {
            return crate::core::option::Option::None;
        }
        crate::core::option::Option::Some(1 << (8 - self.leading_zeros() as u8))
    }

    #[inline]
    fn bytes_needed(self) -> usize {
        if self == 0 { return 1; }
        ((self as f64).log2().floor() as usize) / 8 + 1
    }

    #[inline]
    fn wrapping_add(self, rhs: Self) -> Self {
        self.wrapping_add(rhs)
    }

    #[inline]
    fn wrapping_sub(self, rhs: Self) -> Self {
        self.wrapping_sub(rhs)
    }

    #[inline]
    fn wrapping_mul(self, rhs: Self) -> Self {
        self.wrapping_mul(rhs)
    }

    #[inline]
    fn saturating_add(self, rhs: Self) -> Self {
        self.saturating_add(rhs)
    }

    #[inline]
    fn saturating_sub(self, rhs: Self) -> Self {
        self.saturating_sub(rhs)
    }
}

impl UIntExt for u16 {
    #[inline]
    fn popcount(self) -> usize {
        (self as usize).count_ones() as usize
    }

    #[inline]
    fn leading_zeros(self) -> usize {
        (self as usize).leading_zeros() as usize
    }

    #[inline]
    fn trailing_zeros(self) -> usize {
        (self as usize).trailing_zeros() as usize
    }

    #[inline]
    fn is_power_of_two(self) -> bool {
        self != 0 && (self & (self - 1)) == 0
    }

    #[inline]
    fn next_power_of_two(self) -> crate::core::option::Option<u16> {
        if self == 0 {
            return crate::core::option::Option::Some(1);
        }
        if self > u16::MAX / 2 + 1 {
            return crate::core::option::Option::None;
        }
        crate::core::option::Option::Some(1 << (16 - self.leading_zeros() as u16))
    }

    #[inline]
    fn bytes_needed(self) -> usize {
        if self == 0 { return 1; }
        ((self as f64).log2().floor() as usize) / 8 + 1
    }

    #[inline]
    fn wrapping_add(self, rhs: Self) -> Self {
        self.wrapping_add(rhs)
    }

    #[inline]
    fn wrapping_sub(self, rhs: Self) -> Self {
        self.wrapping_sub(rhs)
    }

    #[inline]
    fn wrapping_mul(self, rhs: Self) -> Self {
        self.wrapping_mul(rhs)
    }

    #[inline]
    fn saturating_add(self, rhs: Self) -> Self {
        self.saturating_add(rhs)
    }

    #[inline]
    fn saturating_sub(self, rhs: Self) -> Self {
        self.saturating_sub(rhs)
    }
}

impl UIntExt for u32 {
    #[inline]
    fn popcount(self) -> usize {
        self.count_ones() as usize
    }

    #[inline]
    fn leading_zeros(self) -> usize {
        self.leading_zeros() as usize
    }

    #[inline]
    fn trailing_zeros(self) -> usize {
        self.trailing_zeros() as usize
    }

    #[inline]
    fn is_power_of_two(self) -> bool {
        self != 0 && (self & (self - 1)) == 0
    }

    #[inline]
    fn next_power_of_two(self) -> crate::core::option::Option<u32> {
        if self == 0 {
            return crate::core::option::Option::Some(1);
        }
        if self > u32::MAX / 2 + 1 {
            return crate::core::option::Option::None;
        }
        crate::core::option::Option::Some(1 << (32 - self.leading_zeros()))
    }

    #[inline]
    fn bytes_needed(self) -> usize {
        if self == 0 { return 1; }
        ((self as f64).log2().floor() as usize) / 8 + 1
    }

    #[inline]
    fn wrapping_add(self, rhs: Self) -> Self {
        self.wrapping_add(rhs)
    }

    #[inline]
    fn wrapping_sub(self, rhs: Self) -> Self {
        self.wrapping_sub(rhs)
    }

    #[inline]
    fn wrapping_mul(self, rhs: Self) -> Self {
        self.wrapping_mul(rhs)
    }

    #[inline]
    fn saturating_add(self, rhs: Self) -> Self {
        self.saturating_add(rhs)
    }

    #[inline]
    fn saturating_sub(self, rhs: Self) -> Self {
        self.saturating_sub(rhs)
    }
}

impl UIntExt for u64 {
    #[inline]
    fn popcount(self) -> usize {
        self.count_ones() as usize
    }

    #[inline]
    fn leading_zeros(self) -> usize {
        self.leading_zeros() as usize
    }

    #[inline]
    fn trailing_zeros(self) -> usize {
        self.trailing_zeros() as usize
    }

    #[inline]
    fn is_power_of_two(self) -> bool {
        self != 0 && (self & (self - 1)) == 0
    }

    #[inline]
    fn next_power_of_two(self) -> crate::core::option::Option<u64> {
        if self == 0 {
            return crate::core::option::Option::Some(1);
        }
        if self > u64::MAX / 2 + 1 {
            return crate::core::option::Option::None;
        }
        crate::core::option::Option::Some(1 << (64 - self.leading_zeros()))
    }

    #[inline]
    fn bytes_needed(self) -> usize {
        if self == 0 { return 1; }
        ((self as f64).log2().floor() as usize) / 8 + 1
    }

    #[inline]
    fn wrapping_add(self, rhs: Self) -> Self {
        self.wrapping_add(rhs)
    }

    #[inline]
    fn wrapping_sub(self, rhs: Self) -> Self {
        self.wrapping_sub(rhs)
    }

    #[inline]
    fn wrapping_mul(self, rhs: Self) -> Self {
        self.wrapping_mul(rhs)
    }

    #[inline]
    fn saturating_add(self, rhs: Self) -> Self {
        self.saturating_add(rhs)
    }

    #[inline]
    fn saturating_sub(self, rhs: Self) -> Self {
        self.saturating_sub(rhs)
    }
}

impl UIntExt for usize {
    #[inline]
    fn popcount(self) -> usize {
        self.count_ones() as usize
    }

    #[inline]
    fn leading_zeros(self) -> usize {
        self.leading_zeros() as usize
    }

    #[inline]
    fn trailing_zeros(self) -> usize {
        self.trailing_zeros() as usize
    }

    #[inline]
    fn is_power_of_two(self) -> bool {
        self != 0 && (self & (self - 1)) == 0
    }

    #[inline]
    fn next_power_of_two(self) -> crate::core::option::Option<usize> {
        if self == 0 {
            return crate::core::option::Option::Some(1);
        }
        if self > usize::MAX / 2 + 1 {
            return crate::core::option::Option::None;
        }
        crate::core::option::Option::Some(1 << (usize::BITS - self.leading_zeros() as u32))
    }

    #[inline]
    fn bytes_needed(self) -> usize {
        if self == 0 { return 1; }
        ((self as f64).log2().floor() as usize) / 8 + 1
    }

    #[inline]
    fn wrapping_add(self, rhs: Self) -> Self {
        self.wrapping_add(rhs)
    }

    #[inline]
    fn wrapping_sub(self, rhs: Self) -> Self {
        self.wrapping_sub(rhs)
    }

    #[inline]
    fn wrapping_mul(self, rhs: Self) -> Self {
        self.wrapping_mul(rhs)
    }

    #[inline]
    fn saturating_add(self, rhs: Self) -> Self {
        self.saturating_add(rhs)
    }

    #[inline]
    fn saturating_sub(self, rhs: Self) -> Self {
        self.saturating_sub(rhs)
    }
}

// =============================================================================
// Signed Integer Extensions
// =============================================================================

impl SIntExt for i8 {
    #[inline]
    fn popcount(self) -> usize {
        (self as u8).popcount()
    }

    #[inline]
    fn leading_zeros(self) -> usize {
        (self as u8).leading_zeros() as usize
    }

    fn trailing_zeros(self) -> usize {
        (self as u8).trailing_zeros() as usize
    }

    #[inline]
    fn abs(self) -> i8 {
        self.abs()
    }

    #[inline]
    fn is_positive(self) -> bool {
        self > 0
    }

    #[inline]
    fn is_negative(self) -> bool {
        self < 0
    }

    #[inline]
    fn signum(self) -> i8 {
        self.signum()
    }

    #[inline]
    fn wrapping_add(self, rhs: Self) -> Self {
        self.wrapping_add(rhs)
    }

    #[inline]
    fn wrapping_sub(self, rhs: Self) -> Self {
        self.wrapping_sub(rhs)
    }

    #[inline]
    fn wrapping_mul(self, rhs: Self) -> Self {
        self.wrapping_mul(rhs)
    }

    #[inline]
    fn saturating_add(self, rhs: Self) -> Self {
        self.saturating_add(rhs)
    }

    #[inline]
    fn saturating_sub(self, rhs: Self) -> Self {
        self.saturating_sub(rhs)
    }

    #[inline]
    fn div_floor(self, rhs: Self) -> Self {
        let q = self / rhs;
        let r = self % rhs;
        if (r != 0) && ((r > 0) != (rhs > 0)) {
            q - 1
        } else {
            q
        }
    }
}

impl SIntExt for i32 {
    #[inline]
    fn popcount(self) -> usize {
        (self as u32).popcount()
    }

    #[inline]
    fn leading_zeros(self) -> usize {
        (self as u32).leading_zeros() as usize
    }

    #[inline]
    fn trailing_zeros(self) -> usize {
        (self as u32).trailing_zeros() as usize
    }

    #[inline]
    fn abs(self) -> i32 {
        self.abs()
    }

    #[inline]
    fn is_positive(self) -> bool {
        self > 0
    }

    #[inline]
    fn is_negative(self) -> bool {
        self < 0
    }

    #[inline]
    fn signum(self) -> i32 {
        self.signum()
    }

    #[inline]
    fn wrapping_add(self, rhs: Self) -> Self {
        self.wrapping_add(rhs)
    }

    #[inline]
    fn wrapping_sub(self, rhs: Self) -> Self {
        self.wrapping_sub(rhs)
    }

    #[inline]
    fn wrapping_mul(self, rhs: Self) -> Self {
        self.wrapping_mul(rhs)
    }

    #[inline]
    fn saturating_add(self, rhs: Self) -> Self {
        self.saturating_add(rhs)
    }

    #[inline]
    fn saturating_sub(self, rhs: Self) -> Self {
        self.saturating_sub(rhs)
    }

    #[inline]
    fn div_floor(self, rhs: Self) -> Self {
        let q = self / rhs;
        let r = self % rhs;
        if (r != 0) && ((r > 0) != (rhs > 0)) {
            q - 1
        } else {
            q
        }
    }
}

impl SIntExt for i64 {
    #[inline]
    fn popcount(self) -> usize {
        (self as u64).popcount()
    }

    #[inline]
    fn leading_zeros(self) -> usize {
        (self as u64).leading_zeros() as usize
    }

    #[inline]
    fn trailing_zeros(self) -> usize {
        (self as u64).trailing_zeros() as usize
    }

    #[inline]
    fn abs(self) -> i64 {
        self.abs()
    }

    #[inline]
    fn is_positive(self) -> bool {
        self > 0
    }

    #[inline]
    fn is_negative(self) -> bool {
        self < 0
    }

    #[inline]
    fn signum(self) -> i64 {
        self.signum()
    }

    #[inline]
    fn wrapping_add(self, rhs: Self) -> Self {
        self.wrapping_add(rhs)
    }

    #[inline]
    fn wrapping_sub(self, rhs: Self) -> Self {
        self.wrapping_sub(rhs)
    }

    #[inline]
    fn wrapping_mul(self, rhs: Self) -> Self {
        self.wrapping_mul(rhs)
    }

    #[inline]
    fn saturating_add(self, rhs: Self) -> Self {
        self.saturating_add(rhs)
    }

    #[inline]
    fn saturating_sub(self, rhs: Self) -> Self {
        self.saturating_sub(rhs)
    }

    #[inline]
    fn div_floor(self, rhs: Self) -> Self {
        let q = self / rhs;
        let r = self % rhs;
        if (r != 0) && ((r > 0) != (rhs > 0)) {
            q - 1
        } else {
            q
        }
    }
}

// =============================================================================
// Float Extensions
// =============================================================================

/// Extension methods for floating-point numbers.
pub trait FloatExt: Sized + Copy {
    /// Returns the maximum finite value.
    fn max_value() -> Self;

    /// Returns the minimum finite value.
    fn min_value() -> Self;

    /// Returns positive infinity.
    fn infinity() -> Self;

    /// Returns negative infinity.
    fn neg_infinity() -> Self;

    /// Returns NaN (not a number).
    fn nan() -> Self;

    /// Returns true if the number is infinite.
    fn is_infinite(self) -> bool;

    /// Returns true if the number is finite.
    fn is_finite(self) -> bool;

    /// Returns true if the number is NaN.
    fn is_nan(self) -> bool;

    /// Returns true if the number is normal.
    fn is_normal(self) -> bool;

    /// Returns the floor of the number.
    fn floor(self) -> Self;

    /// Returns the ceiling of the number.
    fn ceil(self) -> Self;

    /// Returns the rounded value.
    fn round(self) -> Self;

    /// Returns the integer part.
    fn trunc(self) -> Self;

    /// Returns the fractional part.
    fn fract(self) -> Self;

    /// Returns the absolute value.
    fn abs(self) -> Self;

    /// Returns x raised to the power of y.
    fn powf(self, y: Self) -> Self;

    /// Returns the square root.
    fn sqrt(self) -> Self;

    /// Returns the cube root.
    fn cbrt(self) -> Self;

    /// Returns e raised to the power of x.
    fn exp(self) -> Self;

    /// Returns 2 raised to the power of x.
    fn exp2(self) -> Self;

    /// Returns the natural logarithm.
    fn ln(self) -> Self;

    /// Returns the base 2 logarithm.
    fn log2(self) -> Self;

    /// Returns the base 10 logarithm.
    fn log10(self) -> Self;

    /// Returns the maximum of two values.
    fn max(self, other: Self) -> Self;

    /// Returns the minimum of two values.
    fn min(self, other: Self) -> Self;
}

impl FloatExt for f32 {
    #[inline]
    fn max_value() -> Self { Self::MAX }

    #[inline]
    fn min_value() -> Self { Self::MIN }

    #[inline]
    fn infinity() -> Self { Self::INFINITY }

    #[inline]
    fn neg_infinity() -> Self { Self::NEG_INFINITY }

    #[inline]
    fn nan() -> Self { Self::NAN }

    #[inline]
    fn is_infinite(self) -> bool { self.is_infinite() }

    #[inline]
    fn is_finite(self) -> bool { self.is_finite() }

    #[inline]
    fn is_nan(self) -> bool { self.is_nan() }

    #[inline]
    fn is_normal(self) -> bool { self.is_normal() }

    #[inline]
    fn floor(self) -> Self { self.floor() }

    #[inline]
    fn ceil(self) -> Self { self.ceil() }

    #[inline]
    fn round(self) -> Self { self.round() }

    #[inline]
    fn trunc(self) -> Self { self.trunc() }

    #[inline]
    fn fract(self) -> Self { self.fract() }

    #[inline]
    fn abs(self) -> Self { self.abs() }

    #[inline]
    fn powf(self, y: Self) -> Self { self.powf(y) }

    #[inline]
    fn sqrt(self) -> Self { self.sqrt() }

    #[inline]
    fn cbrt(self) -> Self { self.cbrt() }

    #[inline]
    fn exp(self) -> Self { self.exp() }

    #[inline]
    fn exp2(self) -> Self { self.exp2() }

    #[inline]
    fn ln(self) -> Self { self.ln() }

    #[inline]
    fn log2(self) -> Self { self.log2() }

    #[inline]
    fn log10(self) -> Self { self.log10() }

    #[inline]
    fn max(self, other: Self) -> Self { self.max(other) }

    #[inline]
    fn min(self, other: Self) -> Self { self.min(other) }
}

impl FloatExt for f64 {
    #[inline]
    fn max_value() -> Self { Self::MAX }

    #[inline]
    fn min_value() -> Self { Self::MIN }

    #[inline]
    fn infinity() -> Self { Self::INFINITY }

    #[inline]
    fn neg_infinity() -> Self { Self::NEG_INFINITY }

    #[inline]
    fn nan() -> Self { Self::NAN }

    #[inline]
    fn is_infinite(self) -> bool { self.is_infinite() }

    #[inline]
    fn is_finite(self) -> bool { self.is_finite() }

    #[inline]
    fn is_nan(self) -> bool { self.is_nan() }

    #[inline]
    fn is_normal(self) -> bool { self.is_normal() }

    #[inline]
    fn floor(self) -> Self { self.floor() }

    #[inline]
    fn ceil(self) -> Self { self.ceil() }

    #[inline]
    fn round(self) -> Self { self.round() }

    #[inline]
    fn trunc(self) -> Self { self.trunc() }

    #[inline]
    fn fract(self) -> Self { self.fract() }

    #[inline]
    fn abs(self) -> Self { self.abs() }

    #[inline]
    fn powf(self, y: Self) -> Self { self.powf(y) }

    #[inline]
    fn sqrt(self) -> Self { self.sqrt() }

    #[inline]
    fn cbrt(self) -> Self { self.cbrt() }

    #[inline]
    fn exp(self) -> Self { self.exp() }

    #[inline]
    fn exp2(self) -> Self { self.exp2() }

    #[inline]
    fn ln(self) -> Self { self.ln() }

    #[inline]
    fn log2(self) -> Self { self.log2() }

    #[inline]
    fn log10(self) -> Self { self.log10() }

    #[inline]
    fn max(self, other: Self) -> Self { self.max(other) }

    #[inline]
    fn min(self, other: Self) -> Self { self.min(other) }
}

// =============================================================================
// Bool Extensions
// =============================================================================

/// Extension methods for boolean.
pub trait BoolExt {
    /// Converts bool to Option with Some for true, None for false.
    fn as_option<T>(self, value: T) -> Option<T>;

    /// Converts bool to Result with Ok for true, Err for false.
    fn as_result<T, E>(self, ok: T, err: E) -> Result<T, E>;
}

impl BoolExt for bool {
    #[inline]
    fn as_option<T>(self, value: T) -> Option<T> {
        if self { Some(value) } else { None }
    }

    #[inline]
    fn as_result<T, E>(self, ok: T, err: E) -> Result<T, E> {
        if self { Ok(ok) } else { Err(err) }
    }
}

// =============================================================================
// Char Extensions
// =============================================================================

/// Extension methods for characters.
pub trait CharExt: Sized {
    /// Returns the Unicode escape sequence for the character.
    fn escape_unicode(self) -> String;

    /// Returns true if the character is ASCII.
    fn is_ascii(self) -> bool;

    /// Returns true if the character is an alphabetic character.
    fn is_alphabetic(self) -> bool;

    /// Returns true if the character is alphanumeric.
    fn is_alphanumeric(self) -> bool;

    /// Returns true if the character is a digit.
    fn is_digit(self, base: u32) -> bool;

    /// Returns true if the character is whitespace.
    fn is_whitespace(self) -> bool;

    /// Converts to lowercase.
    fn to_lowercase(self) -> String;

    /// Converts to uppercase.
    fn to_uppercase(self) -> String;
}

impl CharExt for char {
    #[inline]
    fn escape_unicode(self) -> String {
        let mut s = String::new();
        s.push('\\');
        s.push('u');
        s.push('{');
        s.push_str(&format!("{:x}", self as u32));
        s.push('}');
        s
    }

    #[inline]
    fn is_ascii(self) -> bool {
        self as u32 <= 0x7F
    }

    #[inline]
    fn is_alphabetic(self) -> bool {
        self.is_ascii() && (self.is_ascii_alphabetic())
    }

    #[inline]
    fn is_alphanumeric(self) -> bool {
        self.is_ascii() && (self.is_ascii_alphanumeric())
    }

    #[inline]
    fn is_digit(self, base: u32) -> bool {
        if base <= 10 {
            self.is_ascii_digit() && (self as u8 - b'0') < base as u8
        } else {
            false // Simplified
        }
    }

    #[inline]
    fn is_whitespace(self) -> bool {
        self.is_ascii_whitespace()
    }

    #[inline]
    fn to_lowercase(self) -> String {
        self.to_lowercase().to_string()
    }

    #[inline]
    fn to_uppercase(self) -> String {
        self.to_uppercase().to_string()
    }
}

// =============================================================================
// String Extensions
// =============================================================================

/// Extension methods for string slices.
pub trait StrExt {
    /// Returns true if the string is empty.
    fn is_empty(&self) -> bool;

    /// Returns the length of the string in bytes.
    fn len(&self) -> usize;

    /// Returns a slice of the string as bytes.
    fn as_bytes(&self) -> &[u8];

    /// Returns true if the string starts with the given prefix.
    fn starts_with(&self, prefix: &str) -> bool;

    /// Returns true if the string ends with the given suffix.
    fn ends_with(&self, suffix: &str) -> bool;

    /// Returns true if the string contains the given substring.
    fn contains(&self, substring: &str) -> bool;

    /// Returns the index of the first occurrence of the substring.
    fn find(&self, substring: &str) -> crate::core::option::Option<usize>;

    /// Returns a new string with leading and trailing whitespace removed.
    fn trim(&self) -> &str;

    /// Returns a new string with leading whitespace removed.
    fn trim_start(&self) -> &str;

    /// Returns a new string with trailing whitespace removed.
    fn trim_end(&self) -> &str;

    /// Splits the string by whitespace.
    fn split_whitespace(&self) -> SplitWhitespace<'_>;

    /// Converts to uppercase.
    fn to_uppercase(&self) -> String;

    /// Converts to lowercase.
    fn to_lowercase(&self) -> String;
}

/// Iterator over whitespace-separated parts of a string.
#[derive(Debug, Clone)]
pub struct SplitWhitespace<'a> {
    s: &'a str,
}

impl<'a> Iterator for SplitWhitespace<'a> {
    type Item = &'a str;

    fn next(&mut self) -> std::option::Option<Self::Item> {
        self.s = self.s.trim_start();
        if self.s.is_empty() {
            return std::option::Option::None;
        }
        let idx = self.s.find(|c: char| c.is_whitespace()).unwrap_or(self.s.len());
        let result = &self.s[..idx];
        self.s = &self.s[idx..];
        std::option::Option::Some(result)
    }
}

impl StrExt for str {
    #[inline]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    fn len(&self) -> usize {
        self.len()
    }

    #[inline]
    fn as_bytes(&self) -> &[u8] {
        self.as_bytes()
    }

    #[inline]
    fn starts_with(&self, prefix: &str) -> bool {
        self.starts_with(prefix)
    }

    #[inline]
    fn ends_with(&self, suffix: &str) -> bool {
        self.ends_with(suffix)
    }

    #[inline]
    fn contains(&self, substring: &str) -> bool {
        self.contains(substring)
    }

    #[inline]
    fn find(&self, substring: &str) -> crate::core::option::Option<usize> {
        match self.find(substring) {
            std::option::Option::Some(idx) => crate::core::option::Option::Some(idx),
            std::option::Option::None => crate::core::option::Option::None,
        }
    }

    #[inline]
    fn trim(&self) -> &str {
        self.trim()
    }

    #[inline]
    fn trim_start(&self) -> &str {
        self.trim_start()
    }

    #[inline]
    fn trim_end(&self) -> &str {
        self.trim_end()
    }

    #[inline]
    fn split_whitespace(&self) -> SplitWhitespace<'_> {
        SplitWhitespace { s: self }
    }

    #[inline]
    fn to_uppercase(&self) -> String {
        self.to_uppercase()
    }

    #[inline]
    fn to_lowercase(&self) -> String {
        self.to_lowercase()
    }
}

// =============================================================================
// Slice Extensions
// =============================================================================

/// Extension methods for slices.
pub trait SliceExt<T> {
    /// Returns the first element of the slice.
    fn first(&self) -> Option<&T>;

    /// Returns the last element of the slice.
    fn last(&self) -> Option<&T>;

    /// Returns true if the slice is empty.
    fn is_empty(&self) -> bool;

    /// Returns the length of the slice.
    fn len(&self) -> usize;

    /// Returns a slice starting at the first element.
    fn split_first(&self) -> Option<(&T, &[T])>;

    /// Returns a slice ending at the last element.
    fn split_last(&self) -> Option<(&[T], &T)>;
}

impl<T> SliceExt<T> for [T] {
    #[inline]
    fn first(&self) -> Option<&T> {
        self.get(0)
    }

    #[inline]
    fn last(&self) -> Option<&T> {
        if self.is_empty() { None } else { Some(&self[self.len() - 1]) }
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    fn len(&self) -> usize {
        self.len()
    }

    #[inline]
    fn split_first(&self) -> Option<(&T, &[T])> {
        if self.is_empty() {
            None
        } else {
            Some((&self[0], &self[1..]))
        }
    }

    #[inline]
    fn split_last(&self) -> Option<(&[T], &T)> {
        if self.is_empty() {
            None
        } else {
            let len = self.len();
            Some((&self[..len - 1], &self[len - 1]))
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
    fn test_u32_extensions() {
        let x: u32 = 42;
        assert_eq!(x.popcount(), 3);
        assert!(!x.is_power_of_two());
        let next = UIntExt::next_power_of_two(x);
        assert!(next.is_some());
        assert_eq!(x.wrapping_add(u32::MAX), 41);
    }

    #[test]
    fn test_i32_extensions() {
        let x: i32 = -42;
        assert_eq!(x.abs(), 42);
        assert!(x.is_negative());
        assert!(!x.is_positive());
        assert_eq!(x.signum(), -1);
    }

    #[test]
    fn test_float_extensions() {
        let x: f64 = 64.0;
        assert_eq!(x.sqrt(), 8.0);
        assert!((x.cbrt() - 4.0).abs() < 1e-10);
        assert!((x.ln() - (2.0_f64.ln() * 6.0)).abs() < 1e-10);
    }

    #[test]
    fn test_bool_extensions() {
        assert_eq!(true.as_option(42), Some(42));
        assert_eq!(false.as_option(42), None);
        assert_eq!(true.as_result(42, "err"), Ok(42));
        assert_eq!(false.as_result(42, "err"), Err("err"));
    }

    #[test]
    fn test_str_extensions() {
        let s = "  hello world  ";
        assert_eq!(s.trim(), "hello world");
        assert!(s.starts_with("  "));
        assert!(s.ends_with("  "));
        
        let parts: Vec<_> = "a b c".split_whitespace().collect();
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_slice_extensions() {
        let arr = [1, 2, 3];
        assert_eq!(arr.first(), Some(&1));
        assert_eq!(arr.last(), Some(&3));
        
        let (first, rest) = arr.split_first().unwrap();
        assert_eq!(first, &1);
        assert_eq!(rest, &[2, 3]);
    }
}

