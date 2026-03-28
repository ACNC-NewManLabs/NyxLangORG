//! NYX Collections Layer
//! 
//! High-performance containers: Vec, HashMap, HashSet, BTreeMap, etc.

pub mod vec;
pub mod string;
pub mod hash_map;
pub mod hash_set;
pub mod btree_map;
pub mod btree_set;
pub mod linked_list;
pub mod deque;
pub mod binary_heap;

/// Initialize collections
pub fn init() {
    // Collections initialization
}

// Re-exports
pub use vec::Vec;
pub use string::String;
pub use hash_map::HashMap;
pub use hash_set::HashSet;
pub use btree_map::BTreeMap;
pub use btree_set::BTreeSet;
pub use linked_list::LinkedList;
pub use deque::Deque;
pub use binary_heap::BinaryHeap;
