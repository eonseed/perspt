pub fn respond(input: &str) -> String {
    format!("ok:{}", wcore::normalize(input))
}
