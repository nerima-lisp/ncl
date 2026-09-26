use super::{CLOSED, DATA, POSITION, STRING_INPUT, STRING_OUTPUT};

use ncl_object::{
    BuiltinArgs, MultipleValues, ObjectError, ObjectRef, Package, Runtime, Stream, ThreadContext,
    Word, car, cdr, classify_object, make_cons, make_simple_vector, make_stream, make_string,
    pop_root, push_root, simple_vector_length, simple_vector_ref, simple_vector_set, stream_state,
    string_length, string_ref, with_root, with_roots,
};

pub(crate) fn pass_arguments(args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    (0..args.len())
        .map(|index| args.get(index).ok_or(ObjectError::TypeError))
        .collect()
}

pub(crate) fn stream_from_args(
    args: &BuiltinArgs<'_>,
    index: usize,
) -> Result<Stream, ObjectError> {
    Ok(Stream::from_word(args.required(index)?))
}

pub(crate) fn state_kind(ctx: &ThreadContext, state: Word) -> Result<Option<i64>, ObjectError> {
    let first = simple_vector_ref(ctx, state, 0)?.as_fixnum();
    Ok(first.filter(|value| *value == STRING_INPUT || *value == STRING_OUTPUT))
}

pub(crate) fn ensure_open(ctx: &ThreadContext, state: Word) -> Result<(), ObjectError> {
    if simple_vector_ref(ctx, state, 0)?.as_fixnum() == Some(CLOSED) {
        return Err(ObjectError::TypeError);
    }
    Ok(())
}

pub(crate) fn position(
    ctx: &ThreadContext,
    state: Word,
    index: usize,
) -> Result<usize, ObjectError> {
    usize::try_from(
        simple_vector_ref(ctx, state, index)?
            .as_fixnum()
            .ok_or(ObjectError::Layout)?,
    )
    .map_err(|_| ObjectError::Layout)
}

pub(crate) fn set_position(
    ctx: &mut ThreadContext,
    state: Word,
    index: usize,
    value: usize,
) -> Result<(), ObjectError> {
    simple_vector_set(
        ctx,
        state,
        index,
        Word::fixnum(i64::try_from(value).map_err(|_| ObjectError::Layout)?),
    )
}

pub(crate) fn next_character(
    ctx: &mut ThreadContext,
    stream: Stream,
) -> Result<Option<char>, ObjectError> {
    let state = stream_state(ctx, stream)?;
    ensure_open(ctx, state)?;
    if state_kind(ctx, state)?.is_some() {
        let pos = position(ctx, state, 1)?;
        let string = simple_vector_ref(ctx, state, 2)?;
        let length = string_length(ctx, string)?;
        let end = if simple_vector_length(ctx, state)? > 3 {
            position(ctx, state, 3)?
        } else {
            length
        };
        if pos >= end || pos >= length {
            return Ok(None);
        }
        let character = string_ref(ctx, string, pos)?;
        set_position(ctx, state, 1, pos.saturating_add(1))?;
        return Ok(Some(character));
    }
    let pos = position(ctx, state, POSITION)?;
    let length = simple_vector_length(ctx, state)?.saturating_sub(DATA);
    if pos >= length {
        return Ok(None);
    }
    let byte = simple_vector_ref(ctx, state, DATA + pos)?
        .as_fixnum()
        .ok_or(ObjectError::Layout)?;
    let character = char::from_u32(u32::try_from(byte).map_err(|_| ObjectError::Layout)?)
        .ok_or(ObjectError::Layout)?;
    set_position(ctx, state, POSITION, pos.saturating_add(1))?;
    Ok(Some(character))
}

pub(crate) fn peek_character(
    ctx: &mut ThreadContext,
    stream: Stream,
) -> Result<Option<char>, ObjectError> {
    let state = stream_state(ctx, stream)?;
    let pos_index = if state_kind(ctx, state)?.is_some() {
        1
    } else {
        POSITION
    };
    let pos = position(ctx, state, pos_index)?;
    let result = next_character(ctx, stream)?;
    set_position(ctx, state, pos_index, pos)?;
    Ok(result)
}

pub(crate) fn read_char_adapter(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = stream_from_args(args, 0)?;
    next_character(ctx, stream)?.map_or_else(
        || Ok(args.get(2).unwrap_or(Word::NIL)),
        |character| Ok(Word::character(u32::from(character))),
    )
}

pub(crate) fn unread_char_adapter(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = stream_from_args(args, 1)?;
    let state = stream_state(ctx, stream)?;
    let index = if state_kind(ctx, state)?.is_some() {
        1
    } else {
        POSITION
    };
    let pos = position(ctx, state, index)?;
    if pos == 0 {
        return Err(ObjectError::TypeError);
    }
    set_position(ctx, state, index, pos - 1)?;
    args.required(0)
}

