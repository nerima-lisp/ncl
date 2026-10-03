use super::{
    BuiltinArgs, DATA, MultipleValues, ObjectError, ObjectRef, POSITION, Runtime, Stream,
    StreamKind, ThreadContext, Word, car, classify_object, ensure_open, make_string, position,
    set_position, simple_vector_length, simple_vector_ref, state_kind, stream_from_args,
    stream_or_default, stream_state, string_length, string_ref,
};

pub fn string_output_at_line_start(
    ctx: &ThreadContext,
    stream: Stream,
) -> Result<bool, ObjectError> {
    let state = stream_state(ctx, stream)?;
    ensure_open(ctx, state)?;
    if state_kind(ctx, state)? != StreamKind::StringOutput {
        return Err(ObjectError::TypeError);
    }
    let output = simple_vector_ref(ctx, state, 2)?;
    if output == Word::NIL {
        return Ok(true);
    }
    let character = car(ctx, output)?;
    match classify_object(ctx, character) {
        ObjectRef::Character(code) => Ok(char::from_u32(code) == Some('\n')),
        _ => Err(ObjectError::Layout), // check-added-lines: allow(wildcard) reject non-character output data
    }
}

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
    let data_offset = DATA;
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

enum PeekMode {
    Next,
    NonWhitespace,
    Character(char),
}

fn classify_peek_type(ctx: &ThreadContext, value: Word) -> Result<PeekMode, ObjectError> {
    if value == Word::NIL {
        return Ok(PeekMode::Next);
    }
    if value == Word::TRUE {
        return Ok(PeekMode::NonWhitespace);
    }
    if let ObjectRef::Character(code) = classify_object(ctx, value) {
        return char::from_u32(code)
            .map(PeekMode::Character)
            .ok_or(ObjectError::TypeError);
    }
    Err(ObjectError::TypeError)
}

const fn is_peek_whitespace(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\n' | '\x0c' | '\r')
}

fn peek_until(
    ctx: &mut ThreadContext,
    stream: Stream,
    peek_type: Word,
) -> Result<Option<char>, ObjectError> {
    let state = stream_state(ctx, stream)?;
    let kind = state_kind(ctx, state)?;
    let mode = classify_peek_type(ctx, peek_type)?;
    let position_index = if matches!(
        kind,
        StreamKind::StringInput
            | StreamKind::StringOutput
            | StreamKind::StandardInput
            | StreamKind::StandardTwoWay
    ) {
        1
    } else {
        POSITION
    };
    let initial_position = position(ctx, state, position_index)?;
    let mut before_character = initial_position;
    loop {
        let Some(character) = next_character(ctx, stream)? else {
            return Ok(None);
        };
        let matches = match mode {
            PeekMode::Next => true,
            PeekMode::NonWhitespace => !is_peek_whitespace(character),
            PeekMode::Character(target) => character == target,
        };
        if matches {
            set_position(ctx, state, position_index, before_character)?;
            return Ok(Some(character));
        }
        before_character = position(ctx, state, position_index)?;
    }
}

pub fn read_char_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = stream_or_default(ctx, runtime, args, 0, "*STANDARD-INPUT*")?;
    let eof_error_p = args.get(1).unwrap_or(Word::TRUE);
    let eof_value = args.get(2).unwrap_or(Word::NIL);
    next_character(ctx, stream)?.map_or_else(
        || eof_result(ctx, eof_error_p, eof_value),
        |character| Ok(Word::character(u32::from(character))),
    )
}

fn eof_result(
    ctx: &mut ThreadContext,
    eof_error_p: Word,
    eof_value: Word,
) -> Result<Word, ObjectError> {
    if eof_error_p == Word::NIL {
        Ok(eof_value)
    } else {
        ctx.set_pending_lisp_error(ncl_object::LispError::EndOfFile);
        Err(ObjectError::TypeError)
    }
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
    let peek_type_value = args.get(0).unwrap_or(Word::NIL);
    let stream = stream_or_default(ctx, runtime, args, 1, "*STANDARD-INPUT*")?;
    let eof_error_p = args.get(2).unwrap_or(Word::TRUE);
    let eof_value = args.get(3).unwrap_or(Word::NIL);
    let result = peek_until(ctx, stream, peek_type_value)?;
    result.map_or_else(
        || {
            if eof_error_p == Word::NIL {
                Ok(eof_value)
            } else {
                ctx.set_pending_lisp_error(ncl_object::LispError::EndOfFile);
                Err(ObjectError::TypeError)
            }
        },
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
    let eof_error_p = args.get(1).unwrap_or(Word::TRUE);
    let eof_value = args.get(2).unwrap_or(Word::NIL);
    let mut characters = Vec::new();
    let mut ended = false;
    while let Some(character) = next_character(ctx, stream)? {
        if character == '\n' {
            ended = true;
            break;
        }
        characters.push(character);
    }
    if characters.is_empty() && !ended {
        let value = eof_result(ctx, eof_error_p, eof_value)?;
        values.set(&[value, Word::NIL]);
        return Ok(value);
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
    let eof_error_p = args.get(1).unwrap_or(Word::TRUE);
    let eof_value = args.get(2).unwrap_or(Word::NIL);
    let state = stream_state(ctx, stream)?;
    ensure_open(ctx, state)?;
    let kind = state_kind(ctx, state)?;
    if matches!(kind, StreamKind::StandardInput | StreamKind::StandardTwoWay) {
        let mut input = std::io::stdin();
        let mut byte = [0_u8; 1];
        let count = std::io::Read::read(&mut input, &mut byte).map_err(|_| ObjectError::Layout)?;
        return byte.first().copied().filter(|_| count != 0).map_or_else(
            || eof_result(ctx, eof_error_p, eof_value),
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
        return eof_result(ctx, eof_error_p, eof_value);
    }
    if kind != StreamKind::Data {
        return Err(ObjectError::TypeError);
    }
    let position = position(ctx, state, POSITION)?;
    let length = simple_vector_length(ctx, state)?.saturating_sub(DATA);
    if position >= length {
        return eof_result(ctx, eof_error_p, eof_value);
    }
    let byte = simple_vector_ref(ctx, state, DATA + position)?
        .as_fixnum()
        .ok_or(ObjectError::Layout)?;
    set_position(ctx, state, POSITION, position.saturating_add(1))?;
    Ok(Word::fixnum(byte))
}
