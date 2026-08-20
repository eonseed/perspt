use report_core::{summarize, Summary};

#[test]
fn boundaries_and_overflow() {
    assert_eq!(summarize(&[]), None);
    assert_eq!(
        summarize(&[i64::MAX, i64::MAX, i64::MIN]),
        Some(Summary {
            count: 3,
            min: i64::MIN,
            max: i64::MAX,
            mean: 3_074_457_345_618_258_602
        })
    );
    assert_eq!(
        summarize(&[-2, -1]),
        Some(Summary {
            count: 2,
            min: -2,
            max: -1,
            mean: -1
        })
    );
}
