pub fn normalize_ranges(ranges: &[(u64, u64)]) -> Vec<(u64, u64)> {
    let mut ranges: Vec<_> = ranges
        .iter()
        .copied()
        .filter(|(start, end)| start < end)
        .collect();
    ranges.sort_unstable();
    let mut result: Vec<(u64, u64)> = Vec::new();
    for (start, end) in ranges {
        if let Some(last) = result.last_mut() {
            if start <= last.1 {
                last.1 = last.1.max(end);
                continue;
            }
        }
        result.push((start, end));
    }
    result
}
