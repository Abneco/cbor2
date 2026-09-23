use alloc::{format, string::String, vec::Vec};

use crate::de::Error;
use crate::value::Value;

use super::encode::{
    ellipsis_item, write_array_len, write_definite_text, write_elided_string, write_tag,
};
use super::types::{Arg, Atom, ElidedStringPart, Indicator, ELLIPSIS_TAG, UNRESOLVED_APP_TAG};

pub(super) fn unresolved_app_sequence(prefix: &str, args: Vec<Arg>) -> Result<Atom, Error> {
    let mut out = Vec::new();
    write_tag(&mut out, UNRESOLVED_APP_TAG)?;
    write_array_len(&mut out, 2)?;
    write_definite_text(&mut out, prefix, Indicator::None)?;
    write_array_len(&mut out, args.len())?;
    for arg in args {
        out.extend_from_slice(&arg.into_encoded()?);
    }
    Ok(Atom::Raw(out))
}

fn push_elided_part(parts: &mut Vec<ElidedStringPart>, part: ElidedStringPart) {
    match (parts.last_mut(), part) {
        (Some(ElidedStringPart::Bytes(prev)), ElidedStringPart::Bytes(bytes)) => {
            prev.extend_from_slice(&bytes);
        }
        (Some(ElidedStringPart::Text(prev)), ElidedStringPart::Text(text)) => {
            prev.push_str(&text);
        }
        (Some(ElidedStringPart::Ellipsis), ElidedStringPart::Ellipsis) => {}
        (_, part) => parts.push(part),
    }
}

pub(super) fn concat_app_strings(
    prefix: &str,
    args: Vec<Arg>,
    offset: usize,
) -> Result<Atom, Error> {
    let mut concat = StringConcat {
        text: prefix == "t1",
        bytes: Vec::new(),
        parts: Vec::new(),
        elided: false,
    };
    for arg in args {
        concat.collect(arg.into_value()?, offset)?;
    }
    if !concat.elided {
        return if concat.text {
            String::from_utf8(concat.bytes)
                .map(Atom::Text)
                .map_err(|_| Error::semantic(offset, "t1 result is not UTF-8"))
        } else {
            Ok(Atom::Bytes(concat.bytes))
        };
    }
    concat.flush(offset)?;
    if concat
        .parts
        .iter()
        .all(|part| matches!(part, ElidedStringPart::Ellipsis))
    {
        return Ok(Atom::Raw(ellipsis_item()));
    }
    let mut out = Vec::new();
    write_elided_string(&mut out, &concat.parts)?;
    Ok(Atom::Raw(out))
}

struct StringConcat {
    text: bool,
    bytes: Vec<u8>,
    parts: Vec<ElidedStringPart>,
    elided: bool,
}

impl StringConcat {
    fn flush(&mut self, offset: usize) -> Result<(), Error> {
        if self.bytes.is_empty() {
            return Ok(());
        }
        let bytes = core::mem::take(&mut self.bytes);
        let part = if self.text {
            ElidedStringPart::Text(
                String::from_utf8(bytes)
                    .map_err(|_| Error::semantic(offset, "t1 span is not UTF-8"))?,
            )
        } else {
            ElidedStringPart::Bytes(bytes)
        };
        push_elided_part(&mut self.parts, part);
        Ok(())
    }
    fn collect(&mut self, value: Value, offset: usize) -> Result<(), Error> {
        match value {
            Value::Text(text) => self.bytes.extend_from_slice(text.as_bytes()),
            Value::Bytes(bytes) => self.bytes.extend_from_slice(&bytes),
            Value::Tag(ELLIPSIS_TAG, inner) => {
                self.elided = true;
                match *inner {
                    Value::Null => {
                        self.flush(offset)?;
                        push_elided_part(&mut self.parts, ElidedStringPart::Ellipsis);
                    }
                    Value::Array(items) => {
                        for item in items {
                            self.collect(item, offset)?;
                        }
                    }
                    _ => {
                        return Err(Error::semantic(
                            offset,
                            "ellipsis must contain null or string parts",
                        ))
                    }
                }
            }
            _ => {
                return Err(Error::semantic(
                    offset,
                    "concatenation arguments must be strings or ellipses",
                ))
            }
        }
        Ok(())
    }
}

