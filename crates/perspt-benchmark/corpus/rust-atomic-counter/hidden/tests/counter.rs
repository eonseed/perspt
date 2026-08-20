#[test]
fn clones_share_linearizable_state() {
    let counter = rust_atomic_counter::Counter::new(7);
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let c = counter.clone();
            std::thread::spawn(move || {
                for _ in 0..1000 {
                    c.add(1);
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(counter.get(), 8007);
    assert_eq!(counter.add(-7), 8000);
}
