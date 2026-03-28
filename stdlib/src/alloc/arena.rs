//! NYX Arena Allocator Module

/// Arena allocator for bulk memory allocation
pub struct Arena {
    memory: Vec<u8>,
    offset: usize,
}

impl Arena {
    /// Create new arena
    pub fn new(capacity: usize) -> Arena {
        Arena {
            memory: vec![0; capacity],
            offset: 0,
        }
    }

    /// Allocate from arena
    pub fn alloc(&mut self, size: usize, align: usize) -> Option<usize> {
        let aligned = (self.offset + align - 1) & !(align - 1);
        if aligned + size <= self.memory.len() {
            let result = aligned;
            self.offset = aligned + size;
            Some(result)
        } else {
            None
        }
    }

    /// Reset arena
    pub fn reset(&mut self) {
        self.offset = 0;
    }

    /// Get remaining capacity
    pub fn remaining(&self) -> usize {
        self.memory.len() - self.offset
    }
}
