use super::*;

pub(crate) fn open_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let path = text(ctx, args.required(0)?)?;
    let path_word = args.required(0)?;
    let (direction_word, direction) = direction_word(ctx, args)?;
    let exists = fs::metadata(&path).is_ok();
    let format = format_word(ctx, args)?;
    if direction.eq_ignore_ascii_case("INPUT") {
        let missing = missing_policy(ctx, args, MissingPolicy::Error)?;
        if !exists {
            match missing {
                MissingPolicy::Nil => return Ok(Word::NIL),
                MissingPolicy::Error => return Err(ObjectError::Layout),
                MissingPolicy::Create => {
                    fs::File::create(&path).map_err(|_| ObjectError::Layout)?;
                }
            }
        }
        let mut file = fs::File::open(&path).map_err(|_| ObjectError::Layout)?;
        let interactive = file.is_terminal();
        let mut data = Vec::new();
        if !interactive {
            file.read_to_end(&mut data)
                .map_err(|_| ObjectError::Layout)?;
        }
        let state_values = std::iter::once(Word::fixnum(StreamKind::Data.code()))
            .chain(std::iter::once(Word::fixnum(0)))
            .chain(data.into_iter().map(|byte| Word::fixnum(i64::from(byte))))
            .collect::<Vec<_>>();
        let state = make_simple_vector(ctx, runtime, &state_values)?;
        return Ok(make_stream(
            ctx,
            runtime,
            direction_word,
            Word::NIL,
            format,
            state,
            if interactive { Word::TRUE } else { Word::NIL },
        )?
        .into());
    }
    if direction.eq_ignore_ascii_case("OUTPUT") {
        let missing = missing_policy(ctx, args, MissingPolicy::Create)?;
        if !exists {
            match missing {
                MissingPolicy::Nil => return Ok(Word::NIL),
                MissingPolicy::Error => return Err(ObjectError::Layout),
                MissingPolicy::Create => {}
            }
        }
        let policy = exists_policy(ctx, args)?;
        if exists && policy == ExistsPolicy::Error {
            return Err(ObjectError::TypeError);
        }
        if exists && policy == ExistsPolicy::Nil {
            return Ok(Word::NIL);
        }
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true);
        let position = if policy == ExistsPolicy::Append {
            options.append(true);
            fs::metadata(&path).map_err(|_| ObjectError::Layout)?.len()
        } else {
            if policy == ExistsPolicy::Supersede {
                options.truncate(true);
            }
            0
        };
        let file = options.open(&path).map_err(|_| ObjectError::Layout)?;
        let interactive = file.is_terminal();
        drop(file);
        return make_file_stream(
            ctx,
            runtime,
            path_word,
            direction_word,
            format,
            StreamKind::FileOutput,
            usize::try_from(position).map_err(|_| ObjectError::Layout)?,
            if interactive { Word::TRUE } else { Word::NIL },
        );
    }
    if direction.eq_ignore_ascii_case("IO") {
        let missing = missing_policy(ctx, args, MissingPolicy::Error)?;
        if !exists {
            match missing {
                MissingPolicy::Nil => return Ok(Word::NIL),
                MissingPolicy::Error => return Err(ObjectError::Layout),
                MissingPolicy::Create => {
                    fs::File::create(&path).map_err(|_| ObjectError::Layout)?;
                }
            }
        }
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|_| ObjectError::Layout)?;
        let interactive = file.is_terminal();
        drop(file);
        return make_file_stream(
            ctx,
            runtime,
            path_word,
            direction_word,
            format,
            StreamKind::FileIo,
            0,
            if interactive { Word::TRUE } else { Word::NIL },
        );
    }
    Err(ObjectError::TypeError)
}


pub(crate) fn close_adapter(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let state = stream_state(ctx, stream_from_args(args, 0)?)?;
    ensure_open(ctx, state)?;
    flush_file_stream(ctx, state)?;
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
    let kind = state_kind(ctx, state)?;
    if let Some(position) = args.get(1) {
        let position = usize::try_from(position.as_fixnum().ok_or(ObjectError::TypeError)?)
            .map_err(|_| ObjectError::TypeError)?;
        if kind != StreamKind::FileOutput
            && kind != StreamKind::FileIo
            && position > simple_vector_length(ctx, state)?.saturating_sub(DATA)
        {
            return Err(ObjectError::TypeError);
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
    let kind = state_kind(ctx, state)?;
    let length = if kind == StreamKind::FileOutput || kind == StreamKind::FileIo {
        fs::metadata(file_path(ctx, state)?)
            .map_err(|_| ObjectError::Layout)?
            .len()
            .try_into()
            .map_err(|_| ObjectError::Layout)?
    } else if kind == StreamKind::Data {
        simple_vector_length(ctx, state)?.saturating_sub(DATA)
    } else {
        return Err(ObjectError::TypeError);
    };
    Ok(Word::fixnum(
        i64::try_from(length).map_err(|_| ObjectError::Layout)?,
    ))
}

pub(crate) fn file_string_length_adapter(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    if stream_external_format(ctx, Stream::from_word(args.required(0)?))? != Word::NIL {
        return Err(ObjectError::TypeError);
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

pub(crate) fn interactive_stream_p_adapter(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let implementation = ncl_object::stream_implementation(ctx, stream_from_args(args, 0)?)?;
    Ok(if implementation == Word::TRUE {
        Word::TRUE
    } else {
        Word::NIL
    })
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
