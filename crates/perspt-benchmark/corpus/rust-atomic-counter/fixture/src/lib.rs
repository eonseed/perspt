#[derive(Clone)]
pub struct Counter;

impl Counter {
    pub fn new(initial: i64) -> Self {
        let _ = initial;
        Self
    }
    pub fn add(&self, delta: i64) -> i64 {
        let _ = delta;
        todo!()
    }
    pub fn get(&self) -> i64 {
        todo!()
    }
}
