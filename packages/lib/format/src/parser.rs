//! Parser for the recursive FORMAT control language.

use crate::model::{
    ControlPart, ControlSegment, Directive, DirectiveBody, DirectiveKind, DirectiveModifiers,
    FormatControl, Parameter, Terminator, TerminatorKind,
};

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

struct Header {
    modifiers: DirectiveModifiers,
    character: char,
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
            self.push_literal(&mut parts, &mut literal);
            let header = self.parse_header()?;
            if header.character == ';' {
                if boundary.allow_separator {
                    return Ok(ParsedParts {
                        parts,
                        stop: Stop::Separator(header.modifiers),
                    });
                }
                return Err(self.error(
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
                return Err(self.error(
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
        self.push_literal(&mut parts, &mut literal);
        if let Some(expected) = boundary.terminator {
            return Err(self.error(
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

    fn parse_header(&mut self) -> Result<Header, ParseError> {
        let position = self.index;
        self.index += 1;
        let mut parameters = Vec::new();
        let mut colon = false;
        let mut at_sign = false;
        let mut slot_started = false;
        let mut slot_has_value = false;
        let mut modifiers_started = false;
        while let Some(character) = self.characters.get(self.index).copied() {
            match character {
                ':' => {
                    if slot_started && !slot_has_value {
                        parameters.push(Parameter::Unsupplied);
                        slot_has_value = true;
                    }
                    colon = true;
                    modifiers_started = true;
                    self.index += 1;
                }
                '@' => {
                    if slot_started && !slot_has_value {
                        parameters.push(Parameter::Unsupplied);
                        slot_has_value = true;
                    }
                    at_sign = true;
                    modifiers_started = true;
                    self.index += 1;
                }
                ',' => {
                    if modifiers_started {
                        return Err(self.error(position, None, ParseErrorKind::InvalidParameter));
                    }
                    if !slot_started {
                        parameters.push(Parameter::Unsupplied);
                        slot_started = true;
                    } else if slot_has_value {
                        slot_has_value = false;
                    } else {
                        parameters.push(Parameter::Unsupplied);
                    }
                    self.index += 1;
                }
                '\'' => {
                    if modifiers_started || slot_has_value {
                        return Err(self.error(position, None, ParseErrorKind::InvalidParameter));
                    }
                    self.index += 1;
                    let Some(value) = self.characters.get(self.index).copied() else {
                        return Err(self.error(position, None, ParseErrorKind::InvalidParameter));
                    };
                    parameters.push(Parameter::Character(value));
                    slot_started = true;
                    slot_has_value = true;
                    self.index += 1;
                }
                '+' | '-' | '0'..='9' => {
                    if modifiers_started || slot_has_value {
                        return Err(self.error(position, None, ParseErrorKind::InvalidParameter));
                    }
                    parameters.push(self.parse_integer_parameter(position)?);
                    slot_started = true;
                    slot_has_value = true;
                }
                'v' | 'V' => {
                    if modifiers_started || slot_has_value {
                        return Err(self.error(position, None, ParseErrorKind::InvalidParameter));
                    }
                    parameters.push(Parameter::Relative);
                    slot_started = true;
                    slot_has_value = true;
                    self.index += 1;
                }
                '#' => {
                    if modifiers_started || slot_has_value {
                        return Err(self.error(position, None, ParseErrorKind::InvalidParameter));
                    }
                    parameters.push(Parameter::Remaining);
                    slot_started = true;
                    slot_has_value = true;
                    self.index += 1;
                }
                other => {
                    if other.is_numeric() {
                        return Err(self.error(position, None, ParseErrorKind::InvalidParameter));
                    }
                    if slot_started && !slot_has_value {
                        parameters.push(Parameter::Unsupplied);
                    }
                    self.index += 1;
                    return Ok(Header {
                        modifiers: DirectiveModifiers {
                            parameters,
                            colon,
                            at_sign,
                            position,
                        },
                        character: other,
                    });
                }
            }
        }
        Err(self.error(position, None, ParseErrorKind::UnterminatedDirective))
    }

    fn parse_integer_parameter(
        &mut self,
        directive_position: usize,
    ) -> Result<Parameter, ParseError> {
        let sign = match self.characters.get(self.index).copied() {
            Some('-') => -1_i128,
            _ => 1_i128,
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
                self.error(directive_position, None, ParseErrorKind::InvalidParameter)
            })?;
            value = value
                .checked_mul(10)
                .and_then(|current| current.checked_add(i128::from(digit)))
                .ok_or_else(|| {
                    self.error(directive_position, None, ParseErrorKind::InvalidParameter)
                })?;
            self.index += 1;
        }
        if self.index == start {
            return Err(self.error(directive_position, None, ParseErrorKind::InvalidParameter));
        }
        let signed = value.checked_mul(sign).ok_or_else(|| {
            self.error(directive_position, None, ParseErrorKind::InvalidParameter)
        })?;
        let value = i64::try_from(signed)
            .map_err(|_| self.error(directive_position, None, ParseErrorKind::InvalidParameter))?;
        Ok(Parameter::Integer(value))
    }

    fn parse_directive(&mut self, header: Header) -> Result<Directive, ParseError> {
        let kind = DirectiveKind::from_character(header.character).ok_or_else(|| {
            self.error(
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
                    return Err(self.error(
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
            _ => (kind, DirectiveBody::None, None),
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
                    return Err(self.error(
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
        Err(self.error(
            directive_position,
            Some('/'),
            ParseErrorKind::UnterminatedFunctionName,
        ))
    }

    fn push_literal(&self, parts: &mut Vec<ControlPart>, literal: &mut String) {
        if !literal.is_empty() {
            parts.push(ControlPart::Literal(std::mem::take(literal)));
        }
    }

    fn error(&self, offset: usize, directive: Option<char>, kind: ParseErrorKind) -> ParseError {
        ParseError {
            offset,
            directive,
            kind,
        }
    }
}
