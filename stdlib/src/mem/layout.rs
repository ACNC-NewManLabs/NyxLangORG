//! NYX Memory Layout Module

/// Memory layout utilities
pub mod layout {
    use core::mem::MaybeUninit;

    /// Memory layout representation
    pub struct Layout {
        size: usize,
        align: usize,
    }

    impl Layout {
        /// Create a new layout from size and alignment
        pub fn from_size_align(size: usize, align: usize) -> Option<Layout> {
            if align == 0 || !align.is_power_of_two() {
                return None;
            }
            Some(Layout { size, align })
        }

        /// Create a layout for type T
        pub fn new<T>() -> Layout {
            Layout {
                size: core::mem::size_of::<T>(),
                align: core::mem::align_of::<T>(),
            }
        }

        /// Get the size
        pub fn size(&self) -> usize {
            self.size
        }

        /// Get the alignment
        pub fn align(&self) -> usize {
            self.align
        }

        /// Extend layout with additional size
        pub fn extend(&self, other: Layout) -> Option<(Layout, usize)> {
            let new_size = self.size_for(other.size);
            Some((Layout::from_size_align(new_size, self.align)?, self.size))
        }

        /// Calculate size needed for additional bytes
        pub fn size_for(&self, additional: usize) -> usize {
            let offset = self.size;
            let aligned = (offset + self.align - 1) & !(self.align - 1);
            aligned + additional
        }

        /// Create a layout for an array of T
        pub fn array<T>(n: usize) -> Option<Layout> {
            let layout = Layout::new::<T>();
            Layout::from_size_align(layout.size * n, layout.align)
        }

        /// Pad the layout to meet alignment requirements
        pub fn pad_to_align(&self) -> Layout {
            let size = (self.size + self.align - 1) & !(self.align - 1);
            Layout {
                size,
                align: self.align,
            }
        }
    }

    /// Create uninitialized memory
    #[inline]
    pub unsafe fn uninit<T>() -> MaybeUninit<T> {
        MaybeUninit::uninit()
    }

    /// Create zeroed memory
    #[inline]
    pub fn zeroed<T>() -> MaybeUninit<T>
    where
        T: Copy,
    {
        MaybeUninit::zeroed()
    }
}

pub use layout::*;
