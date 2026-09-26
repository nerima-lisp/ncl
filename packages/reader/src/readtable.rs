//! Readtables: syntax tables, dispatch tables, and case modes.

use ncl_object::{
    ObjectError, ObjectRef, Readtable as ObjectReadtable, Runtime, ThreadContext, Word,
    classify_object, make_cons, make_readtable, make_simple_vector, pop_root, push_root,
    readtable_case as object_readtable_case, readtable_dispatch, readtable_syntax,
    simple_vector_ref, simple_vector_set,
};

use crate::error::ReadError;

/// Number of entries in a syntax or dispatch table (one per base character).
const TABLE_SIZE: usize = 256;
const TABLE_SIZE_U32: u32 = 256;

/// Fixnum encoding of the [`ReadtableCase::Upcase`] mode.
const CASE_UPCASE: i64 = 0;
/// Fixnum encoding of the [`ReadtableCase::Downcase`] mode.
const CASE_DOWNCASE: i64 = 1;
/// Fixnum encoding of the [`ReadtableCase::Preserve`] mode.
const CASE_PRESERVE: i64 = 2;
/// Fixnum encoding of the [`ReadtableCase::Invert`] mode.
const CASE_INVERT: i64 = 3;

/// Fixnum encoding of a constituent syntax entry.
const SYNTAX_CONSTITUENT: i64 = 0;
/// Fixnum encoding of a whitespace syntax entry.
const SYNTAX_WHITESPACE: i64 = 1;
/// Fixnum encoding of a terminating macro character entry.
const SYNTAX_TERMINATING_MACRO: i64 = 2;
/// Fixnum encoding of a non-terminating macro character entry.
const SYNTAX_NON_TERMINATING_MACRO: i64 = 3;
/// Fixnum encoding of a single-escape entry (`\`).
const SYNTAX_SINGLE_ESCAPE: i64 = 4;
/// Fixnum encoding of a multiple-escape entry (`|`).
const SYNTAX_MULTIPLE_ESCAPE: i64 = 5;

/// How a readtable case-folds symbol names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ReadtableCase {
    /// Uppercase every unescaped constituent (`*readtable*` default).
    Upcase,
    /// Lowercase every unescaped constituent.
    Downcase,
    /// Preserve the case of every unescaped constituent.
    Preserve,
    /// Invert the case of every unescaped constituent.
    Invert,
}

/// How a character behaves inside a readtable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SyntaxKind {
    /// Part of a symbol or number token.
    Constituent,
    /// Separates tokens and is ignored.
    Whitespace,
    /// A macro character that terminates a token.
    TerminatingMacro,
    /// A macro character that does not terminate a token.
    NonTerminatingMacro,
    /// The single-escape character (`\`).
    SingleEscape,
    /// The multiple-escape character (`|`).
    MultipleEscape,
    /// A character with no defined syntax.
    Invalid,
    /// A user-installed macro character and its termination behavior.
    CustomMacro(CustomMacroKind),
}

/// Whether an installed reader macro terminates a token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CustomMacroKind {
    /// The macro terminates a preceding token.
    Terminating,
    /// The macro may occur within a token.
    NonTerminating,
}

/// A Common Lisp readtable: a syntax table, a dispatch table, and a case mode.
///
/// The type wraps the `ncl_object` readtable descriptor. The syntax and
/// dispatch tables are 256-entry simple vectors indexed by character code;
/// characters above 255 are treated as constituents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Readtable {
    object: ObjectReadtable,
}

impl Readtable {
    /// The underlying `ncl_object` readtable word.
    #[must_use]
    pub const fn object(self) -> ObjectReadtable {
        self.object
    }

    /// Wrap an `ncl_object` readtable descriptor.
    #[must_use]
    pub const fn from_object(object: ObjectReadtable) -> Self {
        Self { object }
    }

