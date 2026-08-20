use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn preserves_order_and_runs_once() {
    let calls = AtomicUsize::new(0);
    let values = vec![5, 1, 4, 2, 3];
    let got = rust_worker_pool::parallel_map(&values, 3, |value| {
        calls.fetch_add(1, Ordering::SeqCst);
        value * value
    })
    .unwrap();
    assert_eq!(got, vec![25, 1, 16, 4, 9]);
    assert_eq!(calls.load(Ordering::SeqCst), values.len());
}

#[test]
fn boundaries() {
    assert_eq!(
        rust_worker_pool::parallel_map::<i32, i32, _>(&[], 4, |v| *v).unwrap(),
        vec![]
    );
    assert!(rust_worker_pool::parallel_map(&[1], 0, |v| *v).is_err());
}
