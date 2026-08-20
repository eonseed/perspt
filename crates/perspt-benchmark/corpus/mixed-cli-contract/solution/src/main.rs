fn main() {
    let mut values = Vec::new();
    for raw in std::env::args().skip(1) {
        match raw.parse::<i64>() {
            Ok(value) => values.push(value),
            Err(_) => {
                eprintln!("invalid integer: {raw}");
                std::process::exit(2);
            }
        }
    }
    let sum: i128 = values.iter().map(|v| i128::from(*v)).sum();
    match (values.iter().min(), values.iter().max()) {
        (Some(min), Some(max)) => println!(
            "{{\"count\":{},\"min\":{},\"max\":{},\"sum\":{}}}",
            values.len(),
            min,
            max,
            sum
        ),
        _ => println!("{{\"count\":0,\"min\":null,\"max\":null,\"sum\":0}}"),
    }
}
