use std::str::FromStr;

use pest::Parser;
use pest::error::{ErrorVariant, InputLocation};
use pest::iterators::Pair;
use pest_derive::Parser;
use platynui_core::platform::{KeyCode, KeyboardDevice};
use thiserror::Error;

use crate::runtime::KeyboardActionError;

#[derive(Parser)]
#[grammar = "keyboard_sequence.pest"]
pub struct KeyboardSequenceParser;

/// How to write a character of the sequence syntax literally, as an error
/// suggests it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxHint {
    /// A literal `<` is written `\<`.
    LiteralLessThan,
    /// A literal backslash is written `\\`.
    LiteralBackslash,
}

impl SyntaxHint {
    /// The hint as an error message gives it.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            SyntaxHint::LiteralLessThan => "write \\< to type a literal <",
            SyntaxHint::LiteralBackslash => "write \\\\ to type a backslash",
        }
    }
}

/// What is wrong where a sequence stops matching the grammar, in the user's
/// terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxProblem {
    /// A `<` opens a key block that is not closed with `>`.
    UnclosedKeyBlock,
    /// A key name is expected inside a key block; `at_end` when the text ends
    /// there, so the block is not closed either.
    KeyNameExpected { at_end: bool },
    /// A `\` ends the text or comes before a line break, so it escapes nothing.
    NothingToEscape,
    /// None of the above.
    Unreadable,
}

impl SyntaxProblem {
    fn message(self, position: usize) -> String {
        let (explanation, hint) = match self {
            SyntaxProblem::UnclosedKeyBlock => (
                format!("the < at position {position} opens a key block that is not closed with >"),
                Some(SyntaxHint::LiteralLessThan),
            ),
            SyntaxProblem::KeyNameExpected { at_end: false } => {
                (format!("a key name is expected at position {position}"), Some(SyntaxHint::LiteralLessThan))
            }
            SyntaxProblem::KeyNameExpected { at_end: true } => (
                format!("a key name is expected at position {position}, and the key block is not closed with >"),
                Some(SyntaxHint::LiteralLessThan),
            ),
            SyntaxProblem::NothingToEscape => (
                format!(
                    "the \\ at position {position} escapes nothing, because it ends the text or comes before a line break"
                ),
                Some(SyntaxHint::LiteralBackslash),
            ),
            SyntaxProblem::Unreadable => (format!("the text cannot be read at position {position}"), None),
        };
        match hint {
            Some(hint) => format!("{explanation}; {}", hint.text()),
            None => explanation,
        }
    }

    fn hint(self) -> Option<SyntaxHint> {
        match self {
            SyntaxProblem::UnclosedKeyBlock | SyntaxProblem::KeyNameExpected { .. } => {
                Some(SyntaxHint::LiteralLessThan)
            }
            SyntaxProblem::NothingToEscape => Some(SyntaxHint::LiteralBackslash),
            SyntaxProblem::Unreadable => None,
        }
    }
}

/// A keyboard sequence that cannot be read. Positions count the characters of
/// the sequence from 1.
///
/// A parse error keeps only the position and the explanation, never the
/// input, so neither its `Display` nor its `Debug` repeats the text. An escape
/// error names the escape itself, which is the reason;
/// [`sensitive`](Self::sensitive) leaves it out.
#[derive(Debug, Error)]
pub enum KeyboardSequenceError {
    /// The sequence does not match the grammar at `position`.
    #[error("{}", .problem.message(*.position))]
    Parse { position: usize, problem: SyntaxProblem },
    /// A `\` at `position` ends the text.
    #[error("the \\ at position {position} ends the text; write \\\\ to type a backslash")]
    DanglingEscape { position: usize },
    /// A `\x` escape at `position` is not followed by 2 hex digits.
    #[error("the escape \\x{literal} at position {position} is invalid: \\x needs exactly 2 hex digits")]
    InvalidHexEscape { literal: String, position: usize },
    /// A `\u` escape at `position` is not followed by 4 hex digits that name a character.
    #[error(
        "the escape \\u{literal} at position {position} is invalid: \\u needs exactly 4 hex digits that name a character"
    )]
    InvalidUnicodeEscape { literal: String, position: usize },
    /// The key name at `position` is empty.
    #[error("the key name at position {position} is empty")]
    EmptyKey { position: usize },
}

