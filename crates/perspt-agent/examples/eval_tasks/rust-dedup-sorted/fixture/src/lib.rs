pub fn dedup_sorted(values: &mut Vec<i64>) { let _ = values; todo!() }

#[cfg(test)]
mod tests {
    #[test]
    fn removes_consecutive() {
        let mut v = vec![1, 1, 2, 3, 3, 3];
        super::dedup_sorted(&mut v);
        assert_eq!(v, vec![1, 2, 3]);
    }
}
