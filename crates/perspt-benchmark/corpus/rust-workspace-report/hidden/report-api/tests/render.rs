#[test]
fn renders_contract() {
    assert_eq!(report_api::render(&[]), "empty");
    assert_eq!(report_api::render(&[3, 1, 8]), "count=3,min=1,max=8,mean=4");
}
