//! The recursive reader and its public entry points.

use ncl_object::{
    Package, Runtime, ThreadContext, Word, make_cons, make_string, pop_root, push_root, rplacd,
};

use crate::error::ReadError;
use crate::input::{CharSource, StringSource};
use crate::readtable::{Readtable, SyntaxKind, readtable_from_word, syntax_kind};
use crate::token::read_token;

/// The float format used when a token has no explicit float marker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FloatFormat {
    /// Single-float; not representable by the object layer yet.
    SingleFloat,
    /// Double-float (the default).
    DoubleFloat,
}

/// Options controlling a single [`read`] call.
///
/// The fields mirror the reader dynamic variables `*read-base*`,
/// `*read-eval*`, `*read-suppress*`, `*read-default-float-format*`, and the
/// active readtable. `*readtable*` dynamic rebinding is not wired yet, so the
/// readtable is passed explicitly.
#[derive(Clone, Debug)]
pub struct ReadOptions {
    /// The active readtable.
    pub readtable: Readtable,
    /// The input radix for integers and ratios, in the closed range `2..=36`.
    pub read_base: u32,
    /// Whether `#.` is permitted (it has no evaluator in this build).
    pub read_eval: bool,
    /// When true, forms are read and discarded, and [`read`] returns `NIL`.
    pub read_suppress: bool,
    /// The float format for markers without an explicit type.
    pub read_default_float_format: FloatFormat,
    /// Package used to intern unprefixed symbols; `None` means `COMMON-LISP-USER`.
    pub current_package: Option<String>,
}

impl ReadOptions {
    /// Options with the standard readtable and default dynamic-variable values.
    ///
    /// # Errors
    /// Returns an object-layer failure when the readtable cannot be built.
    pub fn standard(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<Self, ReadError> {
        Ok(Self {
            readtable: crate::readtable::standard_readtable(ctx, runtime)?,
            read_base: 10,
            read_eval: true,
            read_suppress: false,
            read_default_float_format: FloatFormat::DoubleFloat,
            current_package: None,
        })
    }
}

/// Read the next form from `source`, skipping leading whitespace and comments.
///
/// Returns `None` at end of input. The readtable word and the labels table are
/// rooted for the duration of the call so a collection cannot leave a stale
/// value.
///
/// # Errors
/// Returns a [`ReadError`] describing the lexical or dispatch failure.
pub fn read(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
    opts: &ReadOptions,
) -> Result<Option<Word>, ReadError> {
    let mut rt = opts.readtable.object().as_word();
    let rt_token = push_root(ctx, &mut rt);
    let mut labels = Word::NIL;
    let labels_token = push_root(ctx, &mut labels);
    let result = read_form(ctx, runtime, source, opts, &rt, &mut labels);
    let _ = pop_root(ctx, labels_token);
    let _ = pop_root(ctx, rt_token);
    result
}

/// Read the next form, preserving any trailing whitespace.
///
/// This reader never consumes trailing whitespace, so the behavior is the same
/// as [`read`].
///
/// # Errors
/// Returns a [`ReadError`] describing the lexical or dispatch failure.
pub fn read_preserving_whitespace(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
    opts: &ReadOptions,
) -> Result<Option<Word>, ReadError> {
    read(ctx, runtime, source, opts)
}

/// Read the next form from a string slice.
///
/// # Errors
/// Returns a [`ReadError`] describing the lexical or dispatch failure.
pub fn read_from_string(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    text: &str,
    opts: &ReadOptions,
) -> Result<Option<Word>, ReadError> {
    let mut source = StringSource::new(text);
    read(ctx, runtime, &mut source, opts)
}

/// Read a list of forms terminated by a `)`, returning the proper list.
///
/// The opening parenthesis is not consumed by this function; it reads forms
/// until an unmatched `)`.
///
/// # Errors
/// Returns [`ReadError::UnexpectedEof`] when input ends before the `)`.
pub fn read_delimited_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
    opts: &ReadOptions,
) -> Result<Word, ReadError> {
    let mut rt = opts.readtable.object().as_word();
    let rt_token = push_root(ctx, &mut rt);
    let mut labels = Word::NIL;
    let labels_token = push_root(ctx, &mut labels);
    let result = read_list(ctx, runtime, source, opts, &rt, &mut labels);
    let _ = pop_root(ctx, labels_token);
    let _ = pop_root(ctx, rt_token);
    result
}

/// Read one form, recursing past comments and suppressed forms.
///
/// `rt` and `labels` are rooted slots owned by the caller. They are re-read
/// before every use, so a collection that moves the readtable or labels table
/// cannot leave a stale value.
pub fn read_form(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
    opts: &ReadOptions,
    rt: &Word,
    labels: &mut Word,
) -> Result<Option<Word>, ReadError> {
    loop {
        skip_whitespace(ctx, source, opts, rt)?;
        let Some(c) = source.peek_char() else {
            return Ok(None);
        };
        let kind = syntax_kind(ctx, readtable_from_word(*rt)?, c)?;
        match kind {
            SyntaxKind::Constituent | SyntaxKind::SingleEscape | SyntaxKind::MultipleEscape => {
                let form = read_token(ctx, runtime, source, opts, rt)?;
                return Ok(apply_suppress(form, opts));
            }
            SyntaxKind::TerminatingMacro | SyntaxKind::NonTerminatingMacro => {
                source.read_char();
                if let Some(word) = read_macro_char(ctx, runtime, source, opts, rt, labels, c)? {
                    return Ok(apply_suppress(Some(word), opts));
                }
            }
            SyntaxKind::CustomMacro(_) | SyntaxKind::Invalid => {
                return Err(ReadError::UninvocableMacroFunction(c));
            }
            SyntaxKind::Whitespace => {}
        }
    }
}

