use rust_protocol_codec::{decode, encode, Frame};

#[test]
fn round_trips_reserved_characters() {
    let frame = Frame {
        kind: "a|b".into(),
        sequence: 42,
        payload: "x\\y\nz\r".into(),
    };
    assert_eq!(decode(&encode(&frame)).unwrap(), frame);
}

#[test]
fn rejects_malformed_lines() {
    for line in ["a|1", "a|x|b", "a|1|b|c", "a|1|bad\\q", "a|1|b\n"] {
        assert!(decode(line).is_err(), "{line:?}");
    }
}
