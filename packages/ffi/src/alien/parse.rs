//! Parsing alien type names and type specifiers.

use super::{AlienEnum, AlienRecord, AlienRoutine, AlienType};
use crate::FfiError;

/// Parse an atomic alien type name such as `int` or `unsigned-long`.
///
/// Hyphens, underscores, and spaces are interchangeable, and the name is
/// case-insensitive.
///
/// # Errors
/// Returns [`FfiError::UnknownAlienType`] when the name is not a known alien
/// type.
pub fn parse_type_name(name: &str) -> Result<AlienType, FfiError> {
    let normalized = normalize(name);
    let ty = match normalized.as_str() {
        "void" => AlienType::Void,
        "boolean" | "bool" => AlienType::Boolean,
        "char" => AlienType::Char,
        "unsigned-char" => AlienType::UnsignedChar,
        "short" | "signed-short" | "short-int" => AlienType::Short,
        "unsigned-short" | "unsigned-short-int" => AlienType::UnsignedShort,
        "int" | "signed" | "signed-int" => AlienType::Int,
        "unsigned" | "unsigned-int" => AlienType::UnsignedInt,
        "long" | "signed-long" | "long-int" => AlienType::Long,
        "unsigned-long" | "unsigned-long-int" => AlienType::UnsignedLong,
        "long-long" | "signed-long-long" | "long-long-int" => AlienType::LongLong,
        "unsigned-long-long" | "unsigned-long-long-int" => AlienType::UnsignedLongLong,
        "size-t" => AlienType::SizeT,
        "ssize-t" => AlienType::SSizeT,
        "float" | "single-float" => AlienType::SingleFloat,
        "double" | "double-float" => AlienType::DoubleFloat,
        "long-float" => AlienType::LongFloat,
        "c-string" | "cstring" => AlienType::CString,
        "utf8-string" => AlienType::Utf8String,
        "system-area-pointer" | "sap" => AlienType::SystemAreaPointer,
        _ => return Err(FfiError::UnknownAlienType(name.trim().to_owned())),
    };
    Ok(ty)
}

/// Parse a full alien type specifier, atomic or parenthesized.
///
/// Supported compound forms are `(pointer T)`, `(array T N)`,
/// `(struct NAME (FIELD T) ...)`, `(union NAME (FIELD T) ...)`,
/// `(enum NAME (VARIANT N) ...)`, and
/// `(function RESULT (ARG ...))`, where an argument may be a bare type or a
/// `(NAME TYPE)` pair.
///
/// # Errors
/// Returns [`FfiError::UnknownAlienType`] when the input is malformed or names
/// an unknown type.
pub fn parse_type_specifier(input: &str) -> Result<AlienType, FfiError> {
    let tokens = tokenize(input);
    let mut parser = Parser {
        items: &tokens,
        position: 0,
    };
    let ty = parser.parse_spec(input)?;
    if parser.position != tokens.len() {
        return Err(bad(input));
    }
    Ok(ty)
}

/// Split `input` into parentheses and whitespace-delimited atoms.
fn tokenize(input: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut start: Option<usize> = None;
    for (index, ch) in input.char_indices() {
        match ch {
            '(' | ')' => {
                if let Some(begin) = start.take()
                    && let Some(token) = input.get(begin..index)
                {
                    tokens.push(token);
                }
                if let Some(token) = input.get(index..index + ch.len_utf8()) {
                    tokens.push(token);
                }
            }
            _ if ch.is_whitespace() => {
                if let Some(begin) = start.take()
                    && let Some(token) = input.get(begin..index)
                {
                    tokens.push(token);
                }
            }
            _ => {
                if start.is_none() {
                    start = Some(index);
                }
            }
        }
    }
    if let Some(begin) = start
        && let Some(token) = input.get(begin..)
    {
        tokens.push(token);
    }
    tokens
}

