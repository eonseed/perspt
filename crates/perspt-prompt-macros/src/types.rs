//! The closed set of supported section variable types (PSP-10 system 23).

use std::fmt;

/// A declared variable type. Unbounded `String` and `Vec<String>` are not
/// representable: every type carries its bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarType {
    Text(usize),
    Observation(usize),
    List { count: usize, bytes: usize },
    OptText(usize),
    OptObservation(usize),
    Int,
    UInt,
    Float,
}

impl VarType {
    /// Parse a declared type string, e.g. `"BoundedList<64,128>"` or
    /// `"Option<BoundedText<512>>"`.
    pub fn parse(declared: &str) -> Option<Self> {
        let declared = declared.trim();
        match declared {
            "i64" => return Some(VarType::Int),
            "u64" => return Some(VarType::UInt),
            "f64" => return Some(VarType::Float),
            _ => {}
        }
        if let Some(inner) = strip_generic(declared, "Option") {
            return match VarType::parse(&inner)? {
                VarType::Text(n) => Some(VarType::OptText(n)),
                VarType::Observation(n) => Some(VarType::OptObservation(n)),
                _ => None,
            };
        }
        if let Some(inner) = strip_generic(declared, "BoundedText") {
            return inner.parse().ok().map(VarType::Text);
        }
        if let Some(inner) = strip_generic(declared, "ObservationText") {
            return inner.parse().ok().map(VarType::Observation);
        }
        if let Some(inner) = strip_generic(declared, "BoundedList") {
            let (count, bytes) = inner.split_once(',')?;
            return Some(VarType::List {
                count: count.trim().parse().ok()?,
                bytes: bytes.trim().parse().ok()?,
            });
        }
        None
    }

    /// Whether absence/emptiness omits the placeholder's line, which is why
    /// such a placeholder must stand alone on its line.
    pub fn omits_line(&self) -> bool {
        matches!(
            self,
            VarType::List { .. } | VarType::OptText(_) | VarType::OptObservation(_)
        )
    }

    /// Worst-case rendered bytes, for the `max_bytes` build check.
    pub fn worst_case_bytes(&self) -> usize {
        match self {
            VarType::Text(n) | VarType::OptText(n) => *n,
            // Escaping can double the content; the block header and closer
            // add fixed overhead.
            VarType::Observation(n) | VarType::OptObservation(n) => n * 2 + 64,
            // Numbered prefix plus newline per item.
            VarType::List { count, bytes } => count * (bytes + 8),
            VarType::Int | VarType::UInt | VarType::Float => 24,
        }
    }

    /// The generated struct field's Rust type.
    pub fn rust_type(&self) -> String {
        match self {
            VarType::Text(n) => format!("perspt_sdk::prompt::BoundedText<{n}>"),
            VarType::Observation(n) => format!("perspt_sdk::prompt::ObservationText<{n}>"),
            VarType::List { count, bytes } => {
                format!("perspt_sdk::prompt::BoundedList<{count}, {bytes}>")
            }
            VarType::OptText(n) => format!("Option<perspt_sdk::prompt::BoundedText<{n}>>"),
            VarType::OptObservation(n) => {
                format!("Option<perspt_sdk::prompt::ObservationText<{n}>>")
            }
            VarType::Int => "i64".into(),
            VarType::UInt => "u64".into(),
            VarType::Float => "f64".into(),
        }
    }

    /// The generated expression converting the field into a `VarValue`.
    pub fn value_expr(&self, field: &str) -> String {
        match self {
            VarType::Int => format!("perspt_sdk::prompt::VarValue::Int(self.{field})"),
            VarType::UInt => format!("perspt_sdk::prompt::VarValue::UInt(self.{field})"),
            VarType::Float => format!("perspt_sdk::prompt::VarValue::Float(self.{field})"),
            _ => format!("perspt_sdk::prompt::VarValue::from(self.{field}.clone())"),
        }
    }
}

impl fmt::Display for VarType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.rust_type())
    }
}

fn strip_generic(declared: &str, name: &str) -> Option<String> {
    let rest = declared.strip_prefix(name)?.trim();
    let inner = rest.strip_prefix('<')?.strip_suffix('>')?;
    Some(inner.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_supported_list_parses_and_nothing_else_does() {
        assert_eq!(VarType::parse("BoundedText<512>"), Some(VarType::Text(512)));
        assert_eq!(
            VarType::parse("Option<BoundedText<512>>"),
            Some(VarType::OptText(512))
        );
        assert_eq!(
            VarType::parse("BoundedList<64,128>"),
            Some(VarType::List {
                count: 64,
                bytes: 128
            })
        );
        assert_eq!(VarType::parse("u64"), Some(VarType::UInt));
        assert_eq!(VarType::parse("String"), None);
        assert_eq!(VarType::parse("Vec<String>"), None);
        assert_eq!(VarType::parse("Option<BoundedList<2,3>>"), None);
    }
}
