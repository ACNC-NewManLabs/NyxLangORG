use std::collections::{HashMap, HashSet, VecDeque};

pub type Map<K, V> = HashMap<K, V>;
pub type Set<T> = HashSet<T>;

pub fn queue_new<T>() -> VecDeque<T> {
    VecDeque::new()
}

pub fn queue_push<T>(queue: &mut VecDeque<T>, item: T) {
    queue.push_back(item);
}

pub fn queue_pop<T>(queue: &mut VecDeque<T>) -> Option<T> {
    queue.pop_front()
}
