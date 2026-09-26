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
    Percent,
    Ampersand,
    Tilde,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsupportedDirectiveKind {
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
    Bar,
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
    UnsupportedDirective { directive: UnsupportedDirectiveKind },
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
                other => {
                    let kind = directive_kind(other).map_err(|kind| ParseError {
                        offset: index,
                        kind,
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
            }
            index += 1;
        }
    }
    if !literal.is_empty() {
        parts.push(ControlPart::Literal(literal));
    }
    Ok(FormatControl { parts })
}

const fn directive_kind(value: char) -> Result<DirectiveKind, ParseErrorKind> {
    match value.to_ascii_uppercase() {
        'A' => Ok(DirectiveKind::A),
        'S' => Ok(DirectiveKind::S),
        'D' => Ok(DirectiveKind::D),
        'B' => Ok(DirectiveKind::B),
        'O' => Ok(DirectiveKind::O),
        'X' => Ok(DirectiveKind::X),
        '%' => Ok(DirectiveKind::Percent),
        '&' => Ok(DirectiveKind::Ampersand),
        '~' => Ok(DirectiveKind::Tilde),
        'R' => Err(ParseErrorKind::UnsupportedDirective {
            directive: UnsupportedDirectiveKind::R,
        }),
        'P' => Err(ParseErrorKind::UnsupportedDirective {
            directive: UnsupportedDirectiveKind::P,
        }),
        'C' => Err(ParseErrorKind::UnsupportedDirective {
            directive: UnsupportedDirectiveKind::C,
        }),
        'I' => Err(ParseErrorKind::UnsupportedDirective {
            directive: UnsupportedDirectiveKind::I,
        }),
        'F' => Err(ParseErrorKind::UnsupportedDirective {
            directive: UnsupportedDirectiveKind::F,
        }),
        'E' => Err(ParseErrorKind::UnsupportedDirective {
            directive: UnsupportedDirectiveKind::E,
        }),
        'G' => Err(ParseErrorKind::UnsupportedDirective {
            directive: UnsupportedDirectiveKind::G,
        }),
        'T' => Err(ParseErrorKind::UnsupportedDirective {
            directive: UnsupportedDirectiveKind::T,
        }),
        'W' => Err(ParseErrorKind::UnsupportedDirective {
            directive: UnsupportedDirectiveKind::W,
        }),
        '$' => Err(ParseErrorKind::UnsupportedDirective {
            directive: UnsupportedDirectiveKind::Dollar,
        }),
        '|' => Err(ParseErrorKind::UnsupportedDirective {
            directive: UnsupportedDirectiveKind::Bar,
        }),
        '<' => Err(ParseErrorKind::UnsupportedDirective {
            directive: UnsupportedDirectiveKind::TildeOpen,
        }),
        '>' => Err(ParseErrorKind::UnsupportedDirective {
            directive: UnsupportedDirectiveKind::TildeClose,
        }),
        '*' => Err(ParseErrorKind::UnsupportedDirective {
            directive: UnsupportedDirectiveKind::Star,
        }),
        '?' => Err(ParseErrorKind::UnsupportedDirective {
            directive: UnsupportedDirectiveKind::Question,
        }),
        '(' => Err(ParseErrorKind::UnsupportedDirective {
            directive: UnsupportedDirectiveKind::ParenOpen,
        }),
        ')' => Err(ParseErrorKind::UnsupportedDirective {
            directive: UnsupportedDirectiveKind::ParenClose,
        }),
        '[' => Err(ParseErrorKind::UnsupportedDirective {
            directive: UnsupportedDirectiveKind::BracketOpen,
        }),
        ']' => Err(ParseErrorKind::UnsupportedDirective {
            directive: UnsupportedDirectiveKind::BracketClose,
        }),
        '{' => Err(ParseErrorKind::UnsupportedDirective {
            directive: UnsupportedDirectiveKind::BraceOpen,
        }),
        '}' => Err(ParseErrorKind::UnsupportedDirective {
            directive: UnsupportedDirectiveKind::BraceClose,
        }),
        '^' => Err(ParseErrorKind::UnsupportedDirective {
            directive: UnsupportedDirectiveKind::UpArrow,
        }),
        ';' => Err(ParseErrorKind::UnsupportedDirective {
            directive: UnsupportedDirectiveKind::Semicolon,
        }),
        '/' => Err(ParseErrorKind::UnsupportedDirective {
            directive: UnsupportedDirectiveKind::Slash,
        }),
        _other => Err(ParseErrorKind::UnknownDirective),
    }
}
