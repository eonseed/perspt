fn normalize(value: &str) -> String {
    let mut out = String::new();
    let mut separator = false;
    for ch in value
        .trim_matches(|c: char| c.is_ascii_whitespace())
        .chars()
    {
        if ch.is_ascii_alphanumeric() {
            if separator && !out.is_empty() {
                out.push('-');
            }
            separator = false;
            out.push(ch.to_ascii_lowercase());
        } else {
            separator = true;
        }
    }
    out
}
fn main() {
    println!(
        "{}",
        normalize(&std::env::args().nth(1).unwrap_or_default())
    );
}
