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
    fn touch(&mut self, key: &str) {
        self.order.retain(|candidate| candidate != key);
        self.order.push_back(key.to_owned());
    }
    pub fn put(&mut self, key: String, value: i32) {
        if self.capacity == 0 {
            return;
        }
        self.values.insert(key.clone(), value);
        self.touch(&key);
        while self.values.len() > self.capacity {
            if let Some(old) = self.order.pop_front() {
                self.values.remove(&old);
            }
        }
    }
    pub fn get(&mut self, key: &str) -> Option<i32> {
        let value = self.values.get(key).copied()?;
        self.touch(key);
        Some(value)
    }
}
