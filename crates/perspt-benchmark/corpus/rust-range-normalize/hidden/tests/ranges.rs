#[test]
fn canonicalizes_boundaries() {
    assert_eq!(
        rust_range_normalize::normalize_ranges(&[(8, 10), (1, 3), (3, 8), (5, 5), (12, 13)]),
        vec![(1, 10), (12, 13)]
    );
    assert_eq!(
        rust_range_normalize::normalize_ranges(&[
            (u64::MAX - 1, u64::MAX),
            (u64::MAX - 2, u64::MAX - 1)
        ]),
        vec![(u64::MAX - 2, u64::MAX)]
    );
}
