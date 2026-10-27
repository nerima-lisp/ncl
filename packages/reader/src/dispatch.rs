//! Dispatch macro character (`#`) handling.

mod read_eval;

use std::cell::Cell;
use std::path::Path;

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, HeapRootedWord, HeapRootedWords, Local, ObjectRef, Runtime, Scope,
    ThreadContext, Word, car, cdr, classify_object, make_complex, make_cons, make_simple_vector,
    make_specialized_array, make_string, make_structure, make_symbol, pop_root, push_root,
    simple_vector_length, simple_vector_ref, string_length, string_ref, symbol_name,
};

use crate::error::ReadError;
use crate::features::eval_feature_expr;
use crate::input::CharSource;
use crate::number::{digit_value, parse_integer_chars};
use crate::reader::{ReadOptions, ReadSuppression, read_form, read_list};
use crate::readtable::readtable_from_word;
use crate::token::{intern_common_lisp, read_token_chars, refresh_dynamic_reader_options};

pub fn read_sharp(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
    opts: &ReadOptions,
    rt: &ncl_object::HeapRootedWord,
    labels: &ncl_object::HeapRootedWord,
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
            let form = HeapRootedWord::new(ctx, form)?;
            let function = intern_common_lisp(ctx, runtime, "FUNCTION")?;
            let function = HeapRootedWord::new(ctx, function)?;
            let cell = make_cons(ctx, runtime, form.get(), Word::NIL)?;
            let cell = HeapRootedWord::new(ctx, cell)?;
            Ok(Some(make_cons(ctx, runtime, function.get(), cell.get())?))
        }
        '(' => {
            let list = read_list(ctx, runtime, source, opts, rt, labels)?;
            Ok(Some(list_to_vector(ctx, runtime, list)?))
        }
        '*' => read_bit_vector(ctx, runtime, source).map(Some),
        ':' => read_uninterned(ctx, runtime, source, rt).map(Some),
        '.' => read_eval::read_eval_dispatch(ctx, runtime, source, opts, rt, labels).map(Some),
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
        's' | 'S' => read_structure(ctx, runtime, source, opts, rt, labels).map(Some),
        'p' | 'P' => read_pathname(ctx, runtime, source, opts, rt, labels).map(Some),
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

/// Read a `#P` pathname literal from its namestring.
fn read_pathname(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
    opts: &ReadOptions,
    rt: &ncl_object::HeapRootedWord,
    labels: &ncl_object::HeapRootedWord,
) -> Result<Word, ReadError> {
    let Some(namestring) = read_form(ctx, runtime, source, opts, rt, labels)? else {
        return Err(ReadError::UnexpectedEof);
    };
    if !matches!(classify_object(ctx, namestring), ObjectRef::String(_)) {
        return Err(ReadError::PathnameSyntax);
    }
    let namestring = (0..string_length(ctx, namestring)?)
        .map(|index| string_ref(ctx, namestring, index))
        .collect::<Result<String, _>>()?;
    ncl_lib_pathnames::from_host_path(ctx, runtime, Path::new(&namestring)).map_err(Into::into)
}

