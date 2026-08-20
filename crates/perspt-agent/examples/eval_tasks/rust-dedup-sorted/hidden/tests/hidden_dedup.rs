#[test]
fn empty_ok() {
    let mut v: Vec<i64> = vec![];
    t::dedup_sorted(&mut v);
    assert!(v.is_empty());
}

#[test]
fn single_and_negative() {
    let mut v = vec![-5, -5, -5, 0, 7];
    t::dedup_sorted(&mut v);
    assert_eq!(v, vec![-5, 0, 7]);
}
