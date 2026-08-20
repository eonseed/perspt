#[derive(Debug, PartialEq, Eq)]
pub struct Summary {
    pub count: usize,
    pub min: i64,
    pub max: i64,
    pub mean: i64,
}

pub fn summarize(values: &[i64]) -> Option<Summary> {
    let _ = values;
    todo!()
}