pub(super) fn concat_bytes(args: Vec<Arg>, offset: usize) -> Result<Atom, Error> {
    let mut out = Vec::new();

    for arg in args {
        match arg.into_value()? {
            Value::Text(text) => out.extend_from_slice(text.as_bytes()),
            Value::Bytes(bytes) => out.extend_from_slice(&bytes),
            _ => return Err(Error::semantic(offset, "bytes arguments must be strings")),
        }
    }

    Ok(Atom::Bytes(out))
}

pub(super) fn same_args(args: Vec<Arg>, offset: usize) -> Result<Atom, Error> {
    let mut args = args.into_iter();
    let Some(first) = args.next() else {
        return Err(Error::semantic(
            offset,
            "same expects at least one argument",
        ));
    };

    let encoded = first.into_encoded()?;
    let first = crate::de::value_from_slice(&encoded)?;
    for arg in args {
        if !values_same(&first, &arg.into_value()?) {
            return Err(Error::semantic(offset, "same arguments are not equal"));
        }
    }

    // `same` checks data model equality but preserves the first spelling.
    Ok(Atom::Raw(encoded))
}

fn values_same(left: &Value, right: &Value) -> bool {
    values_equal(left, right, false)
}

fn values_equal(left: &Value, right: &Value, key: bool) -> bool {
    match (left, right) {
        (Value::Integer(left), Value::Integer(right)) => left == right,
        (Value::Bytes(left), Value::Bytes(right)) => left == right,
        (Value::Float(left), Value::Float(right)) => {
            if key {
                left == right
                    || (left.is_nan()
                        && right.is_nan()
                        && (left.to_bits() & 0x000f_ffff_ffff_ffff)
                            == (right.to_bits() & 0x000f_ffff_ffff_ffff))
            } else {
                left.to_bits() == right.to_bits()
            }
        }
        (Value::Text(left), Value::Text(right)) => left == right,
        (Value::Bool(left), Value::Bool(right)) => left == right,
        (Value::Null, Value::Null) => true,
        (Value::Tag(left_tag, left), Value::Tag(right_tag, right)) => {
            left_tag == right_tag && values_equal(left, right, key)
        }
        (Value::Array(left), Value::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(left, right)| values_equal(left, right, key))
        }
        (Value::Map(left), Value::Map(right)) => {
            if left.len() != right.len() {
                return false;
            }
            let mut matched = alloc::vec![false; right.len()];
            left.iter().all(|(left_key, value)| {
                let found = right.iter().enumerate().position(|(i, (k, v))| {
                    !matched[i] && values_equal(left_key, k, true) && values_equal(value, v, key)
                });
                if let Some(i) = found {
                    matched[i] = true;
                    true
                } else {
                    false
                }
            })
        }
        (Value::Simple(left), Value::Simple(right)) => left == right,
        _ => false,
    }
}

pub(super) fn hex_atom(content: &str, offset: usize) -> Result<Atom, Error> {
    match hex_content_parts(content, offset)? {
        HexParsed::Complete(bytes) => Ok(Atom::Bytes(bytes)),
        HexParsed::Elided(parts) => {
            let mut out = Vec::new();
            write_elided_string(&mut out, &parts)?;
            Ok(Atom::Raw(out))
        }
    }
}

pub(super) fn hex_content(content: &str, offset: usize) -> Result<Vec<u8>, Error> {
    let mut parser = HexContent {
        input: content,
        pos: 0,
        base_offset: offset.saturating_sub(content.len()),
    };
    match parser.parse()? {
        HexParsed::Complete(bytes) => Ok(bytes),
        HexParsed::Elided(..) => Err(Error::semantic(
            offset,
            "CDN ellipses are not supported here",
        )),
    }
}

