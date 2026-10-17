//! Constrained canonical JSON for the inference contract (MVP-A §7, §18).
//!
//! This is **not** RFC 8785. It is a deliberately narrow subset whose every
//! rule is stated here and tested, so that two implementations produce the
//! same bytes for the same document without an external specification:
//!
//! | Construct | Rule |
//! |---|---|
//! | object | members sorted by key, byte-wise ascending over the key's UTF-8; no duplicate keys |
//! | array | element order preserved |
//! | string | UTF-8; `"` → `\"`, `\` → `\\`, U+0000–U+001F → `\u00xx` with lowercase hex; every other scalar raw |
//! | integer | non-negative, decimal, no leading zeros, at most `u64::MAX`; the only number form |
//! | whitespace | none |
//! | `true`, `false`, `null`, negative or fractional numbers, exponents | **rejected** on parse, never produced |
//! | nesting | at most [`MAX_DEPTH`] levels |
//!
//! The parser accepts any RFC 8259 document that stays inside those
//! constraints (whitespace, escapes and surrogate pairs included) and
//! rejects everything else, so `canonicalize(text)` is total over the
//! accepted set and idempotent: `canonicalize(serialize(v)) == serialize(v)`.
//! Nothing here is consensus; the chain never sees these bytes
//! (RFC 0005 §2.12).

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Maximum nesting depth accepted by the parser.
pub const MAX_DEPTH: usize = 16;

/// A value in the constrained canonical model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// A UTF-8 string.
    String(String),
    /// A non-negative integer.
    Uint(u64),
    /// An ordered list.
    Array(Vec<Value>),
    /// A map with byte-sorted keys.
    Object(BTreeMap<String, Value>),
}

/// Why a document is outside the constrained model. Carries positions, never
/// content.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CanonicalError {
    /// Not well-formed JSON, or a construct the model excludes.
    #[error("invalid JSON at byte {0}: {1}")]
    Syntax(usize, &'static str),
    /// A key appears twice in one object.
    #[error("duplicate key at byte {0}")]
    DuplicateKey(usize),
    /// Nesting deeper than [`MAX_DEPTH`].
    #[error("nesting deeper than {MAX_DEPTH}")]
    TooDeep,
    /// Document longer than the caller's bound.
    #[error("document longer than {0} bytes")]
    TooLong(usize),
}

/// Parses `text` into the constrained model. `max_bytes` bounds the input.
pub fn parse(text: &str, max_bytes: usize) -> Result<Value, CanonicalError> {
    if text.len() > max_bytes {
        return Err(CanonicalError::TooLong(max_bytes));
    }
    let mut p = Parser {
        s: text.as_bytes(),
        i: 0,
    };
    p.ws();
    let v = p.value(0)?;
    p.ws();
    if p.i != p.s.len() {
        return Err(CanonicalError::Syntax(p.i, "trailing characters"));
    }
    Ok(v)
}

/// Serializes `v` to its one canonical byte form.
pub fn serialize(v: &Value) -> Vec<u8> {
    let mut out = String::new();
    write_value(&mut out, v);
    out.into_bytes()
}

/// `serialize(parse(text))`: the canonical bytes of any accepted document.
pub fn canonicalize(text: &str, max_bytes: usize) -> Result<Vec<u8>, CanonicalError> {
    Ok(serialize(&parse(text, max_bytes)?))
}

/// Whether `bytes` are already canonical: they parse, and re-serialize to
/// themselves.
pub fn is_canonical(bytes: &[u8], max_bytes: usize) -> bool {
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|t| parse(t, max_bytes).ok())
        .is_some_and(|v| serialize(&v) == bytes)
}

fn write_value(out: &mut String, v: &Value) {
    match v {
        Value::String(s) => write_string(out, s),
        Value::Uint(n) => {
            let _ = write!(out, "{n}");
        }
        Value::Array(items) => {
            out.push('[');
            for (k, item) in items.iter().enumerate() {
                if k > 0 {
                    out.push(',');
                }
                write_value(out, item);
            }
            out.push(']');
        }
        Value::Object(members) => {
            out.push('{');
            for (k, (key, item)) in members.iter().enumerate() {
                if k > 0 {
                    out.push(',');
                }
                write_string(out, key);
                out.push(':');
                write_value(out, item);
            }
            out.push('}');
        }
    }
}

fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn expect(&mut self, b: u8, what: &'static str) -> Result<(), CanonicalError> {
        if self.peek() == Some(b) {
            self.i += 1;
            Ok(())
        } else {
            Err(CanonicalError::Syntax(self.i, what))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, CanonicalError> {
        if depth > MAX_DEPTH {
            return Err(CanonicalError::TooDeep);
        }
        match self.peek() {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => self.string().map(Value::String),
            Some(b'0'..=b'9') => self.uint().map(Value::Uint),
            Some(b't' | b'f' | b'n') => Err(CanonicalError::Syntax(
                self.i,
                "true, false and null are outside the constrained model",
            )),
            Some(b'-') => Err(CanonicalError::Syntax(
                self.i,
                "negative numbers are not allowed",
            )),
            Some(_) => Err(CanonicalError::Syntax(self.i, "unexpected character")),
            None => Err(CanonicalError::Syntax(self.i, "unexpected end of input")),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, CanonicalError> {
        self.expect(b'{', "expected '{'")?;
        let mut members = BTreeMap::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(Value::Object(members));
        }
        loop {
            self.ws();
            let key_at = self.i;
            if self.peek() != Some(b'"') {
                return Err(CanonicalError::Syntax(self.i, "expected a string key"));
            }
            let key = self.string()?;
            self.ws();
            self.expect(b':', "expected ':'")?;
            self.ws();
            let v = self.value(depth + 1)?;
            if members.insert(key, v).is_some() {
                return Err(CanonicalError::DuplicateKey(key_at));
            }
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Value::Object(members));
                }
                _ => return Err(CanonicalError::Syntax(self.i, "expected ',' or '}'")),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value, CanonicalError> {
        self.expect(b'[', "expected '['")?;
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.ws();
            items.push(self.value(depth + 1)?);
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(CanonicalError::Syntax(self.i, "expected ',' or ']'")),
            }
        }
    }

    fn uint(&mut self) -> Result<u64, CanonicalError> {
        let start = self.i;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.i += 1;
        }
        let digits = &self.s[start..self.i];
        if digits.len() > 1 && digits[0] == b'0' {
            return Err(CanonicalError::Syntax(start, "leading zero"));
        }
        if matches!(self.peek(), Some(b'.' | b'e' | b'E')) {
            return Err(CanonicalError::Syntax(
                self.i,
                "fractions and exponents are not allowed",
            ));
        }
        std::str::from_utf8(digits)
            .ok()
            .and_then(|d| d.parse::<u64>().ok())
            .ok_or(CanonicalError::Syntax(start, "integer out of range"))
    }

    fn string(&mut self) -> Result<String, CanonicalError> {
        self.expect(b'"', "expected '\"'")?;
        let mut out = String::new();
        loop {
            let at = self.i;
            match self.peek() {
                None => return Err(CanonicalError::Syntax(at, "unterminated string")),
                Some(b'"') => {
                    self.i += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.i += 1;
                    let esc =
                        self.peek().ok_or(CanonicalError::Syntax(at, "unterminated escape"))?;
                    self.i += 1;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4(at)?;
                            let c = if (0xD800..0xDC00).contains(&hi) {
                                // High surrogate: a `\u` low surrogate must follow.
                                if self.peek() != Some(b'\\')
                                    || self.s.get(self.i + 1) != Some(&b'u')
                                {
                                    return Err(CanonicalError::Syntax(at, "lone high surrogate"));
                                }
                                self.i += 2;
                                let lo = self.hex4(at)?;
                                if !(0xDC00..0xE000).contains(&lo) {
                                    return Err(CanonicalError::Syntax(
                                        at,
                                        "invalid low surrogate",
                                    ));
                                }
                                0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                            } else if (0xDC00..0xE000).contains(&hi) {
                                return Err(CanonicalError::Syntax(at, "lone low surrogate"));
                            } else {
                                hi
                            };
                            out.push(
                                char::from_u32(c)
                                    .ok_or(CanonicalError::Syntax(at, "invalid code point"))?,
                            );
                        }
                        _ => return Err(CanonicalError::Syntax(at, "invalid escape")),
                    }
                }
                Some(b) if b < 0x20 => {
                    return Err(CanonicalError::Syntax(
                        at,
                        "raw control character in string",
                    ))
                }
                Some(_) => {
                    // Copy one whole UTF-8 scalar; the input is a `&str`, so
                    // the sequence is valid by construction.
                    let rest = std::str::from_utf8(&self.s[self.i..])
                        .map_err(|_| CanonicalError::Syntax(at, "invalid UTF-8"))?;
                    let c = rest
                        .chars()
                        .next()
                        .ok_or(CanonicalError::Syntax(at, "unexpected end of input"))?;
                    out.push(c);
                    self.i += c.len_utf8();
                }
            }
        }
    }

    fn hex4(&mut self, at: usize) -> Result<u32, CanonicalError> {
        let end = self.i + 4;
        let slice = self
            .s
            .get(self.i..end)
            .ok_or(CanonicalError::Syntax(at, "truncated \\u escape"))?;
        let text =
            std::str::from_utf8(slice).map_err(|_| CanonicalError::Syntax(at, "bad \\u escape"))?;
        let v = u32::from_str_radix(text, 16)
            .map_err(|_| CanonicalError::Syntax(at, "bad \\u escape"))?;
        self.i = end;
        Ok(v)
    }
}

