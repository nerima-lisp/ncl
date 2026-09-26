//! Typed parsing primitives for the FORMAT directive set.
#![allow(missing_docs)]

mod executor;

pub use executor::{FormatError, execute};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormatControl {
    pub parts: Vec<ControlPart>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ControlPart {
    Literal(String),
    Directive(Directive),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Directive {
    pub parameters: Vec<Parameter>,
    pub colon: bool,
    pub at_sign: bool,
    pub kind: DirectiveKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Parameter {
    Integer(i64),
    Character(char),
    Relative,
    Unsupplied,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectiveKind {
    A,
    S,
    D,
    B,
    O,
    X,
    R,
    P,
    C,
    I,
    F,
    E,
    G,
    T,
    W,
    Dollar,
    Percent,
    Ampersand,
    Bar,
    Tilde,
    TildeOpen,
    TildeClose,
    Star,
    Question,
    ParenOpen,
    ParenClose,
    BracketOpen,
    BracketClose,
    BraceOpen,
    BraceClose,
    UpArrow,
    Semicolon,
    Slash,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseError {
    pub offset: usize,
    pub kind: ParseErrorKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseErrorKind {
    UnterminatedDirective,
    InvalidParameter,
    MissingDirective,
    UnknownDirective,
}

/// Parse a FORMAT control string into typed parts.
///
/// # Errors
///
/// Returns the byte-independent character offset and reason when the control
/// string contains an incomplete, malformed, or unknown directive.
#[allow(clippy::too_many_lines)]
pub fn parse(control: &str) -> Result<FormatControl, ParseError> {
    let chars: Vec<char> = control.chars().collect();
    let mut parts = Vec::new();
    let mut literal = String::new();
    let mut index = 0;
    while index < chars.len() {
        let current = chars.get(index).copied().ok_or(ParseError {
            offset: index,
            kind: ParseErrorKind::MissingDirective,
        })?;
        if current != '~' {
            literal.push(current);
            index += 1;
            continue;
        }
        if !literal.is_empty() {
            parts.push(ControlPart::Literal(std::mem::take(&mut literal)));
        }
        let start = index;
        index += 1;
        let mut parameters = Vec::new();
        let mut colon = false;
        let mut at_sign = false;
        loop {
            let current = chars.get(index).copied().ok_or(ParseError {
                offset: start,
                kind: ParseErrorKind::UnterminatedDirective,
            })?;
            match current {
                ':' => colon = true,
                '@' => at_sign = true,
                ',' => parameters.push(Parameter::Unsupplied),
                '\'' => {
                    index += 1;
                    let value = chars.get(index).copied().ok_or(ParseError {
                        offset: index,
                        kind: ParseErrorKind::InvalidParameter,
                    })?;
                    parameters.push(Parameter::Character(value));
                }
                '+' | '-' | '0'..='9' => {
                    let sign = if current == '-' { -1 } else { 1 };
                    if current == '+' || current == '-' {
                        index += 1;
                    }
                    let digit_start = index;
                    while chars
                        .get(index)
                        .copied()
                        .is_some_and(|digit| digit.is_ascii_digit())
                    {
                        index = index.checked_add(1).ok_or(ParseError {
                            offset: digit_start,
                            kind: ParseErrorKind::InvalidParameter,
                        })?;
                    }
                    if digit_start == index {
                        return Err(ParseError {
                            offset: index,
                            kind: ParseErrorKind::InvalidParameter,
                        });
                    }
                    let mut value: i64 = 0;
                    for digit in chars.iter().skip(digit_start).take(index - digit_start) {
                        let digit = digit.to_digit(10).ok_or(ParseError {
                            offset: digit_start,
                            kind: ParseErrorKind::InvalidParameter,
                        })?;
                        value = value
                            .checked_mul(10)
                            .and_then(|n| n.checked_add(i64::from(digit)))
                            .ok_or(ParseError {
                                offset: digit_start,
                                kind: ParseErrorKind::InvalidParameter,
                            })?;
                    }
                    parameters.push(Parameter::Integer(value.checked_mul(sign).ok_or(
                        ParseError {
                            offset: digit_start,
                            kind: ParseErrorKind::InvalidParameter,
                        },
                    )?));
                    continue;
                }
                'v' | 'V' => parameters.push(Parameter::Relative),
                'a' | 'A' | 's' | 'S' | 'd' | 'D' | 'b' | 'B' | 'o' | 'O' | 'x' | 'X' | 'r'
                | 'R' | 'p' | 'P' | 'c' | 'C' | 'f' | 'F' | 'e' | 'E' | 'g' | 'G' | '$' | '%'
                | '&' | '|' | '~' | '<' | '>' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}'
                | '^' | ';' | '/' | 'i' | 'I' | 't' | 'T' | 'w' | 'W' => {
                    let kind = directive_kind(current).ok_or(ParseError {
                        offset: index,
                        kind: ParseErrorKind::UnknownDirective,
                    })?;
                    parts.push(ControlPart::Directive(Directive {
                        parameters,
                        colon,
                        at_sign,
                        kind,
                    }));
                    index += 1;
                    break;
                }
                _ => {
                    return Err(ParseError {
                        offset: index,
                        kind: ParseErrorKind::UnknownDirective,
                    });
                }
            }
            index += 1;
        }
    }
    if !literal.is_empty() {
        parts.push(ControlPart::Literal(literal));
    }
    Ok(FormatControl { parts })
}

const fn directive_kind(value: char) -> Option<DirectiveKind> {
    match value.to_ascii_uppercase() {
        'A' => Some(DirectiveKind::A),
        'S' => Some(DirectiveKind::S),
        'D' => Some(DirectiveKind::D),
        'B' => Some(DirectiveKind::B),
        'O' => Some(DirectiveKind::O),
        'X' => Some(DirectiveKind::X),
        'R' => Some(DirectiveKind::R),
        'P' => Some(DirectiveKind::P),
        'C' => Some(DirectiveKind::C),
        'I' => Some(DirectiveKind::I),
        'F' => Some(DirectiveKind::F),
        'E' => Some(DirectiveKind::E),
        'G' => Some(DirectiveKind::G),
        'T' => Some(DirectiveKind::T),
        'W' => Some(DirectiveKind::W),
        '$' => Some(DirectiveKind::Dollar),
        '%' => Some(DirectiveKind::Percent),
        '&' => Some(DirectiveKind::Ampersand),
        '|' => Some(DirectiveKind::Bar),
        '~' => Some(DirectiveKind::Tilde),
        '<' => Some(DirectiveKind::TildeOpen),
        '>' => Some(DirectiveKind::TildeClose),
        '*' => Some(DirectiveKind::Star),
        '?' => Some(DirectiveKind::Question),
        '(' => Some(DirectiveKind::ParenOpen),
        ')' => Some(DirectiveKind::ParenClose),
        '[' => Some(DirectiveKind::BracketOpen),
        ']' => Some(DirectiveKind::BracketClose),
        '{' => Some(DirectiveKind::BraceOpen),
        '}' => Some(DirectiveKind::BraceClose),
        '^' => Some(DirectiveKind::UpArrow),
        ';' => Some(DirectiveKind::Semicolon),
        '/' => Some(DirectiveKind::Slash),
        _ => None,
    }
}
