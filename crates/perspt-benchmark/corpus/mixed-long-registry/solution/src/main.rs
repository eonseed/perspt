mod catalog;
fn main() {
    let key = std::env::args().nth(1).unwrap_or_default();
    match catalog::ENTRIES
        .iter()
        .find_map(|(candidate, value)| (*candidate == key).then_some(*value))
    {
        Some(value) => println!("{value}"),
        None => println!("none"),
    }
}