/// Convenience constructors for building values in code.
pub mod build {
    use super::Value;
    use std::collections::BTreeMap;

    /// A string value.
    pub fn s(v: &str) -> Value {
        Value::String(v.to_owned())
    }
    /// An integer value.
    pub fn u(v: u64) -> Value {
        Value::Uint(v)
    }
    /// An object from `(key, value)` pairs; later duplicates overwrite.
    pub fn obj(pairs: Vec<(&str, Value)>) -> Value {
        let mut m = BTreeMap::new();
        for (k, v) in pairs {
            m.insert(k.to_owned(), v);
        }
        Value::Object(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_keys_and_strips_whitespace() {
        let bytes = canonicalize("{ \"b\" : 1 , \"a\" : [ \"x\" , {} ] }", 1024).unwrap();
        assert_eq!(bytes, br#"{"a":["x",{}],"b":1}"#);
        assert!(is_canonical(&bytes, 1024));
    }

    #[test]
    fn escapes_exactly_as_specified() {
        let v = Value::String("q\"b\\n\n\u{1}é😀".into());
        assert_eq!(serialize(&v), "\"q\\\"b\\\\n\\u000a\\u0001é😀\"".as_bytes());
        let round = parse(std::str::from_utf8(&serialize(&v)).unwrap(), 1024).unwrap();
        assert_eq!(round, v);
    }

    #[test]
    fn rejects_excluded_constructs() {
        for bad in [
            "true",
            "null",
            "-1",
            "1.5",
            "1e3",
            "01",
            "{\"a\":1,\"a\":2}",
            "[1,]",
            "\"\\x\"",
            "\"\\ud800\"",
            "{\"a\":1} x",
            "\"\u{1}\"",
            "18446744073709551616",
        ] {
            assert!(parse(bad, 1024).is_err(), "accepted {bad:?}");
        }
        assert!(parse("1", 0).is_err());
        let deep = "[".repeat(MAX_DEPTH + 2) + &"]".repeat(MAX_DEPTH + 2);
        assert_eq!(parse(&deep, 1024), Err(CanonicalError::TooDeep));
    }

    #[test]
    fn surrogate_pairs_decode() {
        let v = parse("\"\\ud83d\\ude00\"", 64).unwrap();
        assert_eq!(v, Value::String("😀".into()));
    }
}