fn hex_content_parts(content: &str, offset: usize) -> Result<HexParsed, Error> {
    let mut parser = HexContent {
        input: content,
        pos: 0,
        base_offset: offset.saturating_sub(content.len()),
    };
    parser.parse()
}

struct HexContent<'a> {
    input: &'a str,
    pos: usize,
    base_offset: usize,
}

impl HexContent<'_> {
    fn syntax(&self) -> Error {
        Error::Syntax(self.base_offset + self.pos)
    }

    fn rest(&self) -> &str {
        &self.input[self.pos..]
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn eat(&mut self, s: &str) -> bool {
        if self.rest().starts_with(s) {
            self.pos += s.len();
            true
        } else {
            false
        }
    }

    fn skip(&mut self) -> Result<(), Error> {
        loop {
            let before = self.pos;
            while matches!(self.peek(), Some('\n' | ' ')) {
                self.bump();
            }
            if self.eat("//") || self.eat("#") {
                while let Some(ch) = self.bump() {
                    if ch == '\n' {
                        break;
                    }
                }
                continue;
            }
            if self.eat("/*") {
                let Some(end) = self.rest().find("*/") else {
                    return Err(self.syntax());
                };
                self.pos += end + 2;
                continue;
            }
            if self.rest().starts_with('/') {
                self.bump();
                let Some(end) = self.rest().find('/') else {
                    return Err(self.syntax());
                };
                self.pos += end + 1;
                continue;
            }
            if before == self.pos {
                return Ok(());
            }
        }
    }

    fn parse(&mut self) -> Result<HexParsed, Error> {
        let mut bytes = Vec::new();
        let mut high = None;
        let mut parts = Vec::new();
        let mut saw_elision = false;
        loop {
            self.skip()?;
            if self.pos == self.input.len() {
                break;
            }
            if self.rest().starts_with("...") {
                if high.is_some() {
                    return Err(self.syntax());
                }
                flush_hex_bytes(&mut bytes, &mut parts);
                while self.eat(".") {}
                saw_elision = true;
                push_elided_part(&mut parts, ElidedStringPart::Ellipsis);
                continue;
            }
            let ch = self.bump().ok_or_else(|| self.syntax())?;
            let Some(digit) = ch.to_digit(16) else {
                return Err(self.syntax());
            };
            match high.take() {
                Some(high) => bytes.push((high << 4) | digit as u8),
                None => high = Some(digit as u8),
            }
        }
        if high.is_some() {
            return Err(self.syntax());
        }
        if saw_elision {
            flush_hex_bytes(&mut bytes, &mut parts);
            Ok(HexParsed::Elided(parts))
        } else {
            Ok(HexParsed::Complete(bytes))
        }
    }
}

enum HexParsed {
    Complete(Vec<u8>),
    Elided(Vec<ElidedStringPart>),
}

fn flush_hex_bytes(bytes: &mut Vec<u8>, parts: &mut Vec<ElidedStringPart>) {
    if !bytes.is_empty() {
        push_elided_part(parts, ElidedStringPart::Bytes(core::mem::take(bytes)));
    }
}

