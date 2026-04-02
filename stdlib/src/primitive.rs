//! NYX Primitive Extensions Layer

pub mod primitive {
    pub mod int {
        pub trait IntExt {
            fn abs(self) -> Self;
            fn pow(self, exp: u32) -> Self;
        }
        impl IntExt for i32 {
            fn abs(self) -> i32 {
                i32::abs(self)
            }
            fn pow(self, exp: u32) -> i32 {
                self.pow(exp)
            }
        }
        impl IntExt for i64 {
            fn abs(self) -> i64 {
                i64::abs(self)
            }
            fn pow(self, exp: u32) -> i64 {
                self.pow(exp)
            }
        }
    }
    pub mod float {
        pub trait FloatExt {
            fn floor(self) -> Self;
            fn ceil(self) -> Self;
            fn round(self) -> Self;
        }
        impl FloatExt for f32 {
            fn floor(self) -> f32 {
                f32::floor(self)
            }
            fn ceil(self) -> f32 {
                f32::ceil(self)
            }
            fn round(self) -> f32 {
                f32::round(self)
            }
        }
        impl FloatExt for f64 {
            fn floor(self) -> f64 {
                f64::floor(self)
            }
            fn ceil(self) -> f64 {
                f64::ceil(self)
            }
            fn round(self) -> f64 {
                f64::round(self)
            }
        }
    }
    pub mod str {
        pub fn len(s: &str) -> usize {
            s.len()
        }
        pub fn is_empty(s: &str) -> bool {
            s.is_empty()
        }
        pub fn contains(haystack: &str, needle: &str) -> bool {
            haystack.contains(needle)
        }
    }
}

pub use primitive::*;