pub(crate) fn peek_char_adapter(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream_index = usize::from(args.len() > 1);
    peek_character(ctx, stream_from_args(args, stream_index)?)?.map_or_else(
        || Ok(args.get(3).unwrap_or(Word::NIL)),
        |character| Ok(Word::character(u32::from(character))),
    )
}

pub(crate) fn read_line_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = stream_from_args(args, 0)?;
    let mut characters = Vec::new();
    let mut ended = false;
    while let Some(character) = next_character(ctx, stream)? {
        if character == '\n' {
            ended = true;
            break;
        }
        characters.push(character);
    }
    let line = make_string(ctx, runtime, &characters)?;
    values.set(&[line, if ended { Word::NIL } else { Word::TRUE }]);
    Ok(line)
}

pub(crate) fn write_to_stream(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    stream: Stream,
    character: char,
) -> Result<(), ObjectError> {
    let state = stream_state(ctx, stream)?;
    ensure_open(ctx, state)?;
    match state_kind(ctx, state)? {
        Some(STRING_OUTPUT) => {
            let mut state = state;
            let token = push_root(ctx, &mut state);
            let result = (|| {
                let count = position(ctx, state, 1)?;
                let list = simple_vector_ref(ctx, state, 2)?;
                let next = make_cons(ctx, runtime, Word::character(u32::from(character)), list)?;
                simple_vector_set(ctx, state, 2, next)?;
                set_position(ctx, state, 1, count.saturating_add(1))
            })();
            if !pop_root(ctx, token) {
                return Err(ObjectError::Layout);
            }
            result
        }
        Some(STRING_INPUT) => Err(ObjectError::TypeError),
        None => {
            let pos = position(ctx, state, POSITION)?;
            if DATA + pos >= simple_vector_length(ctx, state)? {
                return Err(ObjectError::TypeError);
            }
            simple_vector_set(
                ctx,
                state,
                DATA + pos,
                Word::fixnum(i64::from(u32::from(character))),
            )?;
            set_position(ctx, state, POSITION, pos.saturating_add(1))
        }
        Some(_) => Err(ObjectError::Layout),
    }
}

pub(crate) fn write_char_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let character = if let ObjectRef::Character(code) = classify_object(ctx, args.required(0)?) {
        char::from_u32(code).ok_or(ObjectError::TypeError)?
    } else {
        return Err(ObjectError::TypeError);
    };
    write_to_stream(ctx, runtime, stream_from_args(args, 1)?, character)?;
    args.required(0)
}

pub(crate) fn read_byte_adapter(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = stream_from_args(args, 0)?;
    let state = stream_state(ctx, stream)?;
    ensure_open(ctx, state)?;
    if state_kind(ctx, state)?.is_some() {
        return Err(ObjectError::TypeError);
    }
    let position = position(ctx, state, POSITION)?;
    let length = simple_vector_length(ctx, state)?.saturating_sub(DATA);
    if position >= length {
        return Ok(args.get(2).unwrap_or(Word::NIL));
    }
    let byte = simple_vector_ref(ctx, state, DATA + position)?
        .as_fixnum()
        .ok_or(ObjectError::Layout)?;
    set_position(ctx, state, POSITION, position.saturating_add(1))?;
    Ok(Word::fixnum(byte))
}

pub(crate) fn write_string_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let string = args.required(0)?;
    let stream = stream_from_args(args, 1)?;
    let start = args
        .get(2)
        .and_then(Word::as_fixnum)
        .map(|value| usize::try_from(value).map_err(|_| ObjectError::TypeError))
        .transpose()?
        .unwrap_or(0);
    let end = args
        .get(3)
        .and_then(Word::as_fixnum)
        .map(|value| usize::try_from(value).map_err(|_| ObjectError::TypeError))
        .transpose()?
        .unwrap_or(string_length(ctx, string)?);
    if start > end || end > string_length(ctx, string)? {
        return Err(ObjectError::TypeError);
    }
    with_roots(ctx, &[string, stream.into()], |ctx, roots| {
        for index in start..end {
            let string = roots.first().ok_or(ObjectError::Layout)?;
            let stream = roots.get(1).ok_or(ObjectError::Layout)?;
            let character = string_ref(ctx, **string, index)?;
            write_to_stream(ctx, runtime, Stream::from_word(**stream), character)?;
        }
        roots.first().map(|root| **root).ok_or(ObjectError::Layout)
    })
}

