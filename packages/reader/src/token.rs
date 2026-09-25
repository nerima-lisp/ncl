//! Token reading: symbol and number tokens, escapes, and package prefixes.

use ncl_object::{Package, Runtime, ThreadContext, Word};

use crate::error::ReadError;
use crate::number::parse_number;
use crate::reader::ReadOptions;
use crate::readtable::{ReadtableCase, SyntaxKind, readtable_from_word, syntax_kind};

/// One character in a token and whether it was introduced by an escape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenChar {
    character: char,
    escaped: bool,
}

impl TokenChar {
    #[must_use]
    const fn new(character: char, escaped: bool) -> Self {
        Self { character, escaped }
    }

    #[must_use]
    /// Return the source character.
    pub const fn character(self) -> char {
        self.character
    }

    #[must_use]
    /// Return whether the character was introduced by an escape.
    pub const fn is_escaped(self) -> bool {
        self.escaped
    }
}

/// A lexical token, retaining escape information for every character.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Token {
    characters: Vec<TokenChar>,
}

impl Token {
    fn from_parts(chars: Vec<char>, escaped: Vec<bool>) -> Self {
        let characters = chars
            .into_iter()
            .zip(escaped)
            .map(|(character, escaped)| TokenChar::new(character, escaped))
            .collect();
        Self { characters }
    }

    #[must_use]
    /// Borrow the token characters.
    pub fn characters(&self) -> &[TokenChar] {
        &self.characters
    }

    #[must_use]
    fn any_escaped(&self) -> bool {
        self.characters.iter().any(|c| c.escaped)
    }

    /// Split this token into package and name components.
    fn split(&self) -> Result<TokenParts, ReadError> {
        split_marker(&self.characters)
    }

    /// Fold a token component according to the readtable case.
    #[must_use]
    /// Case-fold a token component without changing escaped characters.
    pub fn fold_name(&self, range: &std::ops::Range<usize>, case: ReadtableCase) -> String {
        range
            .clone()
            .filter_map(|index| self.characters.get(index))
            .map(|character| {
                if character.is_escaped() {
                    character.character()
                } else {
                    fold_char(character.character(), case)
                }
            })
            .collect()
    }
}

/// The kind of package marker in a token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Marker {
    None,
    Keyword,
    Named,
    PackageObject,
}

/// The package and name ranges produced by [`Token::split`].
#[derive(Clone, Debug, Eq, PartialEq)]
struct TokenParts {
    marker: Marker,
    package: std::ops::Range<usize>,
    name: std::ops::Range<usize>,
}

impl TokenParts {
    #[must_use]
    const fn marker(&self) -> Marker {
        self.marker
    }

    #[must_use]
    const fn package(&self) -> &std::ops::Range<usize> {
        &self.package
    }

    #[must_use]
    const fn name(&self) -> &std::ops::Range<usize> {
        &self.name
    }
}

/// Read the raw characters and escape flags of one token, or `None` at end of
/// input.
///
/// The caller has already established that the next character begins a token.
/// `rt` is a rooted slot holding the active readtable word; it is re-read
/// before every use so a collection that moves the readtable cannot leave a
/// stale value.
pub fn read_token_chars(
    ctx: &mut ThreadContext,
    source: &mut dyn crate::input::CharSource,
    rt: &Word,
) -> Result<Option<Token>, ReadError> {
    let mut chars: Vec<char> = Vec::new();
    let mut escaped: Vec<bool> = Vec::new();
    while let Some(next) = source.peek_char() {
        let kind = syntax_kind(ctx, readtable_from_word(*rt)?, next)?;
        match kind {
            SyntaxKind::Constituent | SyntaxKind::NonTerminatingMacro => {
                source.read_char();
                chars.push(next);
                escaped.push(false);
            }
            SyntaxKind::SingleEscape => {
                source.read_char();
                let Some(esc) = source.read_char() else {
                    return Err(ReadError::UnexpectedEof);
                };
                chars.push(esc);
                escaped.push(true);
            }
            SyntaxKind::MultipleEscape => {
                source.read_char();
                loop {
                    match source.read_char() {
                        Some('|') => break,
                        Some(ch) => {
                            chars.push(ch);
                            escaped.push(true);
                        }
                        None => return Err(ReadError::UnexpectedEof),
                    }
                }
            }
            SyntaxKind::Whitespace
            | SyntaxKind::TerminatingMacro
            | SyntaxKind::Invalid
            | SyntaxKind::CustomMacro(_) => break,
        }
    }
    if chars.is_empty() {
        return Ok(None);
    }
    Ok(Some(Token::from_parts(chars, escaped)))
}