impl KeyboardSequenceError {
    /// The position the error refers to, counted in characters from 1.
    #[must_use]
    pub fn position(&self) -> usize {
        match self {
            KeyboardSequenceError::Parse { position, .. }
            | KeyboardSequenceError::DanglingEscape { position }
            | KeyboardSequenceError::InvalidHexEscape { position, .. }
            | KeyboardSequenceError::InvalidUnicodeEscape { position, .. }
            | KeyboardSequenceError::EmptyKey { position } => *position,
        }
    }

    /// How to write the character involved literally, when the message says so.
    #[must_use]
    pub fn syntax_hint(&self) -> Option<SyntaxHint> {
        match self {
            KeyboardSequenceError::Parse { problem, .. } => problem.hint(),
            KeyboardSequenceError::DanglingEscape { .. } => Some(SyntaxHint::LiteralBackslash),
            KeyboardSequenceError::InvalidHexEscape { .. }
            | KeyboardSequenceError::InvalidUnicodeEscape { .. }
            | KeyboardSequenceError::EmptyKey { .. } => None,
        }
    }

    /// The message without any part of the sequence: the position and the
    /// kind of failure, with the syntax hint where one applies.
    #[must_use]
    pub fn sensitive(&self) -> String {
        match self {
            KeyboardSequenceError::InvalidHexEscape { position, .. } => {
                format!("the \\x escape at position {position} is invalid: \\x needs exactly 2 hex digits")
            }
            KeyboardSequenceError::InvalidUnicodeEscape { position, .. } => format!(
                "the \\u escape at position {position} is invalid: \\u needs exactly 4 hex digits that name a character"
            ),
            KeyboardSequenceError::Parse { .. }
            | KeyboardSequenceError::DanglingEscape { .. }
            | KeyboardSequenceError::EmptyKey { .. } => self.to_string(),
        }
    }
}

