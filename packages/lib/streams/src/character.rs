use super::{CLOSED, DATA, POSITION, STRING_INPUT, STRING_OUTPUT};

use ncl_object::{
    BuiltinArgs, MultipleValues, ObjectError, ObjectRef, Package, Runtime, Stream, ThreadContext,
    Word, car, cdr, classify_object, make_cons, make_simple_vector, make_stream, make_string,
    pop_root, push_root, simple_vector_length, simple_vector_ref, simple_vector_set, stream_state,
    string_length, string_ref, with_root, with_roots,
};

const FILE_IO: i64 = -5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StreamKind {
    Data,
    StringInput,
    StringOutput,
    FileOutput,
    FileIo,
    Closed,
    StandardInput,
    StandardOutput,
    StandardError,
    StandardTwoWay,
}

impl StreamKind {
    pub(crate) const fn code(self) -> i64 {
        match self {
            Self::Data => 0,
            Self::StringInput => STRING_INPUT,
            Self::StringOutput => STRING_OUTPUT,
            Self::FileOutput => super::FILE_OUTPUT,
            Self::FileIo => FILE_IO,
            Self::Closed => CLOSED,
            Self::StandardInput => -6,
            Self::StandardOutput => -7,
            Self::StandardError => -8,
            Self::StandardTwoWay => -9,
        }
    }
}

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

fn stream_or_default(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    index: usize,
    name: &str,
) -> Result<Stream, ObjectError> {
    match args.get(index) {
        Some(word) if word != Word::NIL => Ok(Stream::from_word(word)),
        Some(_) | None => super::standard::lookup(ctx, runtime, name),
    }
}

pub(crate) fn state_kind(ctx: &ThreadContext, state: Word) -> Result<StreamKind, ObjectError> {
    match simple_vector_ref(ctx, state, 0)?.as_fixnum() {
        Some(0) => Ok(StreamKind::Data),
        Some(value) if value == STRING_INPUT => Ok(StreamKind::StringInput),
        Some(value) if value == STRING_OUTPUT => Ok(StreamKind::StringOutput),
        Some(value) if value == super::FILE_OUTPUT => Ok(StreamKind::FileOutput),
        Some(value) if value == FILE_IO => Ok(StreamKind::FileIo),
        Some(value) if value == CLOSED => Ok(StreamKind::Closed),
        Some(value) if value == StreamKind::StandardInput.code() => Ok(StreamKind::StandardInput),
        Some(value) if value == StreamKind::StandardOutput.code() => Ok(StreamKind::StandardOutput),
        Some(value) if value == StreamKind::StandardError.code() => Ok(StreamKind::StandardError),
        Some(value) if value == StreamKind::StandardTwoWay.code() => Ok(StreamKind::StandardTwoWay),
        Some(value) => invalid_stream_kind(value),
        None => Err(ObjectError::Layout),
    }
}

const fn invalid_stream_kind(_value: i64) -> Result<StreamKind, ObjectError> {
    Err(ObjectError::Layout)
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
        let byte = super::file::read_file_byte(ctx, state, pos)?;
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

pub(crate) fn peek_character(
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

pub(crate) fn read_char_adapter(
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

pub(crate) fn unread_char_adapter(
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

pub(crate) fn peek_char_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream_index = usize::from(args.len() > 1);
    let stream = if args.is_empty() {
        super::standard::lookup(ctx, runtime, "*STANDARD-INPUT*")?
    } else {
        stream_from_args(args, stream_index)?
    };
    peek_character(ctx, stream)?.map_or_else(
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

pub(crate) fn write_to_stream(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    stream: Stream,
    character: char,
) -> Result<(), ObjectError> {
    let state = stream_state(ctx, stream)?;
    ensure_open(ctx, state)?;
    let kind = state_kind(ctx, state)?;
    match kind {
        StreamKind::FileOutput | StreamKind::FileIo => {
            let mut buffer = [0; 4];
            let encoded = character.encode_utf8(&mut buffer);
            super::file::write_bytes(ctx, state, encoded.as_bytes())?;
        }
        StreamKind::StandardOutput | StreamKind::StandardTwoWay => {
            let mut output = std::io::stdout();
            let mut buffer = [0; 4];
            let encoded = character.encode_utf8(&mut buffer);
            std::io::Write::write_all(&mut output, encoded.as_bytes())
                .map_err(|_| ObjectError::Layout)?;
        }
        StreamKind::StandardError => {
            let mut output = std::io::stderr();
            let mut buffer = [0; 4];
            let encoded = character.encode_utf8(&mut buffer);
            std::io::Write::write_all(&mut output, encoded.as_bytes())
                .map_err(|_| ObjectError::Layout)?;
        }
        StreamKind::StandardInput => return Err(ObjectError::TypeError),
        StreamKind::StringOutput => {
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
            result?;
        }
        StreamKind::StringInput | StreamKind::Data | StreamKind::Closed => {
            if kind != StreamKind::Data {
                return Err(ObjectError::TypeError);
            }
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
            set_position(ctx, state, POSITION, pos.saturating_add(1))?;
        }
    }
    Ok(())
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
    let stream = stream_or_default(ctx, runtime, args, 1, "*STANDARD-OUTPUT*")?;
    write_to_stream(ctx, runtime, stream, character)?;
    args.required(0)
}

pub(crate) fn write_byte_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let byte = args
        .required(0)?
        .as_fixnum()
        .ok_or(ObjectError::TypeError)?;
    let byte = u8::try_from(byte).map_err(|_| ObjectError::TypeError)?;
    let stream = stream_or_default(ctx, runtime, args, 1, "*STANDARD-OUTPUT*")?;
    let state = stream_state(ctx, stream)?;
    ensure_open(ctx, state)?;
    match state_kind(ctx, state)? {
        StreamKind::FileOutput | StreamKind::FileIo => {
            super::file::write_bytes(ctx, state, &[byte])?;
        }
        StreamKind::StandardOutput | StreamKind::StandardTwoWay => {
            std::io::Write::write_all(&mut std::io::stdout(), &[byte])
                .map_err(|_| ObjectError::Layout)?;
        }
        StreamKind::StandardError => {
            std::io::Write::write_all(&mut std::io::stderr(), &[byte])
                .map_err(|_| ObjectError::Layout)?;
        }
        StreamKind::StandardInput
        | StreamKind::StringInput
        | StreamKind::StringOutput
        | StreamKind::Data
        | StreamKind::Closed => return Err(ObjectError::TypeError),
    }
    args.required(0)
}

pub(crate) fn finish_output_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = stream_or_default(ctx, runtime, args, 0, "*STANDARD-OUTPUT*")?;
    let state = stream_state(ctx, stream)?;
    ensure_open(ctx, state)?;
    match state_kind(ctx, state)? {
        StreamKind::FileOutput | StreamKind::FileIo => super::file::flush_file_stream(ctx, state)?,
        StreamKind::StandardOutput | StreamKind::StandardTwoWay => {
            std::io::Write::flush(&mut std::io::stdout()).map_err(|_| ObjectError::Layout)?;
        }
        StreamKind::StandardError => {
            std::io::Write::flush(&mut std::io::stderr()).map_err(|_| ObjectError::Layout)?;
        }
        StreamKind::StandardInput
        | StreamKind::StringInput
        | StreamKind::StringOutput
        | StreamKind::Data => {}
        StreamKind::Closed => return Err(ObjectError::TypeError),
    }
    Ok(Word::NIL)
}

pub(crate) fn read_byte_adapter(
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
        let byte = super::file::read_file_byte(ctx, state, position)?;
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