/// Read a `#S(name :slot value ...)` structure literal.
fn read_structure(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
    opts: &ReadOptions,
    rt: &ncl_object::HeapRootedWord,
    labels: &ncl_object::HeapRootedWord,
) -> Result<Word, ReadError> {
    if source.read_char() != Some('(') {
        return Err(ReadError::StructureSyntax);
    }
    let form = read_list(ctx, runtime, source, opts, rt, labels)?;
    let mut scope = Scope::new(ctx);
    let form: ncl_object::Handle<'_, Word> = scope.root(Local::from_word(form));
    let fields = scope
        .list_to_handle_vec(Local::from_word(scope.get(form).as_word()))
        .map_err(|_| ReadError::StructureSyntax)?;
    let name = *fields
        .as_slice()
        .first()
        .ok_or(ReadError::StructureSyntax)?;
    let name_word = scope.get(name).as_word();
    if !matches!(
        classify_object(scope.context(), name_word),
        ObjectRef::Symbol(_)
    ) {
        return Err(ReadError::StructureSyntax);
    }
    let layout = runtime
        .structure_layout_for_symbol(scope.context(), name_word)
        .ok_or(ReadError::StructureSyntax)?;
    let class_word = runtime
        .structure_class(scope.context_mut(), layout)
        .ok_or(ReadError::StructureSyntax)?;
    let class: ncl_object::Handle<'_, Word> = scope.root(Local::from_word(class_word));
    let slots = simple_vector_ref(scope.context(), scope.get(class).as_word(), 4)?;
    let slot_count = simple_vector_length(scope.context(), slots)?;
    let values = HeapRootedWords::new(scope.context(), &vec![Word::NIL; slot_count])?;
    let mut seen = vec![false; slot_count];
    let mut index = 1;
    while index < fields.len() {
        let key = *fields
            .as_slice()
            .get(index)
            .ok_or(ReadError::StructureSyntax)?;
        let value = *fields
            .as_slice()
            .get(index + 1)
            .ok_or(ReadError::StructureSyntax)?;
        let key_word = scope.get(key).as_word();
        if !matches!(
            classify_object(scope.context(), key_word),
            ObjectRef::Symbol(_)
        ) {
            return Err(ReadError::StructureSyntax);
        }
        let key_name = symbol_text(scope.context(), key_word)?;
        let key_name = key_name.strip_prefix(':').unwrap_or(&key_name);
        let mut found = None;
        for slot_index in 0..slot_count {
            let descriptor = simple_vector_ref(scope.context(), slots, slot_index)?;
            let slot_name = simple_vector_ref(scope.context(), descriptor, 0)?;
            if symbol_text(scope.context(), slot_name)? == key_name {
                found = Some(slot_index);
                break;
            }
        }
        let slot_index = found.ok_or(ReadError::StructureSyntax)?;
        if *seen.get(slot_index).ok_or(ReadError::StructureSyntax)? {
            return Err(ReadError::StructureSyntax);
        }
        *seen.get_mut(slot_index).ok_or(ReadError::StructureSyntax)? = true;
        values
            .get(slot_index)
            .ok_or(ReadError::StructureSyntax)?
            .set(scope.get(value).as_word());
        index += 2;
    }
    let values = values.iter().map(HeapRootedWord::get).collect::<Vec<_>>();
    Ok(make_structure(
        scope.context_mut(),
        runtime,
        layout,
        &values,
    )?)
}

fn symbol_text(ctx: &ThreadContext, symbol: Word) -> Result<String, ReadError> {
    let name = symbol_name(ctx, symbol)?;
    let length = string_length(ctx, name)?;
    Ok((0..length)
        .map(|index| string_ref(ctx, name, index))
        .collect::<Result<String, _>>()?)
}

/// Convert a proper list into a simple vector.
fn list_to_vector(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    list: Word,
) -> Result<Word, ReadError> {
    let cursor = HeapRootedWord::new(ctx, list)?;
    let mut elements = HeapRootedWords::default();
    while cursor.get() != Word::NIL {
        let element = car(ctx, cursor.get())?;
        elements.push(ctx, element)?;
        cursor.set(cdr(ctx, cursor.get())?);
    }
    let elements = elements.iter().map(HeapRootedWord::get).collect::<Vec<_>>();
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
    rt: &ncl_object::HeapRootedWord,
) -> Result<Word, ReadError> {
    let Some(token) = read_token_chars(ctx, source, rt)? else {
        return Err(ReadError::UnexpectedEof);
    };
    let case = readtable_from_word(rt.get())?.case_mode(ctx)?;
    let range = 0..token.characters().len();
    let name = token.fold_name(&range, case);
    let name_word = make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
    let name_word = HeapRootedWord::new(ctx, name_word)?;
    Ok(make_symbol(ctx, runtime, name_word.get())?)
}

