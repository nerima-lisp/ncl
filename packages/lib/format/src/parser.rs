//! Parser for the recursive FORMAT control language.

use crate::model::{
    ControlPart, ControlSegment, Directive, DirectiveBody, DirectiveKind, DirectiveModifiers,
    FormatControl, Parameter, Terminator, TerminatorKind,
};

mod header;

use header::Header;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseError {
    pub offset: usize,
    pub directive: Option<char>,
    pub kind: ParseErrorKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseErrorKind {
    UnterminatedDirective,
    InvalidParameter,
    MissingDirective,
    UnknownDirective,
    UnexpectedTerminator,
    UnterminatedConstruct { expected: char },
    UnterminatedFunctionName,
}

/// Parse a FORMAT control string into a recursive directive sequence.
///
/// # Errors
///
/// Returns a character offset, directive character when available, and a
/// typed reason for malformed or incomplete control syntax.
pub fn parse(control: &str) -> Result<FormatControl, ParseError> {
    let mut parser = Parser {
        characters: control.chars().collect(),
        index: 0,
    };
    let parsed = parser.parse_parts(Boundary {
        terminator: None,
        allow_separator: false,
        opening_position: 0,
    })?;
    Ok(FormatControl {
        parts: parsed.parts,
    })
}

struct Parser {
    characters: Vec<char>,
    index: usize,
}

struct ParsedParts {
    parts: Vec<ControlPart>,
    stop: Stop,
}

enum Stop {
    End,
    Separator(DirectiveModifiers),
    Terminator(Terminator),
}

#[derive(Clone, Copy)]
struct Boundary {
    terminator: Option<TerminatorKind>,
    allow_separator: bool,
    opening_position: usize,
}

impl Parser {
    fn parse_parts(&mut self, boundary: Boundary) -> Result<ParsedParts, ParseError> {
        let mut parts = Vec::new();
        let mut literal = String::new();
        while let Some(character) = self.characters.get(self.index).copied() {
            if character != '~' {
                literal.push(character);
                self.index += 1;
                continue;
            }
            Self::push_literal(&mut parts, &mut literal);
            let header = self.parse_header()?;
            if header.character == ';' {
                if boundary.allow_separator {
                    return Ok(ParsedParts {
                        parts,
                        stop: Stop::Separator(header.modifiers),
                    });
                }
                return Err(Self::error(
                    header.modifiers.position,
                    Some(';'),
                    ParseErrorKind::UnexpectedTerminator,
                ));
            }
            if let Some(kind) = TerminatorKind::from_character(header.character) {
                if boundary.terminator == Some(kind) {
                    return Ok(ParsedParts {
                        parts,
                        stop: Stop::Terminator(Terminator {
                            kind,
                            colon: header.modifiers.colon,
                            at_sign: header.modifiers.at_sign,
                            position: header.modifiers.position,
                        }),
                    });
                }
                return Err(Self::error(
                    header.modifiers.position,
                    Some(header.character),
                    ParseErrorKind::UnexpectedTerminator,
                ));
            }
            let skip_whitespace = header.character == '\n' && !header.modifiers.colon;
            let directive = self.parse_directive(header)?;
            parts.push(ControlPart::Directive(directive));
            if skip_whitespace {
                while self
                    .characters
                    .get(self.index)
                    .is_some_and(|character| character.is_whitespace() && *character != '\n')
                {
                    self.index += 1;
                }
            }
        }
        Self::push_literal(&mut parts, &mut literal);
        if let Some(expected) = boundary.terminator {
            return Err(Self::error(
                boundary.opening_position,
                Some(expected.character()),
                ParseErrorKind::UnterminatedConstruct {
                    expected: expected.character(),
                },
            ));
        }
        Ok(ParsedParts {
            parts,
            stop: Stop::End,
        })
    }

    fn parse_integer_parameter(
        &mut self,
        directive_position: usize,
    ) -> Result<Parameter, ParseError> {
        let sign = if self.characters.get(self.index) == Some(&'-') {
            -1_i128
        } else {
            1_i128
        };
        if matches!(self.characters.get(self.index), Some('+' | '-')) {
            self.index += 1;
        }
        let start = self.index;
        let mut value = 0_i128;
        while let Some(character) = self.characters.get(self.index).copied() {
            if !character.is_ascii_digit() {
                break;
            }
            let digit = character.to_digit(10).ok_or_else(|| {
                Self::error(directive_position, None, ParseErrorKind::InvalidParameter)
            })?;
            value = value
                .checked_mul(10)
                .and_then(|current| current.checked_add(i128::from(digit)))
                .ok_or_else(|| {
                    Self::error(directive_position, None, ParseErrorKind::InvalidParameter)
                })?;
            self.index += 1;
        }
        if self.index == start {
            return Err(Self::error(
                directive_position,
                None,
                ParseErrorKind::InvalidParameter,
            ));
        }
        let signed = value.checked_mul(sign).ok_or_else(|| {
            Self::error(directive_position, None, ParseErrorKind::InvalidParameter)
        })?;
        let value = i64::try_from(signed)
            .map_err(|_| Self::error(directive_position, None, ParseErrorKind::InvalidParameter))?;
        Ok(Parameter::Integer(value))
    }

    fn parse_directive(&mut self, header: Header) -> Result<Directive, ParseError> {
        let kind = DirectiveKind::from_character(header.character).ok_or_else(|| {
            Self::error(
                header.modifiers.position,
                Some(header.character),
                ParseErrorKind::UnknownDirective,
            )
        })?;
        let position = header.modifiers.position;
        let mut function_name = None;
        let (kind, body, terminator) = match kind {
            DirectiveKind::Slash => {
                function_name = Some(self.parse_function_name(position)?);
                (kind, DirectiveBody::None, None)
            }
            DirectiveKind::Conditional => {
                let (segments, terminator) =
                    self.parse_clauses(position, TerminatorKind::RightBracket)?;
                (kind, DirectiveBody::Clauses(segments), Some(terminator))
            }
            DirectiveKind::Iteration | DirectiveKind::CaseConversion => {
                let close = if kind == DirectiveKind::Iteration {
                    TerminatorKind::RightBrace
                } else {
                    TerminatorKind::RightParen
                };
                let parsed = self.parse_parts(Boundary {
                    terminator: Some(close),
                    allow_separator: false,
                    opening_position: position,
                })?;
                let Stop::Terminator(terminator) = parsed.stop else {
                    return Err(Self::error(
                        position,
                        None,
                        ParseErrorKind::UnterminatedConstruct {
                            expected: close.character(),
                        },
                    ));
                };
                (
                    kind,
                    DirectiveBody::Sequence(Box::new(FormatControl {
                        parts: parsed.parts,
                    })),
                    Some(terminator),
                )
            }
            DirectiveKind::LogicalBlock | DirectiveKind::Justification => {
                let (segments, terminator) =
                    self.parse_clauses(position, TerminatorKind::GreaterThan)?;
                let kind = if terminator.colon {
                    DirectiveKind::LogicalBlock
                } else {
                    DirectiveKind::Justification
                };
                (kind, DirectiveBody::Clauses(segments), Some(terminator))
            }
            DirectiveKind::A
            | DirectiveKind::S
            | DirectiveKind::W
            | DirectiveKind::C
            | DirectiveKind::R
            | DirectiveKind::D
            | DirectiveKind::B
            | DirectiveKind::O
            | DirectiveKind::X
            | DirectiveKind::F
            | DirectiveKind::E
            | DirectiveKind::G
            | DirectiveKind::Dollar
            | DirectiveKind::Percent
            | DirectiveKind::Ampersand
            | DirectiveKind::Bar
            | DirectiveKind::Tilde
            | DirectiveKind::Newline
            | DirectiveKind::Underscore
            | DirectiveKind::Indent
            | DirectiveKind::Tab
            | DirectiveKind::Star
            | DirectiveKind::Recursive
            | DirectiveKind::Plural
            | DirectiveKind::UpArrow => (kind, DirectiveBody::None, None),
        };
        Ok(Directive {
            position,
            parameters: header.modifiers.parameters,
            colon: header.modifiers.colon,
            at_sign: header.modifiers.at_sign,
            kind,
            function_name,
            body,
            terminator,
        })
    }

    fn parse_clauses(
        &mut self,
        opening_position: usize,
        terminator_kind: TerminatorKind,
    ) -> Result<(Vec<ControlSegment>, Terminator), ParseError> {
        let mut segments = Vec::new();
        let mut separator = None;
        loop {
            let parsed = self.parse_parts(Boundary {
                terminator: Some(terminator_kind),
                allow_separator: true,
                opening_position,
            })?;
            segments.push(ControlSegment {
                separator,
                control: FormatControl {
                    parts: parsed.parts,
                },
            });
            match parsed.stop {
                Stop::Separator(next) => separator = Some(next),
                Stop::Terminator(terminator) => return Ok((segments, terminator)),
                Stop::End => {
                    return Err(Self::error(
                        opening_position,
                        None,
                        ParseErrorKind::UnterminatedConstruct {
                            expected: terminator_kind.character(),
                        },
                    ));
                }
            }
        }
    }

    fn parse_function_name(&mut self, directive_position: usize) -> Result<String, ParseError> {
        let mut name = String::new();
        while let Some(character) = self.characters.get(self.index).copied() {
            self.index += 1;
            if character == '/' {
                return Ok(name.to_uppercase());
            }
            name.push(character);
        }
        Err(Self::error(
            directive_position,
            Some('/'),
            ParseErrorKind::UnterminatedFunctionName,
        ))
    }

    fn push_literal(parts: &mut Vec<ControlPart>, literal: &mut String) {
        if !literal.is_empty() {
            parts.push(ControlPart::Literal(std::mem::take(literal)));
        }
    }

    const fn error(offset: usize, directive: Option<char>, kind: ParseErrorKind) -> ParseError {
        ParseError {
            offset,
            directive,
            kind,
        }
    }
}
