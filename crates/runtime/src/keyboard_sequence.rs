use std::char;
use std::str::FromStr;

use pest::Parser;
use pest::iterators::Pair;
use pest_derive::Parser;
use platynui_core::platform::{KeyCode, KeyboardDevice, KeyboardError};
use thiserror::Error;

#[derive(Parser)]
#[grammar = "keyboard_sequence.pest"]
pub struct KeyboardSequenceParser;

#[derive(Debug, Error)]
pub enum KeyboardSequenceError {
    #[error("sequence parse error: {0}")]
    Parse(Box<pest::error::Error<Rule>>),
    #[error("dangling escape sequence at end of input")]
    DanglingEscape,
    #[error("invalid hex escape \\x{literal}")]
    InvalidHexEscape { literal: String },
    #[error("invalid unicode escape \\u{literal}")]
    InvalidUnicodeEscape { literal: String },
    #[error("shortcut contains an empty key name")]
    EmptyKey,
}

impl From<pest::error::Error<Rule>> for KeyboardSequenceError {
    fn from(value: pest::error::Error<Rule>) -> Self {
        KeyboardSequenceError::Parse(Box::new(value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SequenceSegment {
    Text(String),
    Shortcut(Vec<Vec<String>>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardSequence {
    segments: Vec<SequenceSegment>,
}

impl KeyboardSequence {
    pub fn parse(input: &str) -> Result<Self, KeyboardSequenceError> {
        let mut pairs = KeyboardSequenceParser::parse(Rule::sequence, input)?;
        let sequence = pairs.next().expect("sequence root");
        let mut segments = Vec::new();
        for pair in sequence.into_inner() {
            match pair.as_rule() {
                Rule::segment | Rule::shortcut | Rule::text => {
                    segments.push(parse_segment(pair)?);
                }
                Rule::EOI => {}
                _ => {}
            }
        }
        Ok(Self { segments })
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    pub fn segments(&self) -> &[SequenceSegment] {
        &self.segments
    }

    pub fn resolve(&self, device: &dyn KeyboardDevice) -> Result<ResolvedKeyboardSequence, KeyboardError> {
        let mut resolved = Vec::with_capacity(self.segments.len());
        for segment in &self.segments {
            match segment {
                SequenceSegment::Text(text) => {
                    let mut codes = Vec::with_capacity(text.chars().count());
                    for ch in text.chars() {
                        let name = ch.to_string();
                        let code = device.key_to_code(&name)?;
                        codes.push(code);
                    }
                    resolved.push(ResolvedSegment::Text(codes));
                }
                SequenceSegment::Shortcut(groups) => {
                    let mut resolved_groups = Vec::with_capacity(groups.len());
                    for group in groups {
                        let mut codes = Vec::with_capacity(group.len());
                        for key in group {
                            let code = device.key_to_code(key)?;
                            codes.push(code);
                        }
                        resolved_groups.push(codes);
                    }
                    resolved.push(ResolvedSegment::Shortcut(resolved_groups));
                }
            }
        }
        Ok(ResolvedKeyboardSequence { segments: resolved })
    }
}

impl FromStr for KeyboardSequence {
    type Err = KeyboardSequenceError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedKeyboardSequence {
    segments: Vec<ResolvedSegment>,
}

impl ResolvedKeyboardSequence {
    pub fn segments(&self) -> &[ResolvedSegment] {
        &self.segments
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedSegment {
    Text(Vec<KeyCode>),
    Shortcut(Vec<Vec<KeyCode>>),
}

impl ResolvedSegment {
    pub fn text_codes(&self) -> Option<&[KeyCode]> {
        match self {
            ResolvedSegment::Text(codes) => Some(codes.as_slice()),
            _ => None,
        }
    }

    pub fn shortcut_combinations(&self) -> Option<&[Vec<KeyCode>]> {
        match self {
            ResolvedSegment::Shortcut(groups) => Some(groups.as_slice()),
            _ => None,
        }
    }
}

fn parse_segment(pair: Pair<Rule>) -> Result<SequenceSegment, KeyboardSequenceError> {
    match pair.as_rule() {
        Rule::segment => {
            let inner = pair.into_inner().next().expect("segment inner value");
            parse_segment(inner)
        }
        Rule::text => {
            let decoded = decode_escapes(pair.as_str())?;
            if decoded.is_empty() {
                Ok(SequenceSegment::Text(String::new()))
            } else {
                Ok(SequenceSegment::Text(decoded))
            }
        }
        Rule::shortcut => parse_shortcut(pair),
        _ => unreachable!("unexpected rule in parse_segment"),
    }
}

fn parse_shortcut(pair: Pair<Rule>) -> Result<SequenceSegment, KeyboardSequenceError> {
    let mut groups = Vec::new();
    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::combination {
            let mut keys = Vec::new();
            for key_pair in inner.into_inner() {
                if key_pair.as_rule() == Rule::key {
                    let name = decode_escapes(key_pair.as_str())?;
                    if name.is_empty() {
                        return Err(KeyboardSequenceError::EmptyKey);
                    }
                    keys.push(name);
                }
            }
            if keys.is_empty() {
                return Err(KeyboardSequenceError::EmptyKey);
            }
            groups.push(keys);
        }
    }
    if groups.is_empty() { Err(KeyboardSequenceError::EmptyKey) } else { Ok(SequenceSegment::Shortcut(groups)) }
}

fn decode_escapes(source: &str) -> Result<String, KeyboardSequenceError> {
    let mut result = String::with_capacity(source.len());
    let mut chars = source.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            result.push(ch);
            continue;
        }
        let next = chars.next().ok_or(KeyboardSequenceError::DanglingEscape)?;
        match next {
            'x' => {
                let hi = chars.next().ok_or(KeyboardSequenceError::InvalidHexEscape { literal: String::from("") })?;
                let lo = chars.next().ok_or(KeyboardSequenceError::InvalidHexEscape { literal: hi.to_string() })?;
                let literal = format!("{hi}{lo}");
                let value = u8::from_str_radix(&literal, 16)
                    .map_err(|_| KeyboardSequenceError::InvalidHexEscape { literal })?;
                result.push(value as char);
            }
            'u' => {
                let mut digits = String::new();
                for _ in 0..4 {
                    let digit = chars
                        .next()
                        .ok_or_else(|| KeyboardSequenceError::InvalidUnicodeEscape { literal: digits.clone() })?;
                    digits.push(digit);
                }
                let value = u16::from_str_radix(&digits, 16)
                    .map_err(|_| KeyboardSequenceError::InvalidUnicodeEscape { literal: digits.clone() })?;
                let ch = char::from_u32(value as u32)
                    .ok_or_else(|| KeyboardSequenceError::InvalidUnicodeEscape { literal: digits.clone() })?;
                result.push(ch);
            }
            '<' => result.push('<'),
            '>' => result.push('>'),
            '\\' => result.push('\\'),
            other => result.push(other),
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{
        KeyboardSequence, KeyboardSequenceError, KeyboardSequenceParser, ResolvedSegment, Rule, SequenceSegment,
    };
    use pest::Parser;
    use platynui_core::platform::{KeyCode, KeyboardDevice, KeyboardError, KeyboardEvent};

    #[test]
    fn parse_plain_text() {
        let pairs = KeyboardSequenceParser::parse(Rule::sequence, "hello world").unwrap();
        assert!(pairs.into_iter().next().is_some());
    }

    #[test]
    fn parse_text_with_backslash_escape() {
        let input = "foo\\<bar \\u263A baz";
        let sequence = KeyboardSequence::parse(input).unwrap();
        assert_eq!(sequence.segments().len(), 1);
        assert_eq!(sequence.segments()[0], SequenceSegment::Text("foo<bar ☺ baz".into()));
    }

    #[test]
    fn parse_shortcut_combo() {
        let input = "<Ctrl+Alt+T Ctrl+C>";
        let sequence = KeyboardSequence::parse(input).unwrap();
        assert_eq!(sequence.segments().len(), 1);
        match &sequence.segments()[0] {
            SequenceSegment::Shortcut(groups) => {
                assert_eq!(groups.len(), 2);
                assert_eq!(groups[0], vec![String::from("Ctrl"), String::from("Alt"), String::from("T"),]);
                assert_eq!(groups[1], vec![String::from("Ctrl"), String::from("C")]);
            }
            other => panic!("unexpected segment {other:?}"),
        }
    }

    #[test]
    fn parse_shortcut_with_escape_sequences() {
        let input = "<Ctrl+\\<+\\x41+\\u0042>";
        let sequence = KeyboardSequence::parse(input).unwrap();
        match &sequence.segments()[0] {
            SequenceSegment::Shortcut(groups) => {
                assert_eq!(
                    groups[0],
                    vec![String::from("Ctrl"), String::from("<"), String::from("A"), String::from("B"),]
                );
            }
            _ => panic!("expected shortcut segment"),
        }
    }

    #[test]
    fn reject_unfinished_block() {
        let input = "<Ctrl";
        assert!(matches!(KeyboardSequence::parse(input), Err(KeyboardSequenceError::Parse(_))));
    }

    struct StubKeyboard;

    impl KeyboardDevice for StubKeyboard {
        fn key_to_code(&self, name: &str) -> Result<KeyCode, KeyboardError> {
            Ok(KeyCode::new(name.to_string()))
        }

        fn send_key_event(&self, _event: KeyboardEvent) -> Result<(), KeyboardError> {
            Ok(())
        }
    }

    #[test]
    fn resolve_sequence_maps_keys() {
        let sequence = KeyboardSequence::parse("Hi<Ctrl+A>").unwrap();
        let resolved = sequence.resolve(&StubKeyboard).unwrap();
        assert_eq!(resolved.segments().len(), 2);
        match &resolved.segments()[0] {
            ResolvedSegment::Text(codes) => {
                assert_eq!(codes.len(), 2);
            }
            _ => panic!("expected text segment"),
        }
        match &resolved.segments()[1] {
            ResolvedSegment::Shortcut(groups) => {
                assert_eq!(groups.len(), 1);
                assert_eq!(groups[0].len(), 2);
            }
            _ => panic!("expected shortcut"),
        }
    }

    #[test]
    fn decode_hex_escape() {
        let sequence = KeyboardSequence::parse("\\x41").unwrap();
        match &sequence.segments()[0] {
            SequenceSegment::Text(text) => assert_eq!(text, "A"),
            _ => panic!("expected text"),
        }
    }

    #[test]
    fn decode_unicode_escape() {
        let sequence = KeyboardSequence::parse("\\u00E4").unwrap();
        match &sequence.segments()[0] {
            SequenceSegment::Text(text) => assert_eq!(text, "ä"),
            _ => panic!("expected text"),
        }
    }
}