pub(super) fn base64_content(content: &str, offset: usize) -> Result<Vec<u8>, Error> {
    let mut filtered = Vec::new();
    let mut chars = content.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        match ch {
            '\n' | ' ' => {}
            '#' => {
                for (_, next) in chars.by_ref() {
                    if next == '\n' {
                        break;
                    }
                }
            }
            '=' => filtered.push(64),
            _ => {
                let Some(value) = b64_value(ch) else {
                    return Err(Error::Syntax(offset.saturating_sub(content.len()) + idx));
                };
                filtered.push(value);
            }
        }
    }

    if let Some(first_pad) = filtered.iter().position(|&value| value == 64) {
        if filtered[first_pad..].iter().any(|&value| value != 64) {
            return Err(Error::semantic(offset, "invalid base64 padding"));
        }
        let pad_count = filtered.len() - first_pad;
        if pad_count > 2 || filtered.len() % 4 != 0 {
            return Err(Error::semantic(offset, "invalid base64 padding"));
        }
        match (first_pad % 4, pad_count) {
            (2, 2) | (3, 1) => filtered.truncate(first_pad),
            _ => return Err(Error::semantic(offset, "invalid base64 padding")),
        }
    } else if filtered.len() % 4 == 1 {
        return Err(Error::semantic(offset, "invalid base64 length"));
    }

    let mut out = Vec::new();
    for chunk in filtered.chunks(4) {
        let a = chunk[0];
        let b = *chunk
            .get(1)
            .ok_or_else(|| Error::semantic(offset, "invalid base64"))?;
        if a == 64 || b == 64 {
            return Err(Error::semantic(offset, "invalid base64 padding"));
        }
        out.push((a << 2) | (b >> 4));
        if let Some(&c) = chunk.get(2) {
            if c == 64 {
                continue;
            }
            out.push((b << 4) | (c >> 2));
            if let Some(&d) = chunk.get(3) {
                if d == 64 {
                    continue;
                }
                out.push((c << 6) | d);
            }
        }
    }
    Ok(out)
}

pub(super) fn append_indefinite_string_chunk(
    out: &mut Vec<u8>,
    prefix: &str,
    want_major: u8,
    arg: Arg,
    offset: usize,
) -> Result<(), Error> {
    let encoded = arg.into_encoded()?;
    let Some(&head) = encoded.first() else {
        return Err(Error::Syntax(offset));
    };
    let source_major = head >> 5;
    if !matches!(source_major, 2 | 3) || (head & 0x1f) == 31 {
        return Err(Error::semantic(
            offset,
            format!("{prefix} arguments must be definite strings"),
        ));
    }

    if want_major == 3 {
        match crate::de::value_from_slice(&encoded)? {
            Value::Text(..) => {}
            Value::Bytes(bytes) => {
                core::str::from_utf8(&bytes)
                    .map_err(|_| Error::semantic(offset, "ilts argument is not UTF-8"))?;
            }
            _ => unreachable!("string major type decoded as non-string value"),
        }
    }

    let mut encoded = encoded;
    encoded[0] = (want_major << 5) | (head & 0x1f);
    out.extend_from_slice(&encoded);
    Ok(())
}

fn b64_value(ch: char) -> Option<u8> {
    match ch {
        'A'..='Z' => Some(ch as u8 - b'A'),
        'a'..='z' => Some(ch as u8 - b'a' + 26),
        '0'..='9' => Some(ch as u8 - b'0' + 52),
        '+' | '-' => Some(62),
        '/' | '_' => Some(63),
        _ => None,
    }
}

pub(super) fn one_text_arg(prefix: &str, args: Vec<Arg>, offset: usize) -> Result<String, Error> {
    if args.len() != 1 {
        return Err(Error::semantic(
            offset,
            format!("{prefix} expects exactly one argument"),
        ));
    }
    match args.into_iter().next().unwrap().into_value()? {
        Value::Text(s) => Ok(s),
        Value::Bytes(b) => String::from_utf8(b)
            .map_err(|_| Error::semantic(offset, format!("{prefix} argument is not UTF-8"))),
        _ => Err(Error::semantic(
            offset,
            format!("{prefix} argument must be a string"),
        )),
    }
}

pub(super) fn one_bytes_arg(prefix: &str, args: Vec<Arg>, offset: usize) -> Result<Vec<u8>, Error> {
    if args.len() != 1 {
        return Err(Error::semantic(
            offset,
            format!("{prefix} expects exactly one argument"),
        ));
    }
    match args.into_iter().next().unwrap().into_value()? {
        Value::Text(s) => hex_content(&s, offset),
        Value::Bytes(b) => Ok(b),
        _ => Err(Error::semantic(
            offset,
            format!("{prefix} argument must be a string"),
        )),
    }
}