/// Normalize a type name to lower-case words joined by single hyphens.
fn normalize(name: &str) -> String {
    let mut out = String::new();
    let mut previous_separator = true;
    for ch in name.trim().chars() {
        if ch == '_' || ch.is_whitespace() || ch == '-' {
            if previous_separator {
                continue;
            }
            previous_separator = true;
            out.push('-');
        } else {
            previous_separator = false;
            out.push(ch.to_ascii_lowercase());
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// Build the malformed-input error for a specifier.
fn bad(source: &str) -> FfiError {
    FfiError::UnknownAlienType(source.trim().to_owned())
}

/// A cursor over the token stream of one type specifier.
struct Parser<'a> {
    items: &'a [&'a str],
    position: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&'a str> {
        self.items.get(self.position).copied()
    }

    fn advance(&mut self) -> Option<&'a str> {
        let item = self.peek();
        if item.is_some() {
            self.position += 1;
        }
        item
    }

    fn expect_token(&mut self, token: &str, source: &str) -> Result<(), FfiError> {
        if self.advance() == Some(token) {
            Ok(())
        } else {
            Err(bad(source))
        }
    }

    fn parse_spec(&mut self, source: &str) -> Result<AlienType, FfiError> {
        match self.peek() {
            Some("(") => self.parse_form(source),
            Some(atom) => {
                self.position += 1;
                parse_type_name(atom)
            }
            None => Err(bad(source)),
        }
    }

    fn parse_form(&mut self, source: &str) -> Result<AlienType, FfiError> {
        self.expect_token("(", source)?;
        let head = self.advance().ok_or_else(|| bad(source))?;
        let ty = match head {
            "pointer" => AlienType::pointer(self.parse_spec(source)?),
            "array" => {
                let element = self.parse_spec(source)?;
                AlienType::array(element, self.parse_usize(source)?)
            }
            "struct" => AlienType::Structure(self.parse_record(source)?),
            "union" => AlienType::Union(self.parse_record(source)?),
            "enum" => AlienType::Enumeration(self.parse_enumeration(source)?),
            "function" => {
                let result = self.parse_spec(source)?;
                let arguments = self.parse_arguments(source)?;
                AlienType::Function(Box::new(AlienRoutine::new("", arguments, result, false)))
            }
            other => return Err(FfiError::UnknownAlienType(other.to_owned())),
        };
        self.expect_token(")", source)?;
        Ok(ty)
    }

    fn parse_usize(&mut self, source: &str) -> Result<usize, FfiError> {
        let token = self.advance().ok_or_else(|| bad(source))?;
        token.parse::<usize>().map_err(|_| bad(source))
    }

    fn parse_record(&mut self, source: &str) -> Result<AlienRecord, FfiError> {
        let name = self.advance().ok_or_else(|| bad(source))?.to_owned();
        let mut fields = Vec::new();
        while self.peek() == Some("(") {
            self.expect_token("(", source)?;
            let field_name = self.advance().ok_or_else(|| bad(source))?.to_owned();
            let field_type = self.parse_spec(source)?;
            self.expect_token(")", source)?;
            fields.push((field_name, field_type));
        }
        Ok(AlienRecord::new(name, fields))
    }

    fn parse_enumeration(&mut self, source: &str) -> Result<AlienEnum, FfiError> {
        let name = self.advance().ok_or_else(|| bad(source))?.to_owned();
        let mut variants = Vec::new();
        while self.peek() == Some("(") {
            self.expect_token("(", source)?;
            let variant = self.advance().ok_or_else(|| bad(source))?.to_owned();
            let value = self
                .advance()
                .ok_or_else(|| bad(source))?
                .parse::<i64>()
                .map_err(|_| bad(source))?;
            self.expect_token(")", source)?;
            variants.push((variant, value));
        }
        Ok(AlienEnum::new(name, variants))
    }

    fn parse_arguments(&mut self, source: &str) -> Result<Vec<AlienType>, FfiError> {
        self.expect_token("(", source)?;
        let mut arguments = Vec::new();
        while self.peek() != Some(")") {
            if self.peek() == Some("(") {
                self.expect_token("(", source)?;
                let _name = self.advance().ok_or_else(|| bad(source))?;
                arguments.push(self.parse_spec(source)?);
                self.expect_token(")", source)?;
            } else {
                arguments.push(self.parse_spec(source)?);
            }
        }
        self.expect_token(")", source)?;
        Ok(arguments)
    }
}
