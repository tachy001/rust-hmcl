//! Parser for Java `.properties` files (UTF-8 variant, as used by HMCL).
//!
//! Supports:
//! - comment lines starting with `#` or `!`
//! - `key = value`, `key : value` and `key value` separators
//! - line continuation with a trailing `\`
//! - escapes: `\t` `\n` `\r` `\\` `\=` `\:` `\ ` and `\uXXXX`

use std::collections::HashMap;

/// Parse the contents of a `.properties` file into a flat key-value map.
pub fn parse_properties(text: &str) -> HashMap<String, String> {
    let mut entries = HashMap::new();
    let mut lines = text.lines().peekable();

    while let Some(raw) = lines.next() {
        let mut line = raw.trim().to_owned();
        if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
            continue;
        }

        // Join continuation lines.
        while line.ends_with('\\') && !line.ends_with("\\\\") {
            line.pop();
            match lines.next() {
                Some(cont) => line.push_str(cont.trim()),
                None => break,
            }
        }

        let (key, value) = split_key_value(&line);
        let key = unescape(key.trim());
        if key.is_empty() {
            continue;
        }
        let value = unescape(value.trim());
        entries.insert(key, value);
    }

    entries
}

/// Split `key=value` at the first unescaped separator character.
fn split_key_value(line: &str) -> (&str, &str) {
    let bytes = line.as_bytes();
    let mut escaped = false;
    for (i, b) in bytes.iter().enumerate() {
        match b {
            b'\\' => escaped = !escaped,
            b'=' | b':' if !escaped => return (&line[..i], &line[i + 1..]),
            b' ' | b'\t' if !escaped => return (&line[..i], &line[i + 1..]),
            _ => escaped = false,
        }
    }
    (line, "")
}

/// Unescape `.properties` escape sequences.
fn unescape(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            result.push(c);
            continue;
        }
        match chars.next() {
            Some('t') => result.push('\t'),
            Some('n') => result.push('\n'),
            Some('r') => result.push('\r'),
            Some('u') => {
                let mut code = 0u32;
                for _ in 0..4 {
                    let Some(hex) = chars.next().and_then(|h| h.to_digit(16)) else {
                        break;
                    };
                    code = code * 16 + hex;
                }
                if let Some(c) = char::from_u32(code) {
                    result.push(c);
                }
            }
            Some(other) => result.push(other),
            None => result.push('\\'),
        }
    }
    result
}