/// Read a `#+` or `#-` conditional, producing the form it selects.
fn read_feature_conditional(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
    opts: &ReadOptions,
    rt: &ncl_object::HeapRootedWord,
    labels: &ncl_object::HeapRootedWord,
    positive: bool,
) -> Result<Option<Word>, ReadError> {
    let features = runtime.features();
    let mut feature_options = opts.clone();
    feature_options.set_read_suppression(ReadSuppression::Keep);
    let Some(expression) = read_form(ctx, runtime, source, &feature_options, rt, labels)? else {
        return Err(ReadError::InvalidFeatureExpression);
    };
    refresh_dynamic_reader_options(ctx, runtime, opts)?;
    let present = eval_feature_expr(ctx, runtime, expression, &features)?;
    if present != positive {
        let mut suppressed = opts.clone();
        suppressed.set_read_suppression(ReadSuppression::Discard);
        if read_form(ctx, runtime, source, &suppressed, rt, labels)?.is_none() {
            return Err(ReadError::UnexpectedEof);
        }
        return Ok(None);
    }
    read_form(ctx, runtime, source, opts, rt, labels)?
        .map(Some)
        .ok_or(ReadError::UnexpectedEof)
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
    rt: &ncl_object::HeapRootedWord,
    labels: &ncl_object::HeapRootedWord,
) -> Result<Word, ReadError> {
    if source.read_char() != Some('(') {
        return Err(ReadError::InvalidNumber(
            "expected '(' in complex literal".to_owned(),
        ));
    }
    let list = read_list(ctx, runtime, source, opts, rt, labels)?;
    let real = HeapRootedWord::new(ctx, car(ctx, list)?)?;
    let rest = cdr(ctx, list)?;
    let imag = if rest == Word::NIL {
        Word::fixnum(0)
    } else {
        car(ctx, rest)?
    };
    let imag = HeapRootedWord::new(ctx, imag)?;
    Ok(make_complex(ctx, runtime, real.get(), imag.get())?.into())
}

/// Read a `#n=` label definition and store the form for `#n#` references.
fn read_label(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
    opts: &ReadOptions,
    rt: &ncl_object::HeapRootedWord,
    labels: &ncl_object::HeapRootedWord,
    label: i64,
) -> Result<Option<Word>, ReadError> {
    let form = read_form(ctx, runtime, source, opts, rt, labels)?;
    let Some(form) = form else {
        return Err(ReadError::UnexpectedEof);
    };
    let mut form = Cell::new(form);
    let token = push_root(ctx, form.get_mut());
    let table = ensure_labels_table(ctx, runtime, labels)?;
    let result = HashTable::from_word(table).insert(ctx, runtime, Word::fixnum(label), form.get());
    let _ = pop_root(ctx, token);
    result?;
    Ok(Some(form.get()))
}

/// Read a `#n#` reference to a previously read labelled form.
fn read_label_ref(
    ctx: &mut ThreadContext,
    labels: &ncl_object::HeapRootedWord,
    label: i64,
) -> Result<Option<Word>, ReadError> {
    if labels.get() == Word::NIL {
        return Err(ReadError::InvalidNumber(format!(
            "undefined label #{label}"
        )));
    }
    let mut table = labels.get();
    let token = push_root(ctx, &mut table);
    let value = HashTable::from_word(table).get(ctx, Word::fixnum(label))?;
    let _ = pop_root(ctx, token);
    value
        .map(Some)
        .ok_or_else(|| ReadError::InvalidNumber(format!("undefined label #{label}")))
}

/// The labels hash table, allocated on first use and rooted by the caller.
fn ensure_labels_table(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    labels: &ncl_object::HeapRootedWord,
) -> Result<Word, ReadError> {
    if labels.get() == Word::NIL {
        let table = HashTable::new(ctx, runtime, HashTest::Eq, Weakness::None)?.as_word();
        labels.set(table);
    }
    Ok(labels.get())
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
