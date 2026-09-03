//! A small hand-rolled JSON reader/writer. We only ever need to read and
//! write flat objects with string/number/array-of-string fields, so this
//! isn't a general-purpose library, but the value parser underneath it
//! handles the full grammar rather than special-casing our schema.

use crate::card::Card;
use std::io;
use std::iter::Peekable;
use std::str::Chars;

#[derive(Debug)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }
}

pub fn parse(input: &str) -> Result<Value, String> {
    let mut chars = input.chars().peekable();
    let value = parse_value(&mut chars)?;
    skip_whitespace(&mut chars);
    if chars.next().is_some() {
        return Err("unexpected trailing characters after json value".to_string());
    }
    Ok(value)
}

fn skip_whitespace(chars: &mut Peekable<Chars>) {
    while matches!(chars.peek(), Some(c) if c.is_whitespace()) {
        chars.next();
    }
}

fn parse_value(chars: &mut Peekable<Chars>) -> Result<Value, String> {
    skip_whitespace(chars);
    match chars.peek() {
        Some('"') => parse_string(chars).map(Value::String),
        Some('{') => parse_object(chars),
        Some('[') => parse_array(chars),
        Some('t') | Some('f') => parse_bool(chars),
        Some('n') => parse_null(chars),
        Some(c) if c.is_ascii_digit() || *c == '-' => parse_number(chars),
        Some(c) => Err(format!("unexpected character '{c}'")),
        None => Err("unexpected end of input".to_string()),
    }
}

fn parse_string(chars: &mut Peekable<Chars>) -> Result<String, String> {
    if chars.next() != Some('"') {
        return Err("expected '\"' at start of string".to_string());
    }
    let mut out = String::new();
    loop {
        match chars.next() {
            Some('"') => return Ok(out),
            Some('\\') => match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('/') => out.push('/'),
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('b') => out.push('\u{8}'),
                Some('f') => out.push('\u{c}'),
                Some('u') => {
                    let code = read_hex4(chars)?;
                    out.push(char::from_u32(code).ok_or("invalid unicode escape")?);
                }
                Some(other) => return Err(format!("invalid escape sequence '\\{other}'")),
                None => return Err("unterminated escape sequence".to_string()),
            },
            Some(c) => out.push(c),
            None => return Err("unterminated string".to_string()),
        }
    }
}

fn read_hex4(chars: &mut Peekable<Chars>) -> Result<u32, String> {
    let mut value = 0u32;
    for _ in 0..4 {
        let c = chars.next().ok_or("truncated unicode escape")?;
        let digit = c
            .to_digit(16)
            .ok_or("invalid hex digit in unicode escape")?;
        value = value * 16 + digit;
    }
    Ok(value)
}

fn parse_number(chars: &mut Peekable<Chars>) -> Result<Value, String> {
    let mut raw = String::new();
    while matches!(chars.peek(), Some(c) if c.is_ascii_digit() || matches!(*c, '-' | '+' | '.' | 'e' | 'E'))
    {
        raw.push(chars.next().unwrap());
    }
    raw.parse::<f64>()
        .map(Value::Number)
        .map_err(|_| format!("invalid number literal '{raw}'"))
}

fn parse_bool(chars: &mut Peekable<Chars>) -> Result<Value, String> {
    if take_literal(chars, "true") {
        Ok(Value::Bool(true))
    } else if take_literal(chars, "false") {
        Ok(Value::Bool(false))
    } else {
        Err("invalid literal".to_string())
    }
}

fn parse_null(chars: &mut Peekable<Chars>) -> Result<Value, String> {
    if take_literal(chars, "null") {
        Ok(Value::Null)
    } else {
        Err("invalid literal".to_string())
    }
}

fn take_literal(chars: &mut Peekable<Chars>, literal: &str) -> bool {
    let mut probe = chars.clone();
    for expected in literal.chars() {
        match probe.next() {
            Some(c) if c == expected => continue,
            _ => return false,
        }
    }
    *chars = probe;
    true
}

fn parse_array(chars: &mut Peekable<Chars>) -> Result<Value, String> {
    chars.next(); // '['
    let mut items = Vec::new();
    skip_whitespace(chars);
    if chars.peek() == Some(&']') {
        chars.next();
        return Ok(Value::Array(items));
    }
    loop {
        items.push(parse_value(chars)?);
        skip_whitespace(chars);
        match chars.next() {
            Some(',') => continue,
            Some(']') => break,
            _ => return Err("expected ',' or ']' in array".to_string()),
        }
    }
    Ok(Value::Array(items))
}

