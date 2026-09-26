use super::{
    BuiltinArgs, MultipleValues, ObjectError, ObjectRef, Runtime, Stream, StreamKind,
    ThreadContext, Word, classify_object, ensure_open, make_cons, pop_root, position, push_root,
    set_position, simple_vector_length, simple_vector_ref, simple_vector_set, state_kind,
    stream_or_default, stream_state,
};
use crate::{DATA, POSITION};

pub fn write_to_stream(
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
            crate::file::write_bytes(ctx, state, encoded.as_bytes())?;
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

pub fn write_char_adapter(
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

pub fn write_byte_adapter(
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
            crate::file::write_bytes(ctx, state, &[byte])?;
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

pub fn finish_output_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = stream_or_default(ctx, runtime, args, 0, "*STANDARD-OUTPUT*")?;
    let state = stream_state(ctx, stream)?;
    ensure_open(ctx, state)?;
    match state_kind(ctx, state)? {
        StreamKind::FileOutput | StreamKind::FileIo => crate::file::flush_file_stream(ctx, state)?,
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
