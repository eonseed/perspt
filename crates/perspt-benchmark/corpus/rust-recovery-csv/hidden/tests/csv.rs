#[test]
fn handles_csv_quoting() {
    assert_eq!(
        rust_recovery_csv::parse_record("a,\"b,c\",\"say \"\"hi\"\"\",").unwrap(),
        vec!["a", "b,c", "say \"hi\"", ""]
    );
    assert!(rust_recovery_csv::parse_record("a,\"broken").is_err());
}
