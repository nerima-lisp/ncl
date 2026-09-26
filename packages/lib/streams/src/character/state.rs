use super::*;

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

pub(crate) fn stream_or_default(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    index: usize,
    name: &str,
) -> Result<Stream, ObjectError> {
    match args.get(index) {
        Some(word) if word != Word::NIL => Ok(Stream::from_word(word)),
        Some(_) | None => crate::standard::lookup(ctx, runtime, name),
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
