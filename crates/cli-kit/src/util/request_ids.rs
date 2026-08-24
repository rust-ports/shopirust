use std::sync::Mutex;

pub const MAX_REQUEST_IDS: usize = 100;

struct RequestIdCollection {
    request_ids: Vec<String>,
}

impl RequestIdCollection {
    const fn new() -> Self {
        RequestIdCollection {
            request_ids: Vec::new(),
        }
    }

    fn add_request_id(&mut self, request_id: Option<&str>) {
        if let Some(id) = request_id {
            if self.request_ids.len() < MAX_REQUEST_IDS {
                self.request_ids.push(id.to_string());
            }
        }
    }

    fn get_request_ids(&self) -> Vec<String> {
        self.request_ids.clone()
    }

    fn clear(&mut self) {
        self.request_ids.clear();
    }
}

static INSTANCE: once_cell::sync::Lazy<Mutex<RequestIdCollection>> =
    once_cell::sync::Lazy::new(|| Mutex::new(RequestIdCollection::new()));

fn lock_instance() -> std::sync::MutexGuard<'static, RequestIdCollection> {
    INSTANCE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn add_request_id(request_id: Option<&str>) {
    lock_instance().add_request_id(request_id);
}

pub fn get_request_ids() -> Vec<String> {
    lock_instance().get_request_ids()
}

pub fn clear_request_ids() {
    lock_instance().clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn lock_tests() -> std::sync::MutexGuard<'static, ()> {
        TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn test_add_and_get() {
        let _guard = lock_tests();
        clear_request_ids();
        add_request_id(Some("req-1"));
        add_request_id(Some("req-2"));
        let ids = get_request_ids();
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0], "req-1");
        assert_eq!(ids[1], "req-2");
    }

    #[test]
    fn test_ignores_none() {
        let _guard = lock_tests();
        clear_request_ids();
        add_request_id(None);
        assert!(get_request_ids().is_empty());
    }

    #[test]
    fn test_clear() {
        let _guard = lock_tests();
        clear_request_ids();
        add_request_id(Some("req-1"));
        clear_request_ids();
        assert!(get_request_ids().is_empty());
    }

    #[test]
    fn test_max_capacity() {
        let _guard = lock_tests();
        clear_request_ids();
        for i in 0..MAX_REQUEST_IDS + 10 {
            add_request_id(Some(&format!("req-{i}")));
        }
        assert_eq!(get_request_ids().len(), MAX_REQUEST_IDS);
    }
}