fn parse_object(chars: &mut Peekable<Chars>) -> Result<Value, String> {
    chars.next(); // '{'
    let mut pairs = Vec::new();
    skip_whitespace(chars);
    if chars.peek() == Some(&'}') {
        chars.next();
        return Ok(Value::Object(pairs));
    }
    loop {
        skip_whitespace(chars);
        if chars.peek() != Some(&'"') {
            return Err("expected string key in object".to_string());
        }
        let key = parse_string(chars)?;
        skip_whitespace(chars);
        if chars.next() != Some(':') {
            return Err("expected ':' after object key".to_string());
        }
        let value = parse_value(chars)?;
        pairs.push((key, value));
        skip_whitespace(chars);
        match chars.next() {
            Some(',') => continue,
            Some('}') => break,
            _ => return Err("expected ',' or '}' in object".to_string()),
        }
    }
    Ok(Value::Object(pairs))
}

fn escape_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

pub fn parse_card_line(line: &str) -> Result<Card, String> {
    let value = parse(line)?;
    let front = value
        .get("front")
        .and_then(Value::as_str)
        .ok_or("missing \"front\" field")?
        .to_string();
    let back = value
        .get("back")
        .and_then(Value::as_str)
        .ok_or("missing \"back\" field")?
        .to_string();
    let tags = value
        .get("tags")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();
    let media = value
        .get("media")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();
    let due = value
        .get("due")
        .and_then(Value::as_str)
        .map(|s| s.to_string());
    let interval_days = value
        .get("interval_days")
        .and_then(Value::as_f64)
        .map(|n| n as u32);
    let ease = value.get("ease").and_then(Value::as_f64);
    let reps = value.get("reps").and_then(Value::as_f64).map(|n| n as u32);
    let lapses = value
        .get("lapses")
        .and_then(Value::as_f64)
        .map(|n| n as u32);

    Ok(Card {
        front,
        back,
        tags,
        media,
        due,
        interval_days,
        ease,
        reps,
        lapses,
    })
}

pub fn write_card_line<W: io::Write>(w: &mut W, card: &Card) -> io::Result<()> {
    let mut out = String::from("{\"front\":");
    escape_string(&card.front, &mut out);
    out.push_str(",\"back\":");
    escape_string(&card.back, &mut out);
    out.push_str(",\"tags\":[");
    for (i, tag) in card.tags.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        escape_string(tag, &mut out);
    }
    out.push_str("],\"media\":[");
    for (i, filename) in card.media.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        escape_string(filename, &mut out);
    }
    out.push(']');
    if let Some(due) = &card.due {
        out.push_str(",\"due\":");
        escape_string(due, &mut out);
    }
    if let Some(interval) = card.interval_days {
        out.push_str(&format!(",\"interval_days\":{interval}"));
    }
    if let Some(ease) = card.ease {
        out.push_str(&format!(",\"ease\":{ease}"));
    }
    if let Some(reps) = card.reps {
        out.push_str(&format!(",\"reps\":{reps}"));
    }
    if let Some(lapses) = card.lapses {
        out.push_str(&format!(",\"lapses\":{lapses}"));
    }
    out.push('}');
    writeln!(w, "{out}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_to_string(card: &Card) -> String {
        let mut buf = Vec::new();
        write_card_line(&mut buf, card).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn round_trips_a_fully_scheduled_card() {
        let line = r#"{"front":"mitochondria","back":"powerhouse of the cell","tags":["biology"],"media":["cell.png"],"due":"2026-08-24","interval_days":3,"ease":2.5,"reps":4,"lapses":1}"#;
        let card = parse_card_line(line).unwrap();
        assert_eq!(write_to_string(&card), format!("{line}\n"));
    }

    #[test]
    fn round_trips_a_new_card_with_no_schedule_fields() {
        let line = r#"{"front":"2+2","back":"4","tags":[],"media":[]}"#;
        let card = parse_card_line(line).unwrap();
        assert_eq!(write_to_string(&card), format!("{line}\n"));
    }

    #[test]
    fn round_trips_escaped_characters_through_two_passes() {
        let card = Card::new_unscheduled(
            "line one\nline two".to_string(),
            "a \"quoted\" word".to_string(),
            vec!["needs-escaping\\tag".to_string()],
            Vec::new(),
        );
        let first_pass = write_to_string(&card);
        let reparsed = parse_card_line(first_pass.trim_end()).unwrap();
        assert_eq!(reparsed.front, card.front);
        assert_eq!(reparsed.back, card.back);
        assert_eq!(reparsed.tags, card.tags);
        assert_eq!(write_to_string(&reparsed), first_pass);
    }

    #[test]
    fn parse_card_line_requires_front_and_back() {
        assert!(parse_card_line(r#"{"back":"only back"}"#).is_err());
        assert!(parse_card_line(r#"{"front":"only front"}"#).is_err());
    }
}
