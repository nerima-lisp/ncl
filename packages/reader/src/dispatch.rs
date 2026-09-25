//! Dispatch macro character (`#`) handling.

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, Runtime, ThreadContext, Word, car, cdr, make_complex, make_cons,
    make_simple_vector, make_specialized_array, make_string, make_symbol, pop_root, push_root,
};

use crate::error::ReadError;
use crate::features::eval_feature_expr;
use crate::input::CharSource;
use crate::number::{digit_value, parse_integer_chars};
use crate::reader::{ReadOptions, read_form, read_list};
use crate::readtable::readtable_from_word;
use crate::token::{intern_common_lisp, read_token_chars};

/// Handle the `#` dispatch macro character, returning the sub-form or `None`
/// when nothing was produced (a block comment).
pub fn read_sharp(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
    opts: &ReadOptions,
    rt: &Word,
    labels: &mut Word,
) -> Result<Option<Word>, ReadError> {
    let Some(sub) = source.read_char() else {
        return Err(ReadError::UnexpectedEof);
    };
    match sub {
        '\'' => {
            let form = read_form(ctx, runtime, source, opts, rt, labels)?;
            let Some(form) = form else {
                return Err(ReadError::UnexpectedEof);
            };
            let function = intern_common_lisp(ctx, runtime, "FUNCTION")?;
            let cell = make_cons(ctx, runtime, form, Word::NIL)?;
            Ok(Some(make_cons(ctx, runtime, function, cell)?))
        }
        '(' => {
            let list = read_list(ctx, runtime, source, opts, rt, labels)?;
            Ok(Some(list_to_vector(ctx, runtime, list)?))
        }
        '*' => read_bit_vector(ctx, runtime, source).map(Some),
        ':' => read_uninterned(ctx, runtime, source, rt).map(Some),
        '.' => {
            if matches!(opts.read_evaluation(), crate::ReadEvaluation::Enabled) {
                Err(ReadError::ReadEvalUnavailable)
            } else {
                Err(ReadError::ReadEvalDisabled)
            }
        }
        '+' => read_feature_conditional(ctx, runtime, source, opts, rt, labels, true),
        '-' => read_feature_conditional(ctx, runtime, source, opts, rt, labels, false),
        '|' => {
            read_block_comment(source)?;
            Ok(None)
        }
        '\\' => read_character(source).map(Some),
        'b' | 'B' => read_radix(ctx, runtime, source, 2).map(Some),
        'o' | 'O' => read_radix(ctx, runtime, source, 8).map(Some),
        'x' | 'X' => read_radix(ctx, runtime, source, 16).map(Some),
        'd' | 'D' => read_radix(ctx, runtime, source, 10).map(Some),
        'c' | 'C' => read_complex(ctx, runtime, source, opts, rt, labels).map(Some),
        'a' | 'A' => Err(ReadError::ArraySyntax),
        's' | 'S' => Err(ReadError::StructureSyntax),
        'p' | 'P' => Err(ReadError::PathnameSyntax),
        '0'..='9' => {
            source.unread_char(sub);
            let number = read_label_number(source)?;
            match source.peek_char() {
                Some('=') => {
                    source.read_char();
                    read_label(ctx, runtime, source, opts, rt, labels, number)
                }
                Some('#') => {
                    source.read_char();
                    read_label_ref(ctx, labels, number)
                }
                Some('r' | 'R') => {
                    source.read_char();
                    let base = u32::try_from(number)
                        .map_err(|_| ReadError::InvalidNumber("radix out of range".to_owned()))?;
                    read_radix(ctx, runtime, source, base).map(Some)
                }
                _ => Err(ReadError::InvalidNumber(
                    "expected '=', '#', or 'r' after a radix or label number".to_owned(),
                )),
            }
        }
        other => Err(ReadError::UndefinedDispatchMacro(other)),
    }
}

/// Convert a proper list into a simple vector.
fn list_to_vector(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    list: Word,
) -> Result<Word, ReadError> {
    let mut elements: Vec<Word> = Vec::new();
    let mut cursor = list;
    while cursor != Word::NIL {
        elements.push(car(ctx, cursor)?);
        cursor = cdr(ctx, cursor)?;
    }
    Ok(make_simple_vector(ctx, runtime, &elements)?)
}

/// Read a `#*` bit vector literal.
fn read_bit_vector(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
) -> Result<Word, ReadError> {
    let mut bits: Vec<Word> = Vec::new();
    while let Some(c) = source.peek_char() {
        match c {
            '0' => {
                bits.push(Word::fixnum(0));
                source.read_char();
            }
            '1' => {
                bits.push(Word::fixnum(1));
                source.read_char();
            }
            _ => break,
        }
    }
    Ok(make_specialized_array(
        ctx,
        runtime,
        ArrayElementType::Bit,
        &bits,
    )?)
}

/// Read a `#:` uninterned symbol literal.
fn read_uninterned(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
    rt: &Word,
) -> Result<Word, ReadError> {
    let Some(token) = read_token_chars(ctx, source, rt)? else {
        return Err(ReadError::UnexpectedEof);
    };
    let case = readtable_from_word(*rt)?.case_mode(ctx)?;
    let range = 0..token.characters().len();
    let name = token.fold_name(&range, case);
    let name_word = make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
    Ok(make_symbol(ctx, runtime, name_word)?)
}

/// Read a `#+` or `#-` conditional, producing the form it selects.
fn read_feature_conditional(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
    opts: &ReadOptions,
    rt: &Word,
    labels: &mut Word,
    positive: bool,
) -> Result<Option<Word>, ReadError> {
    let features = runtime.features();
    let Some(expression) = read_form(ctx, runtime, source, opts, rt, labels)? else {
        return Err(ReadError::InvalidFeatureExpression);
    };
    let present = eval_feature_expr(ctx, runtime, expression, &features)?;
    if present != positive {
        let _ = read_form(ctx, runtime, source, opts, rt, labels)?;
    }
    read_form(ctx, runtime, source, opts, rt, labels)
}