    /// The readtable case mode.
    ///
    /// # Errors
    /// Returns [`ReadError::Object`] when the case slot is malformed.
    pub fn case_mode(&self, ctx: &ThreadContext) -> Result<ReadtableCase, ReadError> {
        let word = object_readtable_case(ctx, self.object)?;
        let code = word
            .as_fixnum()
            .ok_or_else(|| ReadError::InvalidNumber(format!("{word:?}")))?;
        match code {
            CASE_UPCASE => Ok(ReadtableCase::Upcase),
            CASE_DOWNCASE => Ok(ReadtableCase::Downcase),
            CASE_PRESERVE => Ok(ReadtableCase::Preserve),
            CASE_INVERT => Ok(ReadtableCase::Invert),
            _ => Err(ReadError::InvalidNumber(format!("{word:?}"))),
        }
    }

    /// The syntax table word (a 256-entry simple vector).
    ///
    /// # Errors
    /// Returns an object-layer failure when the table is malformed.
    pub fn syntax_table(&self, ctx: &ThreadContext) -> Result<Word, ReadError> {
        Ok(readtable_syntax(ctx, self.object)?)
    }

    /// The dispatch table word (a 256-entry simple vector).
    ///
    /// # Errors
    /// Returns an object-layer failure when the table is malformed.
    pub fn dispatch_table(&self, ctx: &ThreadContext) -> Result<Word, ReadError> {
        Ok(readtable_dispatch(ctx, self.object)?)
    }
}

impl From<Readtable> for ObjectReadtable {
    fn from(value: Readtable) -> Self {
        value.object
    }
}

impl From<ObjectReadtable> for Readtable {
    fn from(object: ObjectReadtable) -> Self {
        Self { object }
    }
}

/// Build the standard readtable.
///
/// The standard readtable is case-insensitive with `:upcase` mode, uses `\`
/// and `|` as escapes, `(` `)` `'` `` ` `` `,` `"` `;` as terminating macro
/// characters, `#` as the non-terminating dispatch macro character, and space,
/// tab, newline, return, and form feed as whitespace.
///
/// # Errors
/// Returns an object-layer failure when allocation fails.
pub fn standard_readtable(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
) -> Result<Readtable, ReadError> {
    let mut syntax = [Word::fixnum(SYNTAX_CONSTITUENT); TABLE_SIZE];
    for ch in [' ', '\t', '\n', '\r', '\u{0c}'] {
        if let Some(index) = table_index(ch)
            && let Some(slot) = syntax.get_mut(index)
        {
            *slot = Word::fixnum(SYNTAX_WHITESPACE);
        }
    }
    for ch in ['(', ')', '\'', '`', ',', '"', ';'] {
        if let Some(index) = table_index(ch)
            && let Some(slot) = syntax.get_mut(index)
        {
            *slot = Word::fixnum(SYNTAX_TERMINATING_MACRO);
        }
    }
    for (ch, value) in [
        ('#', SYNTAX_NON_TERMINATING_MACRO),
        ('\\', SYNTAX_SINGLE_ESCAPE),
        ('|', SYNTAX_MULTIPLE_ESCAPE),
    ] {
        if let Some(index) = table_index(ch)
            && let Some(slot) = syntax.get_mut(index)
        {
            *slot = Word::fixnum(value);
        }
    }

    let mut syntax_word = make_simple_vector(ctx, runtime, &syntax)?;
    let syntax_token = push_root(ctx, &mut syntax_word);
    let dispatch = [Word::NIL; TABLE_SIZE];
    let mut dispatch_word = make_simple_vector(ctx, runtime, &dispatch)?;
    let dispatch_token = push_root(ctx, &mut dispatch_word);
    let result = make_readtable(
        ctx,
        runtime,
        syntax_word,
        dispatch_word,
        Word::fixnum(CASE_UPCASE),
    );
    let _ = pop_root(ctx, dispatch_token);
    let _ = pop_root(ctx, syntax_token);
    let object = result?;
    Ok(Readtable::from_object(object))
}

/// Copy a readtable: a new syntax and dispatch table with the same entries.
///
/// # Errors
/// Returns an object-layer failure when allocation fails.
pub fn copy_readtable(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    readtable: Readtable,
) -> Result<Readtable, ReadError> {
    let case = object_readtable_case(ctx, readtable.object)?;
    let syntax_copy = copy_vector(ctx, runtime, readtable.syntax_table(ctx)?)?;
    let mut syntax_copy = syntax_copy;
    let syntax_token = push_root(ctx, &mut syntax_copy);
    let mut dispatch_copy = copy_vector(ctx, runtime, readtable.dispatch_table(ctx)?)?;
    let dispatch_token = push_root(ctx, &mut dispatch_copy);
    let result = make_readtable(ctx, runtime, syntax_copy, dispatch_copy, case);
    let _ = pop_root(ctx, dispatch_token);
    let _ = pop_root(ctx, syntax_token);
    let object = result?;
    Ok(Readtable::from_object(object))
}

