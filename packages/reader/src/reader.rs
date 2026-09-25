//! The recursive reader and its public entry points.

use ncl_object::{
    Runtime, ThreadContext, Word, make_cons, make_string, pop_root, push_root, rplacd,
};

use crate::error::ReadError;
use crate::input::{CharSource, StringSource};
use crate::readtable::{Readtable, SyntaxKind, readtable_from_word, syntax_kind};
use crate::token::{intern_common_lisp, read_token};

/// The float format used when a token has no explicit float marker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FloatFormat {
    /// Single-float; not representable by the object layer yet.
    SingleFloat,
    /// Double-float (the default).
    DoubleFloat,
}

/// The radix used to interpret integer tokens.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadBase(u32);

impl ReadBase {
    /// Construct a radix in the Common Lisp range `2..=36`.
    ///
    /// # Errors
    /// Returns [`ReadError::InvalidBase`] when `base` is outside the range.
    pub const fn new(base: u32) -> Result<Self, ReadError> {
        if base < 2 || base > 36 {
            Err(ReadError::InvalidBase(base))
        } else {
            Ok(Self(base))
        }
    }

    /// Return the numeric radix for the object-layer parser boundary.
    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }
}

/// Whether `#.` reader evaluation is permitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadEvaluation {
    /// Permit the `#.` syntax, subject to evaluator availability.
    Enabled,
    /// Reject the `#.` syntax.
    Disabled,
}

/// Whether forms are constructed or discarded while reading.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadSuppression {
    /// Construct and return the form.
    Keep,
    /// Read the form for syntax only and return `NIL`.
    Discard,
}

/// A package name selected as the current reader package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageName(String);

impl PackageName {
    /// Construct a non-empty package name.
    ///
    /// # Errors
    /// Returns [`ReadError::InvalidSymbolToken`] when `name` is empty.
    pub fn new(name: impl Into<String>) -> Result<Self, ReadError> {
        let name = name.into();
        if name.is_empty() {
            Err(ReadError::InvalidSymbolToken(name))
        } else {
            Ok(Self(name))
        }
    }

    /// Borrow the package name at the runtime lookup boundary.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Options controlling a single [`read`] call.
///
/// The fields mirror the reader dynamic variables `*read-base*`,
/// `*read-eval*`, `*read-suppress*`, `*read-default-float-format*`, and the
/// active readtable. `*readtable*` dynamic rebinding is not wired yet, so the
/// readtable is passed explicitly.
#[derive(Clone, Debug)]
pub struct ReadOptions {
    readtable: Readtable,
    read_base: ReadBase,
    read_eval: ReadEvaluation,
    read_suppress: ReadSuppression,
    read_default_float_format: FloatFormat,
    current_package: Option<PackageName>,
}

impl ReadOptions {
    /// Options with the standard readtable and default dynamic-variable values.
    ///
    /// # Errors
    /// Returns an object-layer failure when the readtable cannot be built.
    pub fn standard(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<Self, ReadError> {
        Ok(Self {
            readtable: crate::readtable::standard_readtable(ctx, runtime)?,
            read_base: ReadBase::new(10)?,
            read_eval: ReadEvaluation::Enabled,
            read_suppress: ReadSuppression::Keep,
            read_default_float_format: FloatFormat::DoubleFloat,
            current_package: None,
        })
    }

    /// Set the active readtable.
    pub const fn set_readtable(&mut self, readtable: Readtable) {
        self.readtable = readtable;
    }

    /// Set the integer radix.
    pub const fn set_read_base(&mut self, base: ReadBase) {
        self.read_base = base;
    }

    /// Set whether reader evaluation is enabled.
    pub const fn set_read_evaluation(&mut self, evaluation: ReadEvaluation) {
        self.read_eval = evaluation;
    }

    /// Set whether forms are suppressed.
    pub const fn set_read_suppression(&mut self, suppression: ReadSuppression) {
        self.read_suppress = suppression;
    }

    /// Set the default float format.
    pub const fn set_default_float_format(&mut self, format: FloatFormat) {
        self.read_default_float_format = format;
    }

    /// Set the current package used for unqualified symbols.
    ///
    /// # Errors
    /// Returns [`ReadError::InvalidSymbolToken`] for an empty package name.
    pub fn set_current_package(&mut self, package: impl Into<String>) -> Result<(), ReadError> {
        self.current_package = Some(PackageName::new(package)?);
        Ok(())
    }

    /// Return the active readtable.
    #[must_use]
    pub const fn readtable(&self) -> Readtable {
        self.readtable
    }

    /// Return the configured radix.
    #[must_use]
    pub const fn read_base(&self) -> ReadBase {
        self.read_base
    }

    /// Return the reader evaluation mode.
    #[must_use]
    pub const fn read_evaluation(&self) -> ReadEvaluation {
        self.read_eval
    }

    /// Return the suppression mode.
    #[must_use]
    pub const fn read_suppression(&self) -> ReadSuppression {
        self.read_suppress
    }

    /// Return the default float format.
    #[must_use]
    pub const fn default_float_format(&self) -> FloatFormat {
        self.read_default_float_format
    }

    /// Return the current package, if one was configured.
    #[must_use]
    pub const fn current_package(&self) -> Option<&PackageName> {
        self.current_package.as_ref()
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
    match opts.read_suppress {
        ReadSuppression::Discard => Some(Word::NIL),
        ReadSuppression::Keep => form,
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
            SyntaxKind::Constituent
            | SyntaxKind::TerminatingMacro
            | SyntaxKind::NonTerminatingMacro
            | SyntaxKind::SingleEscape
            | SyntaxKind::MultipleEscape
            | SyntaxKind::Invalid
            | SyntaxKind::CustomMacro(_) => return Ok(()),
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
