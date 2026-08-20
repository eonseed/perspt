use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

pub fn parallel_map<T: Sync, R: Send, F: Fn(&T) -> R + Sync>(
    values: &[T],
    workers: usize,
    function: F,
) -> Result<Vec<R>, String> {
    if workers == 0 {
        return Err("workers must be positive".into());
    }
    let next = AtomicUsize::new(0);
    let output: Vec<Mutex<Option<R>>> = (0..values.len()).map(|_| Mutex::new(None)).collect();
    std::thread::scope(|scope| {
        for _ in 0..workers.min(values.len()) {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                if index >= values.len() {
                    break;
                }
                *output[index].lock().unwrap() = Some(function(&values[index]));
            });
        }
    });
    Ok(output
        .into_iter()
        .map(|slot| slot.into_inner().unwrap().unwrap())
        .collect())
}