/// Copy the contents of a 256-entry simple vector.
fn copy_vector(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: Word,
) -> Result<Word, ReadError> {
    let mut entries = [Word::NIL; TABLE_SIZE];
    for (index, slot) in entries.iter_mut().enumerate() {
        *slot = simple_vector_ref(ctx, source, index)?;
    }
    Ok(make_simple_vector(ctx, runtime, &entries)?)
}

fn table_index(ch: char) -> Option<usize> {
    let code = u32::from(ch);
    if code < TABLE_SIZE_U32 {
        usize::try_from(code).ok()
    } else {
        None
    }
}

/// Classify a character's syntax within a readtable.
pub fn syntax_kind(
    ctx: &mut ThreadContext,
    readtable: Readtable,
    ch: char,
) -> Result<SyntaxKind, ReadError> {
    let Some(index) = table_index(ch) else {
        return Ok(SyntaxKind::Constituent);
    };
    let entry = simple_vector_ref(ctx, readtable.syntax_table(ctx)?, index)?;
    Ok(classify_entry(ctx, entry))
}

/// Classify a raw syntax-table entry word.
fn classify_entry(ctx: &mut ThreadContext, entry: Word) -> SyntaxKind {
    if let Some(kind) = entry.as_fixnum() {
        return match kind {
            SYNTAX_CONSTITUENT => SyntaxKind::Constituent,
            SYNTAX_WHITESPACE => SyntaxKind::Whitespace,
            SYNTAX_TERMINATING_MACRO => SyntaxKind::TerminatingMacro,
            SYNTAX_NON_TERMINATING_MACRO => SyntaxKind::NonTerminatingMacro,
            SYNTAX_SINGLE_ESCAPE => SyntaxKind::SingleEscape,
            SYNTAX_MULTIPLE_ESCAPE => SyntaxKind::MultipleEscape,
            _ => SyntaxKind::Invalid,
        };
    }
    if entry.is_cons() {
        let terminating = ncl_object::cdr(ctx, entry).ok() == Some(Word::NIL);
        return SyntaxKind::CustomMacro(if terminating {
            CustomMacroKind::Terminating
        } else {
            CustomMacroKind::NonTerminating
        });
    }
    SyntaxKind::Invalid
}

/// Return the reader macro function for `ch`, if one is installed.
///
/// # Errors
/// Returns an object-layer failure when the table is malformed.
pub fn get_macro_character(
    ctx: &mut ThreadContext,
    readtable: Readtable,
    ch: char,
) -> Result<Option<Word>, ReadError> {
    let Some(index) = table_index(ch) else {
        return Ok(None);
    };
    let entry = simple_vector_ref(ctx, readtable.syntax_table(ctx)?, index)?;
    if entry.is_cons() {
        Ok(Some(ncl_object::car(ctx, entry)?))
    } else {
        Ok(None)
    }
}

/// Install a reader macro function for `ch`.
///
/// The function word is stored as `(function . non-terminating-p)`. A `None`
/// function removes the macro status and makes `ch` a constituent. Invocation
/// of a user function is deferred until a callable function ABI exists, so a
/// non-standard macro character signals
/// [`ReadError::UninvocableMacroFunction`] when read.
///
/// # Errors
/// Returns an object-layer failure when the table is malformed.
pub fn set_macro_character(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    readtable: Readtable,
    ch: char,
    function: Option<Word>,
    termination: CustomMacroKind,
) -> Result<(), ReadError> {
    let Some(index) = table_index(ch) else {
        return Ok(());
    };
    let entry = match function {
        Some(function) => {
            let mut function = function;
            let token = push_root(ctx, &mut function);
            let flag = match termination {
                CustomMacroKind::NonTerminating => Word::fixnum(1),
                CustomMacroKind::Terminating => Word::NIL,
            };
            let entry = make_cons(ctx, runtime, function, flag)?;
            let _ = pop_root(ctx, token);
            entry
        }
        None => Word::fixnum(SYNTAX_CONSTITUENT),
    };
    simple_vector_set(ctx, readtable.syntax_table(ctx)?, index, entry)?;
    Ok(())
}

