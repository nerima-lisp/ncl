use super::*;

pub(crate) fn write_string_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let string = args.required(0)?;
    let stream = stream_or_default(ctx, runtime, args, 1, "*STANDARD-OUTPUT*")?;
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
    let stream = stream_or_default(ctx, runtime, args, 1, "*STANDARD-OUTPUT*")?;
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
    let stream = stream_or_default(ctx, runtime, args, 0, "*STANDARD-OUTPUT*")?;
    write_to_stream(ctx, runtime, stream, '\n')?;
    Ok(Word::NIL)
}

pub(crate) fn fresh_line_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = stream_or_default(ctx, runtime, args, 0, "*STANDARD-OUTPUT*")?;
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
