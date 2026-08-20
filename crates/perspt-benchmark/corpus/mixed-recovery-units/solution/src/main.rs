fn parse(value: &str) -> Option<u64> {
    let value = value.trim().to_ascii_lowercase();
    let (digits, multiplier) = match value.as_bytes().last().copied() {
        Some(b'k') => (&value[..value.len() - 1], 1024),
        Some(b'm') => (&value[..value.len() - 1], 1024 * 1024),
        Some(b'g') => (&value[..value.len() - 1], 1024 * 1024 * 1024),
        _ => (value.as_str(), 1),
    };
    digits.parse::<u64>().ok()?.checked_mul(multiplier)
}
fn main() {
    match parse(&std::env::args().nth(1).unwrap_or_default()) {
        Some(value) => println!("{value}"),
        None => println!("none"),
    }
}