/// Skip a (possibly nested) `#| ... |#` block comment.
fn read_block_comment(source: &mut dyn CharSource) -> Result<(), ReadError> {
    let mut depth = 1_u32;
    while depth > 0 {
        let Some(c) = source.read_char() else {
            return Err(ReadError::UnexpectedEof);
        };
        if c == '#' && source.peek_char() == Some('|') {
            source.read_char();
            depth += 1;
        } else if c == '|' && source.peek_char() == Some('#') {
            source.read_char();
            depth -= 1;
        }
    }
    Ok(())
}

/// Read a `#\` character literal.
fn read_character(source: &mut dyn CharSource) -> Result<Word, ReadError> {
    let Some(first) = source.read_char() else {
        return Err(ReadError::InvalidCharacter);
    };
    if !first.is_alphabetic() {
        return Ok(Word::character(u32::from(first)));
    }
    let mut name = String::new();
    name.push(first);
    while let Some(c) = source.peek_char() {
        if c.is_alphanumeric() || c == '-' {
            name.push(c);
            source.read_char();
        } else {
            break;
        }
    }
    let code = match name.to_ascii_uppercase().as_str() {
        "SPACE" => 0x20,
        "NEWLINE" | "LINEFEED" => 0x0A,
        "TAB" => 0x09,
        "RETURN" => 0x0D,
        "PAGE" => 0x0C,
        "BACKSPACE" => 0x08,
        "RUBOUT" | "DELETE" => 0x7F,
        "NULL" => 0x00,
        "ESCAPE" | "ALTMODE" => 0x1B,
        _ => {
            let mut chars = name.chars();
            match (chars.next(), chars.next()) {
                (Some(single), None) => u32::from(single),
                _ => return Err(ReadError::UnknownCharacterName(name)),
            }
        }
    };
    Ok(Word::character(code))
}

/// Read an integer token in a fixed radix (used by `#x`, `#b`, `#o`, `#nR`).
fn read_radix(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
    base: u32,
) -> Result<Word, ReadError> {
    if !(2..=36).contains(&base) {
        return Err(ReadError::InvalidBase(base));
    }
    let mut chars: Vec<char> = Vec::new();
    if let Some(c) = source.peek_char()
        && (c == '+' || c == '-')
    {
        chars.push(c);
        source.read_char();
    }
    while let Some(c) = source.peek_char() {
        if digit_value(c, base).is_some() {
            chars.push(c);
            source.read_char();
        } else {
            break;
        }
    }
    parse_integer_chars(ctx, runtime, &chars, base)
}

/// Read a `#c(real imag)` complex literal.
fn read_complex(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
    opts: &ReadOptions,
    rt: &Word,
    labels: &mut Word,
) -> Result<Word, ReadError> {
    if source.read_char() != Some('(') {
        return Err(ReadError::InvalidNumber(
            "expected '(' in complex literal".to_owned(),
        ));
    }
    let list = read_list(ctx, runtime, source, opts, rt, labels)?;
    let real = car(ctx, list)?;
    let rest = cdr(ctx, list)?;
    let imag = if rest == Word::NIL {
        Word::fixnum(0)
    } else {
        car(ctx, rest)?
    };
    Ok(make_complex(ctx, runtime, real, imag)?.into())
}

/// Read a `#n=` label definition and store the form for `#n#` references.
fn read_label(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
    opts: &ReadOptions,
    rt: &Word,
    labels: &mut Word,
    label: i64,
) -> Result<Option<Word>, ReadError> {
    let form = read_form(ctx, runtime, source, opts, rt, labels)?;
    let Some(form) = form else {
        return Err(ReadError::UnexpectedEof);
    };
    let table = ensure_labels_table(ctx, runtime, labels)?;
    let mut form = form;
    let token = push_root(ctx, &mut form);
    let result = HashTable::from(table).insert(ctx, runtime, Word::fixnum(label), form);
    let _ = pop_root(ctx, token);
    result?;
    Ok(Some(form))
}

/// Read a `#n#` reference to a previously read labelled form.
fn read_label_ref(
    ctx: &mut ThreadContext,
    labels: &Word,
    label: i64,
) -> Result<Option<Word>, ReadError> {
    if *labels == Word::NIL {
        return Err(ReadError::InvalidNumber(format!(
            "undefined label #{label}"
        )));
    }
    let mut table = *labels;
    let token = push_root(ctx, &mut table);
    let value = HashTable::from(table).get(ctx, Word::fixnum(label))?;
    let _ = pop_root(ctx, token);
    value
        .map(Some)
        .ok_or_else(|| ReadError::InvalidNumber(format!("undefined label #{label}")))
}

/// The labels hash table, allocated on first use and rooted by the caller.
fn ensure_labels_table(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    labels: &mut Word,
) -> Result<Word, ReadError> {
    if *labels == Word::NIL {
        let table = HashTable::new(ctx, runtime, HashTest::Eq, Weakness::None)?.as_word();
        *labels = table;
    }
    Ok(*labels)
}

/// Read an unsigned decimal label number.
fn read_label_number(source: &mut dyn CharSource) -> Result<i64, ReadError> {
    let mut digits = String::new();
    while let Some(c) = source.peek_char() {
        if c.is_ascii_digit() {
            digits.push(c);
            source.read_char();
        } else {
            break;
        }
    }
    if digits.is_empty() {
        return Err(ReadError::InvalidNumber("missing label number".to_owned()));
    }
    digits
        .parse::<i64>()
        .map_err(|_| ReadError::NumberOutOfRange)
}