/// Return the dispatch reader macro function for `ch` sub-char `sub`, if any.
///
/// # Errors
/// Returns an object-layer failure when the table is malformed.
pub fn get_dispatch_macro_character(
    ctx: &mut ThreadContext,
    readtable: Readtable,
    ch: char,
    sub: char,
) -> Result<Option<Word>, ReadError> {
    if ch != '#' {
        return Err(ReadError::NotDispatchMacro(ch));
    }
    let Some(index) = table_index(sub) else {
        return Ok(None);
    };
    let entry = simple_vector_ref(ctx, readtable.dispatch_table(ctx)?, index)?;
    if entry.is_cons() {
        Ok(Some(ncl_object::car(ctx, entry)?))
    } else {
        Ok(None)
    }
}

/// Install a dispatch reader macro function for `ch` sub-char `sub`.
///
/// # Errors
/// Returns an object-layer failure when the table is malformed.
pub fn set_dispatch_macro_character(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    readtable: Readtable,
    ch: char,
    sub: char,
    function: Word,
) -> Result<(), ReadError> {
    if ch != '#' {
        return Err(ReadError::NotDispatchMacro(ch));
    }
    let Some(index) = table_index(sub) else {
        return Ok(());
    };
    let mut function = function;
    let token = push_root(ctx, &mut function);
    let entry = make_cons(ctx, runtime, function, Word::NIL)?;
    let _ = pop_root(ctx, token);
    simple_vector_set(ctx, readtable.dispatch_table(ctx)?, index, entry)?;
    Ok(())
}

/// Make `ch` a dispatch macro character with an error-signalling default.
///
/// # Errors
/// Returns an object-layer failure when the table is malformed.
pub fn make_dispatch_macro_character(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    readtable: Readtable,
    ch: char,
) -> Result<(), ReadError> {
    if ch != '#' {
        return Err(ReadError::NotDispatchMacro(ch));
    }
    let entry = make_cons(ctx, runtime, Word::NIL, Word::NIL)?;
    for sub in 0..TABLE_SIZE {
        let slot = simple_vector_ref(ctx, readtable.dispatch_table(ctx)?, sub)?;
        if slot == Word::NIL {
            simple_vector_set(ctx, readtable.dispatch_table(ctx)?, sub, entry)?;
        }
    }
    Ok(())
}

/// Copy the syntax of `from` in `from_table` onto `to` in `to_table`.
///
/// # Errors
/// Returns an object-layer failure when a table is malformed.
pub fn set_syntax_from_char(
    ctx: &mut ThreadContext,
    to: char,
    from: char,
    to_table: Readtable,
    from_table: Readtable,
) -> Result<(), ReadError> {
    let Some(to_index) = table_index(to) else {
        return Ok(());
    };
    let entry = if let Some(from_index) = table_index(from) {
        simple_vector_ref(ctx, from_table.syntax_table(ctx)?, from_index)?
    } else {
        Word::fixnum(SYNTAX_CONSTITUENT)
    };
    simple_vector_set(ctx, to_table.syntax_table(ctx)?, to_index, entry)?;
    Ok(())
}

/// Return the readtable case mode of `readtable`.
///
/// # Errors
/// Returns a malformed-case error.
pub fn readtable_case(
    ctx: &ThreadContext,
    readtable: Readtable,
) -> Result<ReadtableCase, ReadError> {
    readtable.case_mode(ctx)
}

/// Whether `word` is a readtable object.
#[must_use]
pub fn readtablep(ctx: &ThreadContext, word: Word) -> bool {
    matches!(classify_object(ctx, word), ObjectRef::Readtable(_))
}

/// Convert a raw `Word` back into a [`Readtable`], failing for other objects.
pub fn readtable_from_word(word: Word) -> Result<Readtable, ReadError> {
    if word == Word::NIL {
        return Err(ReadError::Object(ObjectError::TypeError));
    }
    Ok(Readtable::from_object(ObjectReadtable::from_word(word)))
}