pub(crate) fn write_line_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let string = args.required(0)?;
    let stream = stream_from_args(args, 1)?;
    let start = args
        .get(2)
        .and_then(Word::as_fixnum)
        .map(|value| usize::try_from(value).map_err(|_| ObjectError::TypeError))
        .transpose()?
        .unwrap_or(0);
    let end = args
        .get(3)
        .and_then(Word::as_fixnum)
        .map(|value| usize::try_from(value).map_err(|_| ObjectError::TypeError))
        .transpose()?
        .unwrap_or(string_length(ctx, string)?);
    if start > end || end > string_length(ctx, string)? {
        return Err(ObjectError::TypeError);
    }
    with_roots(ctx, &[string, stream.into()], |ctx, roots| {
        for index in start..end {
            let string = roots.first().ok_or(ObjectError::Layout)?;
            let stream = roots.get(1).ok_or(ObjectError::Layout)?;
            let character = string_ref(ctx, **string, index)?;
            write_to_stream(ctx, runtime, Stream::from_word(**stream), character)?;
        }
        let stream = roots.get(1).ok_or(ObjectError::Layout)?;
        write_to_stream(ctx, runtime, Stream::from_word(**stream), '\n')?;
        roots.first().map(|root| **root).ok_or(ObjectError::Layout)
    })
}

pub(crate) fn terpri_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    write_to_stream(ctx, runtime, stream_from_args(args, 0)?, '\n')?;
    Ok(Word::NIL)
}

pub(crate) fn fresh_line_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = stream_from_args(args, 0)?;
    if peek_character(ctx, stream)? == Some('\n') {
        return Ok(Word::NIL);
    }
    write_to_stream(ctx, runtime, stream, '\n')?;
    Ok(Word::TRUE)
}

pub(crate) fn make_string_input_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let string = args.required(0)?;
    let length = string_length(ctx, string)?;
    let start = args
        .get(1)
        .and_then(Word::as_fixnum)
        .map(|value| usize::try_from(value).map_err(|_| ObjectError::TypeError))
        .transpose()?
        .unwrap_or(0);
    let end = args
        .get(2)
        .and_then(Word::as_fixnum)
        .map(|value| usize::try_from(value).map_err(|_| ObjectError::TypeError))
        .transpose()?
        .unwrap_or(length);
    if start > end || end > length {
        return Err(ObjectError::TypeError);
    }
    with_root(ctx, &mut string.clone(), |ctx, string| {
        let state = make_simple_vector(
            ctx,
            runtime,
            &[
                Word::fixnum(STRING_INPUT),
                Word::fixnum(i64::try_from(start).map_err(|_| ObjectError::Layout)?),
                *string,
                Word::fixnum(i64::try_from(end).map_err(|_| ObjectError::Layout)?),
            ],
        )?;
        Ok(make_stream(
            ctx,
            runtime,
            Word::NIL,
            Word::NIL,
            Word::NIL,
            state,
            Word::NIL,
        )?
        .into())
    })
}

pub(crate) fn make_string_output_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    _args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let package = runtime.ensure_package(ctx, "COMMON-LISP")?;
    let (mut direction, _) = Package::from_word(package).intern(ctx, runtime, "OUTPUT")?;
    with_root(ctx, &mut direction, |ctx, direction| {
        let state = make_simple_vector(
            ctx,
            runtime,
            &[Word::fixnum(STRING_OUTPUT), Word::fixnum(0), Word::NIL],
        )?;
        Ok(make_stream(
            ctx,
            runtime,
            *direction,
            Word::NIL,
            Word::NIL,
            state,
            Word::NIL,
        )?
        .into())
    })
}

pub(crate) fn get_output_stream_string_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = stream_from_args(args, 0)?;
    let state = stream_state(ctx, stream)?;
    if state_kind(ctx, state)? != Some(STRING_OUTPUT) {
        return Err(ObjectError::TypeError);
    }
    let mut list = simple_vector_ref(ctx, state, 2)?;
    let mut characters = Vec::new();
    while list != Word::NIL {
        let value = car(ctx, list)?;
        let character = if let ObjectRef::Character(code) = classify_object(ctx, value) {
            char::from_u32(code).ok_or(ObjectError::Layout)?
        } else {
            return Err(ObjectError::Layout);
        };
        characters.push(character);
        list = cdr(ctx, list)?;
    }
    characters.reverse();
    simple_vector_set(ctx, state, 1, Word::fixnum(0))?;
    simple_vector_set(ctx, state, 2, Word::NIL)?;
    make_string(ctx, runtime, &characters)
}
