use std::collections::{HashMap, HashSet};

pub fn join_strings(parts: &[&str], sep: &str) -> String {
    parts.join(sep)
}

pub fn array_map<T, U, F>(items: &[T], mut f: F) -> Vec<U>
where
    F: FnMut(&T) -> U,
{
    items.iter().map(&mut f).collect()
}

pub fn hashmap_from_pairs<K, V>(pairs: Vec<(K, V)>) -> HashMap<K, V>
where
    K: std::cmp::Eq + std::hash::Hash,
{
    pairs.into_iter().collect()
}

pub fn set_from_items<T>(items: Vec<T>) -> HashSet<T>
where
    T: std::cmp::Eq + std::hash::Hash,
{
    items.into_iter().collect()
}

pub fn iter_sum(items: &[i64]) -> i64 {
    items.iter().copied().sum()
}