/// One part of a parsed sequence. Every character and key name keeps its
/// position in the sequence, counted in characters from 1; an escape maps to
/// the position of its backslash, a key name to the position of its first
/// character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SequenceSegment {
    /// Characters to type, each with its position.
    Text(Vec<(char, usize)>),
    /// Key combinations, each key name with its position.
    Shortcut(Vec<Vec<(String, usize)>>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardSequence {
    segments: Vec<SequenceSegment>,
}

impl KeyboardSequence {
    /// Parses a keyboard sequence such as `Hello<Ctrl+A>`.
    ///
    /// # Errors
    ///
    /// Returns [`KeyboardSequenceError::Parse`] with the position and the
    /// explanation if `input` does not match the sequence grammar,
    /// [`KeyboardSequenceError::DanglingEscape`],
    /// [`KeyboardSequenceError::InvalidHexEscape`] or
    /// [`KeyboardSequenceError::InvalidUnicodeEscape`] for a malformed escape,
    /// and [`KeyboardSequenceError::EmptyKey`] for a shortcut with an empty key
    /// name.
    ///
    /// # Panics
    ///
    /// Does not panic in practice: the `expect` covers the grammar invariant that a successful
    /// parse of the `sequence` rule always yields exactly one root pair.
    pub fn parse(input: &str) -> Result<Self, KeyboardSequenceError> {
        let mut pairs =
            KeyboardSequenceParser::parse(Rule::sequence, input).map_err(|err| syntax_error(input, &err))?;
        let sequence = pairs.next().expect("sequence root");
        let mut segments = Vec::new();
        for pair in sequence.into_inner() {
            match pair.as_rule() {
                Rule::segment | Rule::shortcut | Rule::text => {
                    segments.push(parse_segment(pair, input)?);
                }
                // `EOI` (and any other rule) carries no segment.
                _ => {}
            }
        }
        Ok(Self { segments })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    #[must_use]
    pub fn segments(&self) -> &[SequenceSegment] {
        &self.segments
    }

    /// Maps every character and key name of the sequence to a device key code.
    ///
    /// # Errors
    ///
    /// Returns [`KeyboardActionError::Key`] for the first character or key
    /// name the device cannot map, with its position and the device's
    /// [`KeyboardError`](platynui_core::platform::KeyboardError).
    pub fn resolve(&self, device: &dyn KeyboardDevice) -> Result<ResolvedKeyboardSequence, KeyboardActionError> {
        let mut resolved = Vec::with_capacity(self.segments.len());
        for segment in &self.segments {
            match segment {
                SequenceSegment::Text(chars) => {
                    let mut codes = Vec::with_capacity(chars.len());
                    for &(ch, position) in chars {
                        let mut buffer = [0; 4];
                        let code = device.key_to_code(ch.encode_utf8(&mut buffer)).map_err(|source| {
                            KeyboardActionError::Key { position, key: ch.to_string(), shortcut: false, source }
                        })?;
                        codes.push(code);
                    }
                    resolved.push(ResolvedSegment::Text(codes));
                }
                SequenceSegment::Shortcut(groups) => {
                    let mut resolved_groups = Vec::with_capacity(groups.len());
                    for group in groups {
                        let mut codes = Vec::with_capacity(group.len());
                        for (key, position) in group {
                            let code = device.key_to_code(key).map_err(|source| KeyboardActionError::Key {
                                position: *position,
                                key: key.clone(),
                                shortcut: true,
                                source,
                            })?;
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
            ResolvedSegment::Shortcut(_) => None,
        }
    }

    pub fn shortcut_combinations(&self) -> Option<&[Vec<KeyCode>]> {
        match self {
            ResolvedSegment::Shortcut(groups) => Some(groups.as_slice()),
            ResolvedSegment::Text(_) => None,
        }
    }
}

/// The position of the character at `byte_offset` in `input`, counted from 1.
fn position_of(input: &str, byte_offset: usize) -> usize {
    input.get(..byte_offset).unwrap_or(input).chars().count() + 1
}

/// Explains where and why `input` does not match the grammar, from pest's
/// expected rules and the character at the failing position, never from rule
/// names.
fn syntax_error(input: &str, err: &pest::error::Error<Rule>) -> KeyboardSequenceError {
    let offset = match err.location {
        InputLocation::Pos(offset) | InputLocation::Span((offset, _)) => offset,
    };
    let next = input.get(offset..).and_then(|rest| rest.chars().next());
    let expects_key =
        matches!(&err.variant, ErrorVariant::ParsingError { positives, .. } if positives.contains(&Rule::key));
    let problem = if expects_key {
        SyntaxProblem::KeyNameExpected { at_end: next.is_none() }
    } else {
        match next {
            Some('<') => SyntaxProblem::UnclosedKeyBlock,
            Some('\\') => SyntaxProblem::NothingToEscape,
            _ => SyntaxProblem::Unreadable,
        }
    };
    KeyboardSequenceError::Parse { position: position_of(input, offset), problem }
}

fn parse_segment(pair: Pair<Rule>, input: &str) -> Result<SequenceSegment, KeyboardSequenceError> {
    match pair.as_rule() {
        Rule::segment => {
            let inner = pair.into_inner().next().expect("segment inner value");
            parse_segment(inner, input)
        }
        Rule::text => {
            let position = position_of(input, pair.as_span().start());
            Ok(SequenceSegment::Text(decode_escapes(pair.as_str(), position)?))
        }
        Rule::shortcut => parse_shortcut(pair, input),
        _ => unreachable!("unexpected rule in parse_segment"),
    }
}

fn parse_shortcut(pair: Pair<Rule>, input: &str) -> Result<SequenceSegment, KeyboardSequenceError> {
    let block_position = position_of(input, pair.as_span().start());
    let mut groups = Vec::new();
    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::combination {
            let combination_position = position_of(input, inner.as_span().start());
            let mut keys = Vec::new();
            for key_pair in inner.into_inner() {
                if key_pair.as_rule() == Rule::key {
                    let position = position_of(input, key_pair.as_span().start());
                    let name: String =
                        decode_escapes(key_pair.as_str(), position)?.into_iter().map(|(ch, _)| ch).collect();
                    if name.is_empty() {
                        return Err(KeyboardSequenceError::EmptyKey { position });
                    }
                    keys.push((name, position));
                }
            }
            if keys.is_empty() {
                return Err(KeyboardSequenceError::EmptyKey { position: combination_position });
            }
            groups.push(keys);
        }
    }
    if groups.is_empty() {
        Err(KeyboardSequenceError::EmptyKey { position: block_position })
    } else {
        Ok(SequenceSegment::Shortcut(groups))
    }
}

/// Decodes the escapes of `source`, which starts at `position`, into its
/// characters with their positions; an escape maps to its backslash.
fn decode_escapes(source: &str, position: usize) -> Result<Vec<(char, usize)>, KeyboardSequenceError> {
    let mut result = Vec::with_capacity(source.len());
    let mut chars = source.chars();
    let mut next_position = position;
    while let Some(ch) = chars.next() {
        let at = next_position;
        next_position += 1;
        if ch != '\\' {
            result.push((ch, at));
            continue;
        }
        let escaped = chars.next().ok_or(KeyboardSequenceError::DanglingEscape { position: at })?;
        next_position += 1;
        match escaped {
            'x' => {
                let literal: String = chars.by_ref().take(2).collect();
                next_position += literal.chars().count();
                let value = hex_digits(&literal, 2).and_then(|digits| u8::from_str_radix(digits, 16).ok()).ok_or_else(
                    || KeyboardSequenceError::InvalidHexEscape { literal: literal.clone(), position: at },
                )?;
                result.push((char::from(value), at));
            }
            'u' => {
                let literal: String = chars.by_ref().take(4).collect();
                next_position += literal.chars().count();
                let value = hex_digits(&literal, 4)
                    .and_then(|digits| u32::from_str_radix(digits, 16).ok())
                    .and_then(char::from_u32)
                    .ok_or_else(|| KeyboardSequenceError::InvalidUnicodeEscape {
                        literal: literal.clone(),
                        position: at,
                    })?;
                result.push((value, at));
            }
            // `\<`, `\>` and `\\` type the character itself, and so does a
            // backslash before any other character.
            other => result.push((other, at)),
        }
    }
    Ok(result)
}

/// `literal` if it is exactly `count` hex digits (no sign, unlike `from_str_radix`).
fn hex_digits(literal: &str, count: usize) -> Option<&str> {
    (literal.len() == count && literal.chars().all(|ch| ch.is_ascii_hexdigit())).then_some(literal)
}

#[cfg(test)]
mod tests {
    use super::{
        KeyboardSequence, KeyboardSequenceError, KeyboardSequenceParser, ResolvedSegment, Rule, SequenceSegment,
        SyntaxProblem,
    };
    use pest::Parser;
    use platynui_core::platform::{KeyCode, KeyboardDevice, KeyboardError, KeyboardEvent};

    /// The characters of a text segment, without their positions.
    fn text(segment: &SequenceSegment) -> String {
        match segment {
            SequenceSegment::Text(chars) => chars.iter().map(|(ch, _)| ch).collect(),
            SequenceSegment::Shortcut(_) => panic!("expected text, got {segment:?}"),
        }
    }

    /// The key names of a shortcut segment's combinations, without their positions.
    fn keys(segment: &SequenceSegment) -> Vec<Vec<&str>> {
        match segment {
            SequenceSegment::Shortcut(groups) => {
                groups.iter().map(|group| group.iter().map(|(name, _)| name.as_str()).collect()).collect()
            }
            SequenceSegment::Text(_) => panic!("expected shortcut, got {segment:?}"),
        }
    }

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
        assert_eq!(text(&sequence.segments()[0]), "foo<bar ☺ baz");
    }

    #[test]
    fn parse_shortcut_combo() {
        let input = "<Ctrl+Alt+T Ctrl+C>";
        let sequence = KeyboardSequence::parse(input).unwrap();
        assert_eq!(sequence.segments().len(), 1);
        assert_eq!(keys(&sequence.segments()[0]), [vec!["Ctrl", "Alt", "T"], vec!["Ctrl", "C"]]);
    }

    #[test]
    fn parse_shortcut_with_escape_sequences() {
        let input = "<Ctrl+\\<+\\x41+\\u0042>";
        let sequence = KeyboardSequence::parse(input).unwrap();
        assert_eq!(keys(&sequence.segments()[0]), [vec!["Ctrl", "<", "A", "B"]]);
    }

    #[test]
    fn parse_shortcut_with_single_char_key() {
        let input = "<Ctrl+#>";
        let sequence = KeyboardSequence::parse(input).expect("parse <Ctrl+#>");
        assert_eq!(keys(&sequence.segments()[0]), [vec!["Ctrl", "#"]]);
    }

    #[test]
    fn parse_shortcut_with_shift_and_dot_char() {
        let input = "<Ctrl+Shift+.>";
        let sequence = KeyboardSequence::parse(input).expect("parse <Ctrl+Shift+.>");
        assert_eq!(keys(&sequence.segments()[0]), [vec!["Ctrl", "Shift", "."]]);
    }

    #[test]
    fn reject_unfinished_block() {
        let input = "<Ctrl";
        assert!(matches!(
            KeyboardSequence::parse(input),
            Err(KeyboardSequenceError::Parse { position: 1, problem: SyntaxProblem::UnclosedKeyBlock })
        ));
    }

    #[test]
    fn characters_and_keys_keep_their_positions() {
        let sequence = KeyboardSequence::parse("aä\\u263A<Ctrl+\\x41 B>z").unwrap();
        assert_eq!(
            sequence.segments(),
            [
                SequenceSegment::Text(vec![('a', 1), ('ä', 2), ('☺', 3)]),
                SequenceSegment::Shortcut(vec![
                    vec![(String::from("Ctrl"), 10), (String::from("A"), 15)],
                    vec![(String::from("B"), 20)]
                ]),
                SequenceSegment::Text(vec![('z', 22)]),
            ]
        );
    }

    #[test]
    fn a_hex_escape_needs_two_hex_digits_without_a_sign() {
        for input in ["\\x+1", "\\x4", "\\xZZ"] {
            assert!(
                matches!(
                    KeyboardSequence::parse(input),
                    Err(KeyboardSequenceError::InvalidHexEscape { position: 1, .. })
                ),
                "{input}"
            );
        }
        assert!(matches!(
            KeyboardSequence::parse("\\uD800"),
            Err(KeyboardSequenceError::InvalidUnicodeEscape { position: 1, .. })
        ));
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
            ResolvedSegment::Shortcut(_) => panic!("expected text segment"),
        }
        match &resolved.segments()[1] {
            ResolvedSegment::Shortcut(groups) => {
                assert_eq!(groups.len(), 1);
                assert_eq!(groups[0].len(), 2);
            }
            ResolvedSegment::Text(_) => panic!("expected shortcut"),
        }
    }

    #[test]
    fn decode_hex_escape() {
        let sequence = KeyboardSequence::parse("\\x41").unwrap();
        assert_eq!(text(&sequence.segments()[0]), "A");
    }

    #[test]
    fn decode_unicode_escape() {
        let sequence = KeyboardSequence::parse("\\u00E4").unwrap();
        assert_eq!(text(&sequence.segments()[0]), "ä");
    }
}
