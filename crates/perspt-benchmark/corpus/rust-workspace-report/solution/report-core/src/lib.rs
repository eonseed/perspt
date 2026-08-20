#[derive(Debug, PartialEq, Eq)]
pub struct Summary {
    pub count: usize,
    pub min: i64,
    pub max: i64,
    pub mean: i64,
}

pub fn summarize(values: &[i64]) -> Option<Summary> {
    let (&first, rest) = values.split_first()?;
    let mut min = first;
    let mut max = first;
    let mut sum = i128::from(first);
    for &value in rest {
        min = min.min(value);
        max = max.max(value);
        sum += i128::from(value);
    }
    Some(Summary {
        count: values.len(),
        min,
        max,
        mean: (sum / values.len() as i128) as i64,
    })
}
