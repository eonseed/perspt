use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

#[derive(Clone)]
pub struct Counter(Arc<AtomicI64>);

impl Counter {
    pub fn new(initial: i64) -> Self {
        Self(Arc::new(AtomicI64::new(initial)))
    }
    pub fn add(&self, delta: i64) -> i64 {
        self.0.fetch_add(delta, Ordering::SeqCst) + delta
    }
    pub fn get(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}
