use std::collections::{HashMap, VecDeque};

pub struct Cache {
    capacity: usize,
    values: HashMap<String, i32>,
    order: VecDeque<String>,
}
impl Cache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            values: HashMap::new(),
            order: VecDeque::new(),
        }
    }
    pub fn put(&mut self, key: String, value: i32) {
        let _ = (key, value);
    }
    pub fn get(&mut self, key: &str) -> Option<i32> {
        self.values.get(key).copied()
    }
}
