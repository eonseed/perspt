pub fn parse_record(input: &str) -> Result<Vec<String>, String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut chars = input.chars().peekable();
    let mut quoted = false;
    while let Some(ch) = chars.next() {
        match ch {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => fields.push(std::mem::take(&mut field)),
            other => field.push(other),
        }
    }
    if quoted {
        return Err("unterminated quote".into());
    }
    fields.push(field);
    Ok(fields)
}

#[cfg(test)]
mod tests {
    #[test]
    fn current_delimiter_is_comma() {
        assert_eq!(super::parse_record("a,b").unwrap(), vec!["a", "b"]);
    }
}
