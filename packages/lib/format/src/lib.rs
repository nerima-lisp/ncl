//! Typed parsing primitives for the FORMAT directive set.
#![allow(missing_docs)]

mod executor;
mod registration;

pub use executor::{FormatError, FormatFunctionCaller, execute, execute_with_caller};
pub use registration::register;

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
    pub name: Option<String>,
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
    ArgumentCount,
    Unsupplied,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectiveKind {
    A,
    S,
    C,
    R,
    D,
    B,
    O,
    X,
    F,
    E,
    G,
    Dollar,
    W,
    Underscore,
    Less,
    Greater,
    ColonGreater,
    I,
    Slash,
    T,
    Star,
    BracketOpen,
    BracketClose,
    BraceOpen,
    BraceClose,
    Question,
    ParenOpen,
    ParenClose,
    P,
    Bar,
    Semicolon,
    UpArrow,
    Newline,
    Percent,
    Ampersand,
    Tilde,
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
        let mut comma_pending = false;
        loop {
            let current = chars.get(index).copied().ok_or(ParseError {
                offset: start,
                kind: ParseErrorKind::UnterminatedDirective,
            })?;
            match current {
                ':' => colon = true,
                '@' => at_sign = true,
                ',' => {
                    if comma_pending {
                        parameters.push(Parameter::Unsupplied);
                    }
                    comma_pending = true;
                }
                '\'' => {
                    index += 1;
                    let value = chars.get(index).copied().ok_or(ParseError {
                        offset: index,
                        kind: ParseErrorKind::InvalidParameter,
                    })?;
                    parameters.push(Parameter::Character(value));
                    comma_pending = false;
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
                    parameters.push(Parameter::Integer(value * sign));
                    comma_pending = false;
                    continue;
                }
                'v' | 'V' => {
                    parameters.push(Parameter::Relative);
                    comma_pending = false;
                }
                '#' => {
                    parameters.push(Parameter::ArgumentCount);
                    comma_pending = false;
                }
                other => {
                    if comma_pending {
                        return Err(ParseError {
                            offset: index,
                            kind: ParseErrorKind::InvalidParameter,
                        });
                    }
                    let kind = directive_kind(other).map_err(|kind| ParseError {
                        offset: index,
                        kind,
                    })?;
                    validate_parameters(kind, &parameters).map_err(|kind| ParseError {
                        offset: start,
                        kind,
                    })?;
                    let name = if kind == DirectiveKind::Slash {
                        parse_slash_name(&chars, &mut index)
                    } else {
                        index += 1;
                        None
                    };
                    parts.push(ControlPart::Directive(Directive {
                        name,
                        parameters,
                        colon,
                        at_sign,
                        kind,
                    }));
                    if other == '\n' {
                        while matches!(chars.get(index), Some(' ' | '\t')) {
                            index += 1;
                        }
                    }
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

fn validate_parameters(
    directive: DirectiveKind,
    parameters: &[Parameter],
) -> Result<(), ParseErrorKind> {
    let nonnegative = |parameter: Option<&Parameter>| {
        if let Some(Parameter::Integer(value)) = parameter
            && *value < 0
        {
            Err(ParseErrorKind::InvalidParameter)
        } else {
            Ok(())
        }
    };
    match directive {
        DirectiveKind::R => {
            if let Some(Parameter::Integer(value)) = parameters.first()
                && !(2..=36).contains(value)
            {
                return Err(ParseErrorKind::InvalidParameter);
            }
        }
        DirectiveKind::T => {
            for parameter in parameters.iter().take(2) {
                if let Parameter::Integer(value) = parameter
                    && *value < 0
                {
                    return Err(ParseErrorKind::InvalidParameter);
                }
            }
        }
        DirectiveKind::A
        | DirectiveKind::S
        | DirectiveKind::B
        | DirectiveKind::O
        | DirectiveKind::X
        | DirectiveKind::F
        | DirectiveKind::E
        | DirectiveKind::G
        | DirectiveKind::Dollar
        | DirectiveKind::W => {
            nonnegative(parameters.first())?;
            if matches!(
                directive,
                DirectiveKind::F | DirectiveKind::E | DirectiveKind::G
            ) {
                nonnegative(parameters.get(1))?;
            }
        }
        DirectiveKind::D
        | DirectiveKind::C
        | DirectiveKind::Slash
        | DirectiveKind::Star
        | DirectiveKind::BracketOpen
        | DirectiveKind::BracketClose
        | DirectiveKind::BraceOpen
        | DirectiveKind::BraceClose
        | DirectiveKind::Question
        | DirectiveKind::ParenOpen
        | DirectiveKind::ParenClose
        | DirectiveKind::P
        | DirectiveKind::Semicolon
        | DirectiveKind::UpArrow
        | DirectiveKind::Newline
        | DirectiveKind::Less
        | DirectiveKind::Greater
        | DirectiveKind::ColonGreater => {}
        DirectiveKind::Percent
        | DirectiveKind::Ampersand
        | DirectiveKind::Tilde
        | DirectiveKind::Bar
        | DirectiveKind::Underscore
        | DirectiveKind::I => nonnegative(parameters.first())?,
    }
    Ok(())
}

fn parse_slash_name(chars: &[char], index: &mut usize) -> Option<String> {
    let name_start = index.saturating_add(1);
    let Some(first) = chars.get(name_start).copied() else {
        *index = index.saturating_add(1);
        return None;
    };
    if first == '~' || first == '\n' {
        *index = index.saturating_add(1);
        return None;
    }
    let Some(length) = chars
        .iter()
        .skip(name_start)
        .position(|character| *character == '/')
    else {
        *index = index.saturating_add(1);
        return None;
    };
    let end = name_start + length;
    let name = chars[name_start..end].iter().collect();
    *index = end + 1;
    Some(name)
}

const fn directive_kind(value: char) -> Result<DirectiveKind, ParseErrorKind> {
    match value.to_ascii_uppercase() {
        '\n' => Ok(DirectiveKind::Newline),
        'A' => Ok(DirectiveKind::A),
        'S' => Ok(DirectiveKind::S),
        'C' => Ok(DirectiveKind::C),
        'R' => Ok(DirectiveKind::R),
        'D' => Ok(DirectiveKind::D),
        'B' => Ok(DirectiveKind::B),
        'O' => Ok(DirectiveKind::O),
        'X' => Ok(DirectiveKind::X),
        'F' => Ok(DirectiveKind::F),
        'E' => Ok(DirectiveKind::E),
        'G' => Ok(DirectiveKind::G),
        '$' => Ok(DirectiveKind::Dollar),
        'W' => Ok(DirectiveKind::W),
        '_' => Ok(DirectiveKind::Underscore),
        '<' => Ok(DirectiveKind::Less),
        '>' => Ok(DirectiveKind::Greater),
        'I' => Ok(DirectiveKind::I),
        '/' => Ok(DirectiveKind::Slash),
        'T' => Ok(DirectiveKind::T),
        '*' => Ok(DirectiveKind::Star),
        '[' => Ok(DirectiveKind::BracketOpen),
        ']' => Ok(DirectiveKind::BracketClose),
        '{' => Ok(DirectiveKind::BraceOpen),
        '}' => Ok(DirectiveKind::BraceClose),
        '?' => Ok(DirectiveKind::Question),
        '(' => Ok(DirectiveKind::ParenOpen),
        ')' => Ok(DirectiveKind::ParenClose),
        'P' => Ok(DirectiveKind::P),
        '|' => Ok(DirectiveKind::Bar),
        ';' => Ok(DirectiveKind::Semicolon),
        '^' => Ok(DirectiveKind::UpArrow),
        '%' => Ok(DirectiveKind::Percent),
        '&' => Ok(DirectiveKind::Ampersand),
        '~' => Ok(DirectiveKind::Tilde),
        _other => Err(ParseErrorKind::UnknownDirective),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers_parser_parameter_validation_edges() {
        assert!(parse("~,A").is_err());
        assert!(parse("~37R").is_err());
        assert!(parse("~0T").is_ok());
        assert!(parse("~-1A").is_err());
        assert!(parse("~-1%").is_err());
        assert!(parse("~9223372036854775808A").is_err());
        assert!(parse("~\n  tail").is_ok());
        assert_eq!(directive_kind('\n'), Ok(DirectiveKind::Newline));
        assert_eq!(directive_kind('?'), Ok(DirectiveKind::Question));
        assert_eq!(directive_kind('!'), Err(ParseErrorKind::UnknownDirective));
        assert!(validate_parameters(DirectiveKind::R, &[Parameter::Integer(1)]).is_err());
        assert!(validate_parameters(DirectiveKind::T, &[Parameter::Integer(0)]).is_ok());
        assert!(validate_parameters(DirectiveKind::A, &[Parameter::Integer(-1)]).is_err());
        assert!(validate_parameters(DirectiveKind::C, &[]).is_ok());
    }
}
