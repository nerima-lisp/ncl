use super::{
    BuiltinArgs, DATA, MultipleValues, ObjectError, POSITION, Runtime, Stream, StreamKind,
    ThreadContext, Word, ensure_open, make_string, position, set_position, simple_vector_length,
    simple_vector_ref, state_kind, stream_from_args, stream_or_default, stream_state,
    string_length, string_ref,
};

pub fn next_character(
    ctx: &mut ThreadContext,
    stream: Stream,
) -> Result<Option<char>, ObjectError> {
    let state = stream_state(ctx, stream)?;
    ensure_open(ctx, state)?;
    let kind = state_kind(ctx, state)?;
    if kind == StreamKind::StringInput {
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
    if matches!(
        kind,
        StreamKind::FileOutput
            | StreamKind::StringOutput
            | StreamKind::StandardOutput
            | StreamKind::StandardError
    ) {
        return Err(ObjectError::TypeError);
    }
    if matches!(kind, StreamKind::StandardInput | StreamKind::StandardTwoWay) {
        let mut input = std::io::stdin();
        let mut byte = [0_u8; 1];
        let count = std::io::Read::read(&mut input, &mut byte).map_err(|_| ObjectError::Layout)?;
        if count == 0 {
            return Ok(None);
        }
        let pos = position(ctx, state, POSITION)?;
        set_position(ctx, state, POSITION, pos.saturating_add(1))?;
        return byte
            .first()
            .copied()
            .map(char::from)
            .map(Some)
            .ok_or(ObjectError::Layout);
    }
    if kind == StreamKind::FileIo {
        let pos = position(ctx, state, POSITION)?;
        let byte = crate::file::read_file_byte(ctx, state, pos)?;
        if let Some(byte) = byte {
            set_position(ctx, state, POSITION, pos.saturating_add(1))?;
            return Ok(Some(char::from(byte)));
        }
        return Ok(None);
    }
    let data_offset = if kind == StreamKind::FileIo {
        DATA + 1
    } else {
        DATA
    };
    let pos = position(ctx, state, POSITION)?;
    let length = simple_vector_length(ctx, state)?.saturating_sub(data_offset);
    if pos >= length {
        return Ok(None);
    }
    let byte = simple_vector_ref(ctx, state, data_offset + pos)?
        .as_fixnum()
        .ok_or(ObjectError::Layout)?;
    let character = char::from_u32(u32::try_from(byte).map_err(|_| ObjectError::Layout)?)
        .ok_or(ObjectError::Layout)?;
    set_position(ctx, state, POSITION, pos.saturating_add(1))?;
    Ok(Some(character))
}

pub fn peek_character(
    ctx: &mut ThreadContext,
    stream: Stream,
) -> Result<Option<char>, ObjectError> {
    let state = stream_state(ctx, stream)?;
    if matches!(
        state_kind(ctx, state)?,
        StreamKind::StandardInput | StreamKind::StandardTwoWay
    ) {
        return Err(ObjectError::TypeError);
    }
    let pos_index = if matches!(
        state_kind(ctx, state)?,
        StreamKind::StringInput
            | StreamKind::StringOutput
            | StreamKind::StandardInput
            | StreamKind::StandardTwoWay
    ) {
        1
    } else {
        POSITION
    };
    let pos = position(ctx, state, pos_index)?;
    let result = next_character(ctx, stream)?;
    set_position(ctx, state, pos_index, pos)?;
    Ok(result)
}

pub fn read_char_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = stream_or_default(ctx, runtime, args, 0, "*STANDARD-INPUT*")?;
    next_character(ctx, stream)?.map_or_else(
        || Ok(args.get(2).unwrap_or(Word::NIL)),
        |character| Ok(Word::character(u32::from(character))),
    )
}

pub fn unread_char_adapter(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = stream_from_args(args, 1)?;
    let state = stream_state(ctx, stream)?;
    if matches!(
        state_kind(ctx, state)?,
        StreamKind::StandardInput | StreamKind::StandardTwoWay
    ) {
        return Err(ObjectError::TypeError);
    }
    let index = if matches!(
        state_kind(ctx, state)?,
        StreamKind::StringInput
            | StreamKind::StringOutput
            | StreamKind::StandardInput
            | StreamKind::StandardTwoWay
    ) {
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

pub fn peek_char_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream_index = usize::from(args.len() > 1);
    let stream = if args.is_empty() {
        crate::standard::lookup(ctx, runtime, "*STANDARD-INPUT*")?
    } else {
        stream_from_args(args, stream_index)?
    };
    peek_character(ctx, stream)?.map_or_else(
        || Ok(args.get(3).unwrap_or(Word::NIL)),
        |character| Ok(Word::character(u32::from(character))),
    )
}

pub fn read_line_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = stream_or_default(ctx, runtime, args, 0, "*STANDARD-INPUT*")?;
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

pub fn read_byte_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = stream_or_default(ctx, runtime, args, 0, "*STANDARD-INPUT*")?;
    let state = stream_state(ctx, stream)?;
    ensure_open(ctx, state)?;
    let kind = state_kind(ctx, state)?;
    if matches!(kind, StreamKind::StandardInput | StreamKind::StandardTwoWay) {
        let mut input = std::io::stdin();
        let mut byte = [0_u8; 1];
        let count = std::io::Read::read(&mut input, &mut byte).map_err(|_| ObjectError::Layout)?;
        return byte.first().copied().filter(|_| count != 0).map_or_else(
            || Ok(args.get(2).unwrap_or(Word::NIL)),
            |value| Ok(Word::fixnum(i64::from(value))),
        );
    }
    if kind == StreamKind::FileIo {
        let position = position(ctx, state, POSITION)?;
        let byte = crate::file::read_file_byte(ctx, state, position)?;
        if let Some(byte) = byte {
            set_position(ctx, state, POSITION, position.saturating_add(1))?;
            return Ok(Word::fixnum(i64::from(byte)));
        }
        return Ok(args.get(2).unwrap_or(Word::NIL));
    }
    if kind != StreamKind::Data {
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
