use super::{
    BuiltinArgs, MultipleValues, ObjectError, ObjectRef, Package, Runtime, STRING_INPUT,
    STRING_OUTPUT, Stream, StreamKind, ThreadContext, Word, car, cdr, classify_object,
    make_simple_vector, make_stream, make_string, peek_character, simple_vector_ref,
    simple_vector_set, state_kind, stream_from_args, stream_or_default, string_length,
    string_output_at_line_start, string_ref, with_root, write_to_stream,
};
use ncl_object::{stream_state, with_roots};

fn keyword_name(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    let ObjectRef::Symbol(symbol) = classify_object(ctx, word) else {
        return Err(ObjectError::TypeError);
    };
    let name = ncl_object::symbol_name(ctx, symbol)?;
    (0..ncl_object::string_length(ctx, name)?)
        .map(|index| ncl_object::string_ref(ctx, name, index))
        .collect()
}

fn bounds(
    ctx: &ThreadContext,
    args: &BuiltinArgs<'_>,
    first_option: usize,
    length: usize,
) -> Result<(usize, usize), ObjectError> {
    let mut start = 0;
    let mut end = length;
    let mut index = first_option;
    if let Some(value) = args.get(index).and_then(Word::as_fixnum) {
        start = usize::try_from(value).map_err(|_| ObjectError::TypeError)?;
        end = args
            .get(index + 1)
            .and_then(Word::as_fixnum)
            .map(|value| usize::try_from(value).map_err(|_| ObjectError::TypeError))
            .transpose()?
            .unwrap_or(length);
        if args.len() > index + 2 {
            return Err(ObjectError::TypeError);
        }
        return if start > end || end > length {
            Err(ObjectError::TypeError)
        } else {
            Ok((start, end))
        };
    }
    while index < args.len() {
        let name = keyword_name(ctx, args.get(index).ok_or(ObjectError::Layout)?)?;
        let value = args.get(index + 1).ok_or(ObjectError::TypeError)?;
        let number = usize::try_from(value.as_fixnum().ok_or(ObjectError::TypeError)?)
            .map_err(|_| ObjectError::TypeError)?;
        match name.as_str() {
            "START" => start = number,
            "END" => end = number,
            _ => return Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) reject unknown keyword
        }
        index += 2;
    }
    if start > end || end > length {
        return Err(ObjectError::TypeError);
    }
    Ok((start, end))
}

pub fn write_string_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let string = args.required(0)?;
    let stream = stream_or_default(ctx, runtime, args, 1, "*STANDARD-OUTPUT*")?;
    let (start, end) = bounds(ctx, args, 2, string_length(ctx, string)?)?;
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

pub fn write_line_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let string = args.required(0)?;
    let stream = stream_or_default(ctx, runtime, args, 1, "*STANDARD-OUTPUT*")?;
    let (start, end) = bounds(ctx, args, 2, string_length(ctx, string)?)?;
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

pub fn terpri_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = stream_or_default(ctx, runtime, args, 0, "*STANDARD-OUTPUT*")?;
    write_to_stream(ctx, runtime, stream, '\n')?;
    Ok(Word::NIL)
}

pub fn fresh_line_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = stream_or_default(ctx, runtime, args, 0, "*STANDARD-OUTPUT*")?;
    let at_line_start = if state_kind(ctx, stream_state(ctx, stream)?)? == StreamKind::StringOutput
    {
        string_output_at_line_start(ctx, stream)?
    } else {
        peek_character(ctx, stream)? == Some('\n')
    };
    if at_line_start {
        return Ok(Word::NIL);
    }
    write_to_stream(ctx, runtime, stream, '\n')?;
    Ok(Word::TRUE)
}

pub fn make_string_input_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let string = args.required(0)?;
    let length = string_length(ctx, string)?;
    let (start, end) = bounds(ctx, args, 1, length)?;
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

pub fn make_string_output_adapter(
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

pub fn get_output_stream_string_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = stream_from_args(args, 0)?;
    let state = stream_state(ctx, stream)?;
    if state_kind(ctx, state)? != StreamKind::StringOutput {
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