/// Read one token (a number or a symbol), or `None` at end of input.
pub fn read_token(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn crate::input::CharSource,
    opts: &ReadOptions,
    rt: &Word,
) -> Result<Option<Word>, ReadError> {
    let Some(token) = read_token_chars(ctx, source, rt)? else {
        return Ok(None);
    };
    let raw_chars: Vec<char> = token.characters().iter().map(|c| c.character()).collect();
    let any_escaped = token.any_escaped();
    if !any_escaped
        && let Some(number) = parse_number(
            ctx,
            runtime,
            &raw_chars,
            opts.read_base().value(),
            opts.default_float_format(),
        )?
    {
        return Ok(Some(number));
    }
    intern_symbol(ctx, runtime, runtime, &token, opts, rt).map(Some)
}

/// Split a raw token on its unescaped colon package marker.
fn split_marker(chars: &[TokenChar]) -> Result<TokenParts, ReadError> {
    let colons: Vec<usize> = chars
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.is_escaped() && c.character() == ':')
        .map(|(i, _)| i)
        .collect();
    let token: String = chars.iter().map(|c| c.character()).collect();
    let length = chars.len();
    match colons.as_slice() {
        [] => Ok(TokenParts {
            marker: Marker::None,
            package: 0..0,
            name: 0..length,
        }),
        [p] => {
            if *p == 0 {
                if length == 1 {
                    return Err(ReadError::InvalidSymbolToken(token));
                }
                Ok(TokenParts {
                    marker: Marker::Keyword,
                    package: 0..0,
                    name: 1..length,
                })
            } else if *p == length - 1 {
                Ok(TokenParts {
                    marker: Marker::PackageObject,
                    package: 0..*p,
                    name: *p..*p,
                })
            } else {
                Ok(TokenParts {
                    marker: Marker::Named,
                    package: 0..*p,
                    name: *p + 1..length,
                })
            }
        }
        [p1, p2] => {
            if *p2 != *p1 + 1 || *p1 == 0 {
                return Err(ReadError::InvalidSymbolToken(token));
            }
            if *p2 == length - 1 {
                Ok(TokenParts {
                    marker: Marker::PackageObject,
                    package: 0..*p1,
                    name: *p1..*p1,
                })
            } else {
                Ok(TokenParts {
                    marker: Marker::Named,
                    package: 0..*p1,
                    name: *p2 + 1..length,
                })
            }
        }
        _ => Err(ReadError::InvalidSymbolToken(token)),
    }
}

/// Case-fold a single constituent character.
const fn fold_char(ch: char, case: ReadtableCase) -> char {
    match case {
        ReadtableCase::Upcase => ch.to_ascii_uppercase(),
        ReadtableCase::Downcase => ch.to_ascii_lowercase(),
        ReadtableCase::Preserve => ch,
        ReadtableCase::Invert => {
            if ch.is_ascii_uppercase() {
                ch.to_ascii_lowercase()
            } else if ch.is_ascii_lowercase() {
                ch.to_ascii_uppercase()
            } else {
                ch
            }
        }
    }
}

/// Package lookup boundary for token interpretation.
trait PackageResolver {
    fn resolve_package(&self, ctx: &ThreadContext, name: &crate::PackageName) -> Option<Word>;
}

impl PackageResolver for Runtime {
    fn resolve_package(&self, ctx: &ThreadContext, name: &crate::PackageName) -> Option<Word> {
        self.find_package(ctx, name.as_str())
    }
}

/// Intern a token as a symbol, applying package-prefix resolution and case.
fn intern_symbol(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    resolver: &impl PackageResolver,
    token: &Token,
    opts: &ReadOptions,
    rt: &Word,
) -> Result<Word, ReadError> {
    let case = readtable_from_word(*rt)?.case_mode(ctx)?;
    let parts = token.split()?;
    let symbol_name = token.fold_name(parts.name(), case);
    let package_name = match parts.marker() {
        Marker::None => opts
            .current_package()
            .map_or("COMMON-LISP-USER", crate::PackageName::as_str)
            .to_owned(),
        Marker::Keyword => "KEYWORD".to_owned(),
        Marker::Named => token.fold_name(parts.package(), case),
        Marker::PackageObject => {
            let name = token.fold_name(parts.package(), case);
            let package_name = crate::PackageName::new(name.clone())?;
            let package = resolver
                .resolve_package(ctx, &package_name)
                .ok_or_else(|| ReadError::PackageNotFound(name.clone()))?;
            return Ok(package);
        }
    };
    let package_name_value = crate::PackageName::new(package_name.clone())?;
    let package = resolver
        .resolve_package(ctx, &package_name_value)
        .ok_or_else(|| ReadError::PackageNotFound(package_name.clone()))?;
    let (symbol, _) = Package::from(package).intern(ctx, runtime, &symbol_name)?;
    Ok(symbol)
}

/// Intern a reader-generated symbol in the `COMMON-LISP` package.
pub fn intern_common_lisp(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<Word, ReadError> {
    let package = runtime
        .find_package(ctx, "COMMON-LISP")
        .ok_or_else(|| ReadError::PackageNotFound("COMMON-LISP".to_owned()))?;
    let (symbol, _) = Package::from(package).intern(ctx, runtime, name)?;
    Ok(symbol)
}
