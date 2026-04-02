//! NYX Memory Pin Module

/// Pinning utilities for preventing moving of types
pub mod pin {
    /// A pinned pointer
    #[derive(Debug)]
    pub struct Pin<P> {
        ptr: P,
    }

    impl<P> Pin<P> {
        /// Create a new pinned pointer
        pub fn new(ptr: P) -> Pin<P> {
            Pin { ptr }
        }

        /// Get a reference to the pinned pointer
        pub fn get(&self) -> &P {
            &self.ptr
        }

        /// Get a mutable reference to the pinned pointer
        pub fn get_mut(&mut self) -> &mut P {
            &mut self.ptr
        }
    }
}

pub use pin::*;
