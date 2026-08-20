use crate::Frame;

fn escape(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '|' => out.push_str("\\p"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out
}

fn split(line: &str) -> Result<Vec<String>, String> {
    if line.contains(['\n', '\r']) {
        return Err("raw newline".into());
    }
    let mut fields = vec![String::new()];
    let mut chars = line.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '|' => fields.push(String::new()),
            '\\' => fields.last_mut().unwrap().push(match chars.next() {
                Some('\\') => '\\',
                Some('p') => '|',
                Some('n') => '\n',
                Some('r') => '\r',
                _ => return Err("bad escape".into()),
            }),
            other => fields.last_mut().unwrap().push(other),
        }
    }
    Ok(fields)
}

pub fn encode(frame: &Frame) -> String {
    format!(
        "{}|{}|{}",
        escape(&frame.kind),
        frame.sequence,
        escape(&frame.payload)
    )
}
pub fn decode(line: &str) -> Result<Frame, String> {
    let fields = split(line)?;
    if fields.len() != 3 {
        return Err("field count".into());
    }
    Ok(Frame {
        kind: fields[0].clone(),
        sequence: fields[1].parse().map_err(|_| "sequence")?,
        payload: fields[2].clone(),
    })
}
