use rust_config_layering::{parse_layer, Config};

#[test]
fn parses_and_layers() {
    let base = parse_layer("host=localhost\nport=80\nfeatures=z, a,z").unwrap();
    let later = parse_layer("# production\nport=443\n").unwrap();
    assert_eq!(
        base.apply(later),
        Config {
            host: Some("localhost".into()),
            port: Some(443),
            features: Some(vec!["a".into(), "z".into()])
        }
    );
}

#[test]
fn rejects_invalid_input() {
    assert!(parse_layer("port=70000").is_err());
    assert!(parse_layer("mystery=yes").is_err());
}
