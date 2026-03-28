//! NYX Stack Allocator Module

/// Stack allocator
pub struct Stack {
    memory: Vec<u8>,
    offset: usize,
}

impl Stack {
    /// Create new stack allocator
    pub fn new(capacity: usize) -> Stack {
        Stack {
            memory: vec![0; capacity],
            offset: 0,
        }
    }

    /// Allocate from stack
    pub fn alloc(&mut self, size: usize) -> Option<usize> {
        if self.offset + size <= self.memory.len() {
            let result = self.offset;
            self.offset += size;
            Some(result)
        } else {
            None
        }
    }

    /// Deallocate from stack (pop)
    pub fn dealloc(&mut self, size: usize) {
        self.offset = self.offset.saturating_sub(size);
    }
}
