use std::collections::HashMap;

/// A linked-list-style ordered map of options, with `first`, `next`, `prev` navigation.
/// Mirrors upstream `OptionMap<T>` used in `useSelectState`.
#[derive(Debug, Clone)]
pub struct OptionMap<T> {
    items: Vec<(String, T)>,
    indices: HashMap<String, usize>,
}

impl<T> OptionMap<T> {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            indices: HashMap::new(),
        }
    }

    pub fn add(&mut self, key: String, value: T) {
        self.indices.insert(key.clone(), self.items.len());
        self.items.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn first(&self) -> Option<&str> {
        self.items.first().map(|(k, _)| k.as_str())
    }

    pub fn next(&self, current: &str) -> Option<&str> {
        self.indices.get(current).and_then(|&i| {
            self.items.get(i + 1).map(|(k, _)| k.as_str())
        })
    }

    pub fn prev(&self, current: &str) -> Option<&str> {
        self.indices.get(current).and_then(|&i| {
            if i == 0 {
                None
            } else {
                self.items.get(i - 1).map(|(k, _)| k.as_str())
            }
        })
    }

    pub fn get(&self, key: &str) -> Option<&T> {
        self.indices.get(key).and_then(|&i| self.items.get(i).map(|(_, v)| v))
    }

    pub fn contains(&self, key: &str) -> bool {
        self.indices.contains_key(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.items.iter().map(|(k, _)| k.as_str())
    }
}

impl<T> Default for OptionMap<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// State for a select prompt with pagination support.
#[derive(Debug, Clone)]
pub struct UseSelectState {
    pub selected: Option<String>,
    pub visible_from: usize,
    pub visible_to: usize,
    pub page_size: usize,
}

impl UseSelectState {
    pub fn new(page_size: usize) -> Self {
        Self {
            selected: None,
            visible_from: 0,
            visible_to: page_size,
            page_size,
        }
    }

    pub fn select(&mut self, key: String) {
        self.selected = Some(key);
    }

    pub fn select_next(&mut self, map: &OptionMap<()>) {
        let current = self.selected.as_deref().unwrap_or("");
        let next = if current.is_empty() {
            map.first()
        } else {
            map.next(current)
        };
        if let Some(key) = next {
            self.selected = Some(key.to_string());
        }
    }

    pub fn select_previous(&mut self, map: &OptionMap<()>) {
        let current = self.selected.as_deref().unwrap_or("");
        let prev = if current.is_empty() {
            map.first()
        } else {
            map.prev(current)
        };
        if let Some(key) = prev {
            self.selected = Some(key.to_string());
        }
    }

    pub fn page_down(&mut self, total: usize) {
        self.visible_from = (self.visible_from + self.page_size).min(total.saturating_sub(self.page_size));
        self.visible_to = (self.visible_from + self.page_size).min(total);
    }

    pub fn page_up(&mut self) {
        self.visible_from = self.visible_from.saturating_sub(self.page_size);
        self.visible_to = self.visible_from + self.page_size;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_option_map_empty() {
        let map: OptionMap<()> = OptionMap::new();
        assert!(map.is_empty());
    }

    #[test]
    fn test_option_map_add_and_get() {
        let mut map: OptionMap<()> = OptionMap::new();
        map.add("a".into(), ());
        map.add("b".into(), ());
        assert_eq!(map.len(), 2);
        assert!(map.contains("a"));
        assert!(map.contains("b"));
    }

    #[test]
    fn test_option_map_navigation() {
        let mut map: OptionMap<()> = OptionMap::new();
        map.add("a".into(), ());
        map.add("b".into(), ());
        map.add("c".into(), ());

        assert_eq!(map.first(), Some("a"));
        assert_eq!(map.next("a"), Some("b"));
        assert_eq!(map.next("b"), Some("c"));
        assert_eq!(map.next("c"), None);
        assert_eq!(map.prev("b"), Some("a"));
        assert_eq!(map.prev("a"), None);
    }

    #[test]
    fn test_use_select_state_new() {
        let state = UseSelectState::new(10);
        assert_eq!(state.page_size, 10);
        assert!(state.selected.is_none());
    }

    #[test]
    fn test_use_select_state_select() {
        let mut state = UseSelectState::new(5);
        state.select("opt".into());
        assert_eq!(state.selected, Some("opt".into()));
    }

    #[test]
    fn test_use_select_state_select_next() {
        let mut map: OptionMap<()> = OptionMap::new();
        map.add("a".into(), ());
        map.add("b".into(), ());

        let mut state = UseSelectState::new(5);
        state.select_next(&map);
        assert_eq!(state.selected, Some("a".into()));
        state.select_next(&map);
        assert_eq!(state.selected, Some("b".into()));
    }

    #[test]
    fn test_use_select_state_page_down() {
        let mut state = UseSelectState::new(5);
        state.page_down(20);
        assert_eq!(state.visible_from, 5);
    }

    #[test]
    fn test_use_select_state_page_up() {
        let mut state = UseSelectState::new(5);
        state.page_down(20);
        state.page_up();
        assert_eq!(state.visible_from, 0);
    }
}
