use std::collections::{HashMap, HashSet};
use rand::Rng;

pub fn take_random_from_array<T>(array: &[T]) -> Option<&T> {
    if array.is_empty() {
        return None;
    }
    let idx = rand::thread_rng().gen_range(0..array.len());
    Some(&array[idx])
}

pub fn get_array_rejecting_undefined<T>(array: Vec<Option<T>>) -> Vec<T> {
    array.into_iter().flatten().collect()
}

pub fn get_array_contains_duplicates<T: Eq + std::hash::Hash>(array: &[T]) -> bool {
    let mut seen = HashSet::new();
    for item in array {
        if !seen.insert(item) {
            return true;
        }
    }
    false
}

pub fn uniq<T: Eq + std::hash::Hash + Clone>(array: &[T]) -> Vec<T> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for item in array {
        if seen.insert(item) {
            result.push(item.clone());
        }
    }
    result
}

pub fn difference<T: Eq + std::hash::Hash + Clone>(array: &[T], values: &[T]) -> Vec<T> {
    let excluded: HashSet<_> = values.iter().collect();
    array.iter().filter(|item| !excluded.contains(item)).cloned().collect()
}

pub fn group_by<T, K, F>(items: &[T], key_fn: F) -> HashMap<K, Vec<&T>>
where
    K: Eq + std::hash::Hash,
    F: Fn(&T) -> K,
{
    let mut map = HashMap::new();
    for item in items {
        let key = key_fn(item);
        map.entry(key).or_insert_with(Vec::new).push(item);
    }
    map
}

pub fn partition<T, F>(items: Vec<T>, predicate: F) -> (Vec<T>, Vec<T>)
where
    F: Fn(&T) -> bool,
{
    let mut truthy = Vec::new();
    let mut falsey = Vec::new();
    for item in items {
        if predicate(&item) {
            truthy.push(item);
        } else {
            falsey.push(item);
        }
    }
    (truthy, falsey)
}

pub fn as_human_friendly_array(items: &[impl AsRef<str>]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        result.push(item.as_ref().to_string());
    }
    if result.len() > 1 {
        if let Some(last) = result.pop() {
            result.push(format!("and {}", last));
        }
    }
    result
}

pub fn join_with_and(items: &[impl AsRef<str>]) -> String {
    let quoted: Vec<String> = items.iter().map(|s| format!("\"{}\"", s.as_ref())).collect();
    as_human_friendly_array(&quoted).join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_take_random_from_empty() {
        let empty: &[i32] = &[];
        assert!(take_random_from_array(empty).is_none());
    }

    #[test]
    fn test_get_array_rejecting_undefined() {
        let input = vec![Some(1), None, Some(3), None];
        assert_eq!(get_array_rejecting_undefined(input), vec![1, 3]);
    }

    #[test]
    fn test_contains_duplicates_true() {
        assert!(get_array_contains_duplicates(&[1, 2, 1]));
    }

    #[test]
    fn test_contains_duplicates_false() {
        assert!(!get_array_contains_duplicates(&[1, 2, 3]));
    }

    #[test]
    fn test_uniq() {
        assert_eq!(uniq(&[1, 2, 1, 3, 2]), vec![1, 2, 3]);
    }

    #[test]
    fn test_difference() {
        assert_eq!(difference(&[1, 2, 3, 4], &[2, 4]), vec![1, 3]);
    }

    #[test]
    fn test_group_by() {
        let items = vec!["a", "ab", "abc"];
        let groups = group_by(&items, |s| s.len());
        assert_eq!(groups[&1].len(), 1);
        assert_eq!(groups[&2].len(), 1);
        assert_eq!(groups[&3].len(), 1);
    }

    #[test]
    fn test_partition() {
        let items = vec![1, 2, 3, 4, 5];
        let (evens, odds) = partition(items, |n| n % 2 == 0);
        assert_eq!(evens, vec![2, 4]);
        assert_eq!(odds, vec![1, 3, 5]);
    }

    #[test]
    fn test_as_human_friendly_array() {
        let result = as_human_friendly_array(&["a", "b", "c"]);
        assert_eq!(result, vec!["a".to_string(), "b".to_string(), "and c".to_string()]);
    }

    #[test]
    fn test_join_with_and() {
        assert_eq!(join_with_and(&["a", "b"]), "\"a\", and \"b\"");
    }
}
