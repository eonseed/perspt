pub fn render(values: &[i64]) -> String {
    match report_core::summarize(values) {
        None => "empty".into(),
        Some(s) => format!(
            "count={},min={},max={},mean={}",
            s.count, s.min, s.max, s.mean
        ),
    }
}
