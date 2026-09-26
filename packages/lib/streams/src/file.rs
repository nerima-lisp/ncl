use super::character::{ensure_open, fail, stream_from_args};
use super::{CLOSED, DATA, POSITION};

use std::fs;

use ncl_object::{
    BuiltinArgs, MultipleValues, ObjectError, ObjectRef, Runtime, Stream, ThreadContext, Word,
    classify_object, make_simple_vector, make_stream, simple_vector_length, simple_vector_ref,
    simple_vector_set, stream_direction, stream_element_type, stream_external_format, stream_state,
    string_length, string_ref, symbol_name,
};

pub(crate) fn text(ctx: &ThreadContext, value: Word) -> Result<String, ObjectError> {
    (0..string_length(ctx, value)?)
        .map(|index| string_ref(ctx, value, index))
        .collect()
}

pub(crate) fn symbol_text(ctx: &ThreadContext, value: Word) -> Result<String, ObjectError> {
    text(ctx, symbol_name(ctx, value)?)
}

pub(crate) fn option(
    ctx: &ThreadContext,
    args: &BuiltinArgs<'_>,
    keyword: &str,
) -> Result<Option<Word>, ObjectError> {
    let mut index = 1;
    while index < args.len() {
        let key = args.get(index).ok_or(ObjectError::TypeError)?;
        let value = args.get(index + 1).ok_or(ObjectError::TypeError)?;
        if symbol_text(ctx, key)?.eq_ignore_ascii_case(keyword) {
            return Ok(Some(value));
        }
        index += 2;
    }
    Ok(None)
}

pub(crate) fn open_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let path = text(ctx, args.required(0)?)?;
    let direction = match option(ctx, args, "DIRECTION")? {
        Some(word) => symbol_text(ctx, word)?,
        None => "INPUT".to_owned(),
    };
    let exists = fs::metadata(&path).is_ok();
    if !exists
        && option(ctx, args, "IF-DOES-NOT-EXIST")?
            .map(|word| symbol_text(ctx, word))
            .transpose()?
            .as_deref()
            == Some("NIL")
    {
        return Ok(Word::NIL);
    }
    let data = match direction.as_str() {
        "INPUT" => fs::read(&path).map_err(|_| ObjectError::Unsupported)?,
        "OUTPUT" => {
            let exists_policy = option(ctx, args, "IF-EXISTS")?
                .map(|word| symbol_text(ctx, word))
                .transpose()?;
            if exists && exists_policy.as_deref() == Some("ERROR") {
                return Ok(fail(ctx, ObjectError::Unsupported));
            }
            fs::File::create(&path).map_err(|_| ObjectError::Unsupported)?;
            Vec::new()
        }
        "IO" => fs::read(&path).map_err(|_| ObjectError::Unsupported)?,
        _ => return Ok(fail(ctx, ObjectError::TypeError)),
    };
    let state_values = std::iter::once(Word::fixnum(0))
        .chain(data.into_iter().map(|byte| Word::fixnum(i64::from(byte))))
        .collect::<Vec<_>>();
    let state = make_simple_vector(ctx, runtime, &state_values)?;
    let direction_word = option(ctx, args, "DIRECTION")?.unwrap_or(Word::NIL);
    let format_word = option(ctx, args, "EXTERNAL-FORMAT")?.unwrap_or(Word::NIL);
    Ok(make_stream(
        ctx,
        runtime,
        direction_word,
        Word::NIL,
        format_word,
        state,
        Word::NIL,
    )?
    .into())
}

pub(crate) fn close_adapter(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let state = stream_state(ctx, stream_from_args(args, 0)?)?;
    simple_vector_set(ctx, state, 0, Word::fixnum(CLOSED))?;
    Ok(Word::TRUE)
}

pub(crate) fn file_position_adapter(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let state = stream_state(ctx, Stream::from_word(args.required(0)?))?;
    ensure_open(ctx, state)?;
    if let Some(position) = args.get(1) {
        let position = usize::try_from(position.as_fixnum().ok_or(ObjectError::TypeError)?)
            .map_err(|_| ObjectError::TypeError)?;
        if position > simple_vector_length(ctx, state)?.saturating_sub(DATA) {
            return Ok(fail(ctx, ObjectError::TypeError));
        }
        simple_vector_set(
            ctx,
            state,
            POSITION,
            Word::fixnum(i64::try_from(position).map_err(|_| ObjectError::Layout)?),
        )?;
    }
    simple_vector_ref(ctx, state, POSITION)
}

pub(crate) fn file_length_adapter(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let state = stream_state(ctx, Stream::from_word(args.required(0)?))?;
    ensure_open(ctx, state)?;
    Ok(Word::fixnum(
        i64::try_from(simple_vector_length(ctx, state)?.saturating_sub(DATA))
            .map_err(|_| ObjectError::Layout)?,
    ))
}

pub(crate) fn file_string_length_adapter(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    if stream_external_format(ctx, Stream::from_word(args.required(0)?))? != Word::NIL {
        return Err(ObjectError::Unsupported);
    }
    let length = text(ctx, args.required(1)?)?.len();
    Ok(Word::fixnum(
        i64::try_from(length).map_err(|_| ObjectError::Layout)?,
    ))
}

pub(crate) fn streamp_adapter(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(
        if matches!(
            classify_object(ctx, args.required(0)?),
            ObjectRef::Stream(_)
        ) {
            Word::TRUE
        } else {
            Word::NIL
        },
    )
}

pub(crate) fn stream_direction_matches(
    ctx: &ThreadContext,
    stream: Word,
    expected: &str,
) -> Result<bool, ObjectError> {
    let direction = stream_direction(ctx, Stream::from_word(stream))?;
    if direction == Word::NIL {
        return Ok(expected == "INPUT");
    }
    let direction = symbol_text(ctx, direction)?;
    Ok(direction.eq_ignore_ascii_case(expected) || direction.eq_ignore_ascii_case("IO"))
}

pub(crate) fn input_stream_p_adapter(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(
        if stream_direction_matches(ctx, args.required(0)?, "INPUT")? {
            Word::TRUE
        } else {
            Word::NIL
        },
    )
}

pub(crate) fn output_stream_p_adapter(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(
        if stream_direction_matches(ctx, args.required(0)?, "OUTPUT")? {
            Word::TRUE
        } else {
            Word::NIL
        },
    )
}

pub(crate) fn open_stream_p_adapter(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let state = stream_state(ctx, stream_from_args(args, 0)?)?;
    if simple_vector_ref(ctx, state, 0)?.as_fixnum() == Some(CLOSED) {
        Ok(Word::NIL)
    } else {
        Ok(Word::TRUE)
    }
}

#[allow(clippy::missing_const_for_fn, clippy::unnecessary_wraps)]
pub(crate) fn interactive_stream_p_adapter(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    _args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(Word::NIL)
}

pub(crate) fn stream_element_type_adapter(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    stream_element_type(ctx, Stream::from_word(args.required(0)?))
}

pub(crate) fn stream_external_format_adapter(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    stream_external_format(ctx, Stream::from_word(args.required(0)?))
}