/// Honour `*read-suppress*` by returning `NIL` instead of a constructed form.
const fn apply_suppress(form: Option<Word>, opts: &ReadOptions) -> Option<Word> {
    if opts.read_suppress {
        Some(Word::NIL)
    } else {
        form
    }
}

/// Skip whitespace and `;` line comments.
fn skip_whitespace(
    ctx: &mut ThreadContext,
    source: &mut dyn CharSource,
    _opts: &ReadOptions,
    rt: &Word,
) -> Result<(), ReadError> {
    loop {
        let Some(c) = source.peek_char() else {
            return Ok(());
        };
        let kind = syntax_kind(ctx, readtable_from_word(*rt)?, c)?;
        match kind {
            SyntaxKind::Whitespace => {
                source.read_char();
            }
            SyntaxKind::TerminatingMacro if c == ';' => {
                source.read_char();
                while let Some(ch) = source.read_char() {
                    if ch == '\n' {
                        break;
                    }
                }
            }
            _ => return Ok(()),
        }
    }
}

/// Dispatch a terminating or non-terminating macro character.
fn read_macro_char(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
    opts: &ReadOptions,
    rt: &Word,
    labels: &mut Word,
    c: char,
) -> Result<Option<Word>, ReadError> {
    match c {
        '(' => read_list(ctx, runtime, source, opts, rt, labels).map(Some),
        ')' => Err(ReadError::UnmatchedRightParen),
        '\'' => read_quoted(ctx, runtime, source, opts, rt, labels, "QUOTE").map(Some),
        '`' => read_quoted(ctx, runtime, source, opts, rt, labels, "QUASIQUOTE").map(Some),
        ',' => {
            if source.peek_char() == Some('@') {
                source.read_char();
                read_quoted(ctx, runtime, source, opts, rt, labels, "UNQUOTE-SPLICING").map(Some)
            } else {
                read_quoted(ctx, runtime, source, opts, rt, labels, "UNQUOTE").map(Some)
            }
        }
        '"' => read_string(ctx, runtime, source).map(Some),
        '#' => crate::dispatch::read_sharp(ctx, runtime, source, opts, rt, labels),
        _ => Err(ReadError::UninvocableMacroFunction(c)),
    }
}

/// Read a `'x`, `` `x ``, `,x`, or `,@x` form, expanding to a cons.
fn read_quoted(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
    opts: &ReadOptions,
    rt: &Word,
    labels: &mut Word,
    name: &str,
) -> Result<Word, ReadError> {
    let form = read_form(ctx, runtime, source, opts, rt, labels)?;
    let Some(form) = form else {
        return Err(ReadError::UnexpectedEof);
    };
    let symbol = intern_common_lisp(ctx, runtime, name)?;
    let cell = make_cons(ctx, runtime, form, Word::NIL)?;
    Ok(make_cons(ctx, runtime, symbol, cell)?)
}

/// Read a `"..."` string literal.
fn read_string(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
) -> Result<Word, ReadError> {
    let mut chars: Vec<char> = Vec::new();
    loop {
        match source.read_char() {
            None => return Err(ReadError::UnexpectedEof),
            Some('"') => break,
            Some('\\') => {
                let Some(escaped) = source.read_char() else {
                    return Err(ReadError::UnexpectedEof);
                };
                chars.push(escaped);
            }
            Some(ch) => chars.push(ch),
        }
    }
    Ok(make_string(ctx, runtime, &chars)?)
}

/// Read a proper list terminated by `)`, with optional dotted-pair support.
pub fn read_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
    opts: &ReadOptions,
    rt: &Word,
    labels: &mut Word,
) -> Result<Word, ReadError> {
    let mut head = Word::NIL;
    let mut tail = Word::NIL;
    let head_token = push_root(ctx, &mut head);
    let tail_token = push_root(ctx, &mut tail);
    let result = read_list_inner(ctx, runtime, source, opts, rt, labels, &mut head, &mut tail);
    let _ = pop_root(ctx, tail_token);
    let _ = pop_root(ctx, head_token);
    result
}

/// The loop body of [`read_list`]; `head` and `tail` are caller-rooted slots.
fn read_list_inner(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
    opts: &ReadOptions,
    rt: &Word,
    labels: &mut Word,
    head: &mut Word,
    tail: &mut Word,
) -> Result<Word, ReadError> {
    loop {
        skip_whitespace(ctx, source, opts, rt)?;
        let Some(c) = source.peek_char() else {
            return Err(ReadError::UnexpectedEof);
        };
        if c == ')' {
            source.read_char();
            break;
        }
        if c == '.' {
            source.read_char();
            let follower = source.peek_char();
            if follower.is_none_or(|n| n.is_whitespace() || n == ')') {
                if *head == Word::NIL {
                    return Err(ReadError::DotWithoutCdr);
                }
                let cdr = read_form(ctx, runtime, source, opts, rt, labels)?;
                let Some(cdr) = cdr else {
                    return Err(ReadError::DotWithoutCdr);
                };
                rplacd(ctx, *tail, cdr)?;
                skip_whitespace(ctx, source, opts, rt)?;
                if source.read_char() != Some(')') {
                    return Err(ReadError::UnmatchedRightParen);
                }
                break;
            }
            source.unread_char('.');
        }
        let form = read_form(ctx, runtime, source, opts, rt, labels)?;
        let Some(form) = form else {
            return Err(ReadError::UnexpectedEof);
        };
        let cell = make_cons(ctx, runtime, form, Word::NIL)?;
        if *head == Word::NIL {
            *head = cell;
        } else {
            rplacd(ctx, *tail, cell)?;
        }
        *tail = cell;
    }
    Ok(*head)
}

/// Intern a symbol in the `COMMON-LISP` package.
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
