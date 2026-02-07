//! XML and string utility functions for the XPath evaluator.

use crate::xdm::XdmAtomicValue;

pub(crate) fn string_like_value(atom: &XdmAtomicValue) -> Option<String> {
    match atom {
        XdmAtomicValue::String(s)
        | XdmAtomicValue::UntypedAtomic(s)
        | XdmAtomicValue::NormalizedString(s)
        | XdmAtomicValue::Token(s)
        | XdmAtomicValue::Language(s)
        | XdmAtomicValue::Name(s)
        | XdmAtomicValue::NCName(s)
        | XdmAtomicValue::NMTOKEN(s)
        | XdmAtomicValue::Id(s)
        | XdmAtomicValue::IdRef(s)
        | XdmAtomicValue::Entity(s)
        | XdmAtomicValue::Notation(s)
        | XdmAtomicValue::AnyUri(s) => Some(s.clone()),
        _ => None,
    }
}

pub(crate) fn replace_xml_whitespace(input: &str) -> String {
    input
        .chars()
        .map(|ch| match ch {
            '\t' | '\n' | '\r' => ' ',
            other => other,
        })
        .collect()
}

pub(crate) fn collapse_xml_whitespace(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_space = false;
    for ch in input.chars() {
        if ch.is_whitespace() {
            if !in_space {
                out.push(' ');
                in_space = true;
            }
        } else {
            out.push(ch);
            in_space = false;
        }
    }
    while out.starts_with(' ') {
        out.remove(0);
    }
    while out.ends_with(' ') {
        out.pop();
    }
    out
}

pub(crate) fn is_valid_language(s: &str) -> bool {
    let mut parts = s.split('-');
    if let Some(first) = parts.next() {
        if !(1..=8).contains(&first.len()) || !first.chars().all(|c| c.is_ascii_alphabetic()) {
            return false;
        }
    } else {
        return false;
    }
    for part in parts {
        if part.is_empty() || part.len() > 8 || !part.chars().all(|c| c.is_ascii_alphanumeric()) {
            return false;
        }
    }
    true
}

pub(crate) fn is_name_start_char(ch: char, allow_colon: bool) -> bool {
    (allow_colon && ch == ':') || ch == '_' || ch.is_ascii_alphabetic()
}

pub(crate) fn is_name_char(ch: char, allow_colon: bool) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.' || (allow_colon && ch == ':')
}

pub(crate) fn is_valid_name(s: &str, require_start: bool, allow_colon: bool) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return !require_start;
    };
    if !is_name_start_char(first, allow_colon) {
        return false;
    }
    for ch in chars {
        if !is_name_char(ch, allow_colon) {
            return false;
        }
    }
    true
}

pub(crate) fn is_valid_nmtoken(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    s.chars().all(|ch| is_name_char(ch, true))
}

pub(crate) fn decode_hex(input: &str) -> Option<Vec<u8>> {
    if !input.len().is_multiple_of(2) {
        return None;
    }
    let mut bytes = Vec::with_capacity(input.len() / 2);
    let mut chars = input.chars();
    while let (Some(high_ch), Some(low_ch)) = (chars.next(), chars.next()) {
        let high = high_ch.to_digit(16)?;
        let low = low_ch.to_digit(16)?;
        bytes.push(((high << 4) | low) as u8);
    }
    Some(bytes)
}

pub(crate) fn encode_hex_upper(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{:02X}", byte));
    }
    out
}
