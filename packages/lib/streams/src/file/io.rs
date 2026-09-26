use super::*;

pub(crate) fn file_path(ctx: &ThreadContext, state: Word) -> Result<String, ObjectError> {
    text(ctx, simple_vector_ref(ctx, state, 2)?)
}

pub(crate) fn write_bytes(
    ctx: &mut ThreadContext,
    state: Word,
    bytes: &[u8],
) -> Result<(), ObjectError> {
    ensure_open(ctx, state)?;
    let kind = state_kind(ctx, state)?;
    let path = file_path(ctx, state)?;
    let position = position(ctx, state, POSITION)?;
    let mut file = fs::OpenOptions::new();
    file.write(true);
    if kind == StreamKind::FileIo {
        file.read(true);
    } else if kind != StreamKind::FileOutput {
        return Err(ObjectError::TypeError);
    }
    let mut file = file.open(path).map_err(|_| ObjectError::Layout)?;
    file.seek(SeekFrom::Start(
        u64::try_from(position).map_err(|_| ObjectError::Layout)?,
    ))
    .map_err(|_| ObjectError::Layout)?;
    file.write_all(bytes).map_err(|_| ObjectError::Layout)?;
    file.flush().map_err(|_| ObjectError::Layout)?;
    set_position(ctx, state, POSITION, position.saturating_add(bytes.len()))
}

pub(crate) fn read_file_byte(
    ctx: &ThreadContext,
    state: Word,
    position: usize,
) -> Result<Option<u8>, ObjectError> {
    if state_kind(ctx, state)? != StreamKind::FileIo {
        return Err(ObjectError::TypeError);
    }
    let mut file = fs::File::open(file_path(ctx, state)?).map_err(|_| ObjectError::Layout)?;
    file.seek(SeekFrom::Start(
        u64::try_from(position).map_err(|_| ObjectError::Layout)?,
    ))
    .map_err(|_| ObjectError::Layout)?;
    let mut byte = [0_u8; 1];
    let count = file.read(&mut byte).map_err(|_| ObjectError::Layout)?;
    if count == 0 {
        Ok(None)
    } else {
        byte.first().copied().ok_or(ObjectError::Layout).map(Some)
    }
}

pub(crate) fn flush_file_stream(ctx: &ThreadContext, state: Word) -> Result<(), ObjectError> {
    let kind = state_kind(ctx, state)?;
    if kind != StreamKind::FileOutput && kind != StreamKind::FileIo {
        return Ok(());
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .open(file_path(ctx, state)?)
        .map_err(|_| ObjectError::Layout)?;
    file.flush().map_err(|_| ObjectError::Layout)
}
