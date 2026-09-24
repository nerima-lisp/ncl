//! Token reading: symbol and number tokens, escapes, and package prefixes.

use ncl_object::{Package, Runtime, ThreadContext, Word};

use crate::error::ReadError;
use crate::number::parse_number;
use crate::reader::ReadOptions;
use crate::readtable::{ReadtableCase, SyntaxKind, readtable_from_word, syntax_kind};

/// A raw token: its characters and the escape flag of each.
type RawToken = (Vec<char>, Vec<bool>);

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
) -> Result<Option<RawToken>, ReadError> {
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
    Ok(Some((chars, escaped)))
}

/// Read one token (a number or a symbol), or `None` at end of input.
pub fn read_token(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn crate::input::CharSource,
    opts: &ReadOptions,
    rt: &Word,
) -> Result<Option<Word>, ReadError> {
    let Some((chars, escaped)) = read_token_chars(ctx, source, rt)? else {
        return Ok(None);
    };
    let any_escaped = escaped.iter().any(|&e| e);
    if !any_escaped
        && let Some(number) = parse_number(
            ctx,
            runtime,
            &chars,
            opts.read_base,
            opts.read_default_float_format,
        )?
    {
        return Ok(Some(number));
    }
    intern_symbol(ctx, runtime, &chars, &escaped, opts, rt).map(Some)
}

/// The kind of package reference a token carries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Marker {
    /// No marker: intern in the current package.
    None,
    /// A leading `:`: intern in the `KEYWORD` package.
    Keyword,
    /// A `pkg:name` or `pkg::name` marker: intern in the named package.
    Named,
    /// A trailing `pkg:` or `pkg::`: the token denotes a package object.
    PackageObject,
}

/// The split points of a token into package and symbol name ranges.
#[derive(Clone, Copy, Debug)]
struct TokenParts {
    marker: Marker,
    pkg_start: usize,
    pkg_end: usize,
    sym_start: usize,
    sym_end: usize,
}

/// Split a raw token on its unescaped colon package marker.
fn split_marker(chars: &[char], escaped: &[bool]) -> Result<TokenParts, ReadError> {
    let colons: Vec<usize> = (0..chars.len())
        .filter(|&i| !escaped[i] && chars[i] == ':')
        .collect();
    let token: String = chars.iter().collect();
    match colons.as_slice() {
        [] => Ok(TokenParts {
            marker: Marker::None,
            pkg_start: 0,
            pkg_end: 0,
            sym_start: 0,
            sym_end: chars.len(),
        }),
        [p] => {
            if *p == 0 {
                if chars.len() == 1 {
                    return Err(ReadError::InvalidSymbolToken(token));
                }
                Ok(TokenParts {
                    marker: Marker::Keyword,
                    pkg_start: 0,
                    pkg_end: 0,
                    sym_start: 1,
                    sym_end: chars.len(),
                })
            } else if *p == chars.len() - 1 {
                Ok(TokenParts {
                    marker: Marker::PackageObject,
                    pkg_start: 0,
                    pkg_end: *p,
                    sym_start: *p,
                    sym_end: *p,
                })
            } else {
                Ok(TokenParts {
                    marker: Marker::Named,
                    pkg_start: 0,
                    pkg_end: *p,
                    sym_start: *p + 1,
                    sym_end: chars.len(),
                })
            }
        }
        [p1, p2] => {
            if *p2 != *p1 + 1 || *p1 == 0 {
                return Err(ReadError::InvalidSymbolToken(token));
            }
            if *p2 == chars.len() - 1 {
                Ok(TokenParts {
                    marker: Marker::PackageObject,
                    pkg_start: 0,
                    pkg_end: *p1,
                    sym_start: *p1,
                    sym_end: *p1,
                })
            } else {
                Ok(TokenParts {
                    marker: Marker::Named,
                    pkg_start: 0,
                    pkg_end: *p1,
                    sym_start: *p2 + 1,
                    sym_end: chars.len(),
                })
            }
        }
        _ => Err(ReadError::InvalidSymbolToken(token)),
    }
}

/// Case-fold a name segment, leaving escaped characters untouched.
pub fn fold_name(chars: &[char], escaped: &[bool], case: ReadtableCase) -> String {
    chars
        .iter()
        .zip(escaped)
        .map(|(&ch, &esc)| if esc { ch } else { fold_char(ch, case) })
        .collect()
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

/// Intern a token as a symbol, applying package-prefix resolution and case.
fn intern_symbol(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    chars: &[char],
    escaped: &[bool],
    opts: &ReadOptions,
    rt: &Word,
) -> Result<Word, ReadError> {
    let case = readtable_from_word(*rt)?.case_mode(ctx)?;
    let parts = split_marker(chars, escaped)?;
    let symbol_name = fold_name(
        &chars[parts.sym_start..parts.sym_end],
        &escaped[parts.sym_start..parts.sym_end],
        case,
    );
    let package_name = match parts.marker {
        Marker::None => opts
            .current_package
            .as_deref()
            .unwrap_or("COMMON-LISP-USER")
            .to_owned(),
        Marker::Keyword => "KEYWORD".to_owned(),
        Marker::Named => fold_name(
            &chars[parts.pkg_start..parts.pkg_end],
            &escaped[parts.pkg_start..parts.pkg_end],
            case,
        ),
        Marker::PackageObject => {
            let name = fold_name(
                &chars[parts.pkg_start..parts.pkg_end],
                &escaped[parts.pkg_start..parts.pkg_end],
                case,
            );
            let package = runtime
                .find_package(ctx, &name)
                .ok_or_else(|| ReadError::PackageNotFound(name.clone()))?;
            return Ok(package);
        }
    };
    let package = runtime
        .find_package(ctx, &package_name)
        .ok_or_else(|| ReadError::PackageNotFound(package_name.clone()))?;
    let (symbol, _) = Package::from(package).intern(ctx, runtime, &symbol_name)?;
    Ok(symbol)
}
