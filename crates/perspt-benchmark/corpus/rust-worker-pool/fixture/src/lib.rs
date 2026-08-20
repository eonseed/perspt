pub fn parallel_map<T: Sync, R: Send, F: Fn(&T) -> R + Sync>(
    values: &[T],
    workers: usize,
    function: F,
) -> Result<Vec<R>, String> {
    let _ = (values, workers, function);
    todo!()
}
