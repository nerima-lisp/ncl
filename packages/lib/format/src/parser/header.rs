use crate::model::{DirectiveModifiers, Parameter};

use super::{ParseError, ParseErrorKind, Parser};

pub(super) struct Header {
    pub(super) modifiers: DirectiveModifiers,
    pub(super) character: char,
}

struct HeaderBuilder {
    modifiers: DirectiveModifiers,
    slot: ParameterSlot,
}

#[derive(Clone, Copy)]
enum ParameterSlot {
    NotStarted,
    Empty,
    Value,
}

impl HeaderBuilder {
    const fn new(position: usize) -> Self {
        Self {
            modifiers: DirectiveModifiers {
                parameters: Vec::new(),
                colon: false,
                at_sign: false,
                position,
            },
            slot: ParameterSlot::NotStarted,
        }
    }

    fn finish(self, character: char) -> Header {
        Header {
            modifiers: self.modifiers,
            character,
        }
    }
}

impl Parser {
    pub(super) fn parse_header(&mut self) -> Result<Header, ParseError> {
        let position = self.index;
        self.index += 1;
        let mut header = HeaderBuilder::new(position);
        while let Some(character) = self.characters.get(self.index).copied() {
            if let Some(character) = self.parse_header_character(character, &mut header)? {
                return Ok(header.finish(character));
            }
        }
        Err(Self::error(
            position,
            None,
            ParseErrorKind::UnterminatedDirective,
        ))
    }

    fn parse_header_character(
        &mut self,
        character: char,
        header: &mut HeaderBuilder,
    ) -> Result<Option<char>, ParseError> {
        match character {
            ':' | '@' => {
                if matches!(header.slot, ParameterSlot::Empty) {
                    header.modifiers.parameters.push(Parameter::Unsupplied);
                    header.slot = ParameterSlot::Value;
                }
                if character == ':' {
                    header.modifiers.colon = true;
                } else {
                    header.modifiers.at_sign = true;
                }
                self.index += 1;
                Ok(None)
            }
            ',' => {
                if header.modifiers.colon || header.modifiers.at_sign {
                    return Err(Self::invalid_parameter(header.modifiers.position));
                }
                match header.slot {
                    ParameterSlot::NotStarted | ParameterSlot::Empty => {
                        header.modifiers.parameters.push(Parameter::Unsupplied);
                    }
                    ParameterSlot::Value => header.slot = ParameterSlot::Empty,
                }
                self.index += 1;
                Ok(None)
            }
            '\'' => {
                if header.modifiers.colon
                    || header.modifiers.at_sign
                    || matches!(header.slot, ParameterSlot::Value)
                {
                    return Err(Self::invalid_parameter(header.modifiers.position));
                }
                self.index += 1;
                let Some(value) = self.characters.get(self.index).copied() else {
                    return Err(Self::invalid_parameter(header.modifiers.position));
                };
                header
                    .modifiers
                    .parameters
                    .push(Parameter::Character(value));
                header.slot = ParameterSlot::Value;
                self.index += 1;
                Ok(None)
            }
            '+' | '-' | '0'..='9' => {
                if header.modifiers.colon
                    || header.modifiers.at_sign
                    || matches!(header.slot, ParameterSlot::Value)
                {
                    return Err(Self::invalid_parameter(header.modifiers.position));
                }
                header
                    .modifiers
                    .parameters
                    .push(self.parse_integer_parameter(header.modifiers.position)?);
                header.slot = ParameterSlot::Value;
                Ok(None)
            }
            'v' | 'V' => {
                if header.modifiers.colon
                    || header.modifiers.at_sign
                    || matches!(header.slot, ParameterSlot::Value)
                {
                    return Err(Self::invalid_parameter(header.modifiers.position));
                }
                header.modifiers.parameters.push(Parameter::Relative);
                header.slot = ParameterSlot::Value;
                self.index += 1;
                Ok(None)
            }
            '#' => {
                if header.modifiers.colon
                    || header.modifiers.at_sign
                    || matches!(header.slot, ParameterSlot::Value)
                {
                    return Err(Self::invalid_parameter(header.modifiers.position));
                }
                header.modifiers.parameters.push(Parameter::Remaining);
                header.slot = ParameterSlot::Value;
                self.index += 1;
                Ok(None)
            }
            other => {
                if other.is_numeric() {
                    return Err(Self::invalid_parameter(header.modifiers.position));
                }
                if matches!(header.slot, ParameterSlot::Empty) {
                    header.modifiers.parameters.push(Parameter::Unsupplied);
                }
                self.index += 1;
                Ok(Some(other))
            }
        }
    }

    const fn invalid_parameter(position: usize) -> ParseError {
        Self::error(position, None, ParseErrorKind::InvalidParameter)
    }
}
