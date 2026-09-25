//! Typed ANSI file-stream builtins.

#![forbid(unsafe_code)]

use std::fs;

use ncl_object::{
    car, cdr, classify_object, make_cons, make_simple_vector, make_stream, make_string,
    simple_vector_length, simple_vector_ref, simple_vector_set, stream_direction,
    stream_element_type, stream_external_format, stream_state, string_length, string_ref,
    symbol_name, Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier,
    BuiltinImplementation, BuiltinName, BuiltinPackage, LambdaList, MultipleValues, ObjectError,
    ObjectRef, Package, Parameter, ParameterType, Runtime, Stream, ThreadContext, Word,
};

const POSITION: usize = 1;
const DATA: usize = 2;
const STRING_INPUT: i64 = -1;
const STRING_OUTPUT: i64 = -2;
const CHARACTER_PARAMETER: Parameter = Parameter {
    name: BuiltinName::new("character"),
    ty: ParameterType::Any,
};
const STRING_PARAMETER: Parameter = Parameter {
    name: BuiltinName::new("string"),
    ty: ParameterType::Any,
};
const ARGUMENTS_PARAMETER: Parameter = Parameter {
    name: BuiltinName::new("arguments"),
    ty: ParameterType::Any,
};
const CHARACTER_REQUIRED: &[Parameter] = &[CHARACTER_PARAMETER];
const CHARACTER_AND_STREAM: &[Parameter] = &[CHARACTER_PARAMETER, STREAM_PARAMETERS[0]];
const OPEN_PARAMETERS: &[Parameter] = &[Parameter {
    name: BuiltinName::new("namestring"),
    ty: ParameterType::Any,
}];
const STREAM_PARAMETERS: &[Parameter] = &[Parameter {
    name: BuiltinName::new("stream"),
    ty: ParameterType::Any,
}];
const STRING_PARAMETERS: &[Parameter] = &[
    Parameter {
        name: BuiltinName::new("stream"),
        ty: ParameterType::Any,
    },
    Parameter {
        name: BuiltinName::new("string"),
        ty: ParameterType::Any,
    },
];

/// Register the implemented file-stream functions.
///
/// # Errors
///
/// Returns an error when a builtin cannot be registered.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    let open = BuiltinImplementation::adapted(
        Builtin {
            lambda_list: LambdaList::with_rest(
                OPEN_PARAMETERS,
                Parameter {
                    name: BuiltinName::new("options"),
                    ty: ParameterType::Any,
                },
            ),
            convention: BuiltinConvention::Adapted,
        },
        open_adapter,
        pass_arguments,
    );
    runtime.register_builtin(
        &mut ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("OPEN")),
        open,
    )?;
    let position = BuiltinImplementation::adapted(
        Builtin {
            lambda_list: LambdaList::with_optional(STREAM_PARAMETERS, &[]),
            convention: BuiltinConvention::Adapted,
        },
        file_position_adapter,
        pass_arguments,
    );
    runtime.register_builtin(
        &mut ctx,
        BuiltinIdentifier::new(
            BuiltinPackage::CommonLisp,
            BuiltinName::new("FILE-POSITION"),
        ),
        position,
    )?;
    runtime.register_builtin(
        &mut ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("FILE-LENGTH")),
        BuiltinImplementation::direct(
            Builtin {
                lambda_list: LambdaList::fixed(STREAM_PARAMETERS),
                convention: BuiltinConvention::Direct(Arity::exact(1)),
            },
            file_length_adapter,
        ),
    )?;
    runtime.register_builtin(
        &mut ctx,
        BuiltinIdentifier::new(
            BuiltinPackage::CommonLisp,
            BuiltinName::new("FILE-STRING-LENGTH"),
        ),
        BuiltinImplementation::direct(
            Builtin {
                lambda_list: LambdaList::fixed(STRING_PARAMETERS),
                convention: BuiltinConvention::Direct(Arity::exact(2)),
            },
            file_string_length_adapter,
        ),
    )?;
    for (name, function) in [
        ("STREAMP", streamp_adapter as _),
        ("INPUT-STREAM-P", input_stream_p_adapter as _),
        ("OUTPUT-STREAM-P", output_stream_p_adapter as _),
        ("STREAM-ELEMENT-TYPE", stream_element_type_adapter as _),
        (
            "STREAM-EXTERNAL-FORMAT",
            stream_external_format_adapter as _,
        ),
    ] {
        runtime.register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            BuiltinImplementation::direct(
                Builtin {
                    lambda_list: LambdaList::fixed(STREAM_PARAMETERS),
                    convention: BuiltinConvention::Direct(Arity::exact(1)),
                },
                function,
            ),
        )?;
    }
    register_character_builtins(runtime, &mut ctx)?;
    Ok(())
}

fn register_character_builtins(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
) -> Result<(), ObjectError> {
    let registrations: &[(&str, Builtin, ncl_object::RustBuiltin)] = &[
        (
            "READ-CHAR",
            Builtin {
                lambda_list: LambdaList::with_rest(
                    &[],
                    Parameter {
                        name: BuiltinName::new("arguments"),
                        ty: ParameterType::Any,
                    },
                ),
                convention: BuiltinConvention::Adapted,
            },
            read_char_adapter,
        ),
        (
            "READ-CHAR-NO-HANG",
            Builtin {
                lambda_list: LambdaList::with_rest(
                    &[],
                    Parameter {
                        name: BuiltinName::new("arguments"),
                        ty: ParameterType::Any,
                    },
                ),
                convention: BuiltinConvention::Adapted,
            },
            read_char_adapter,
        ),
        (
            "UNREAD-CHAR",
            Builtin {
                lambda_list: LambdaList::fixed(CHARACTER_AND_STREAM),
                convention: BuiltinConvention::Direct(Arity::exact(2)),
            },
            unread_char_adapter,
        ),
        (
            "PEEK-CHAR",
            Builtin {
                lambda_list: LambdaList::with_rest(
                    &[],
                    Parameter {
                        name: BuiltinName::new("arguments"),
                        ty: ParameterType::Any,
                    },
                ),
                convention: BuiltinConvention::Adapted,
            },
            peek_char_adapter,
        ),
        (
            "READ-LINE",
            Builtin {
                lambda_list: LambdaList::with_rest(
                    &[],
                    Parameter {
                        name: BuiltinName::new("arguments"),
                        ty: ParameterType::Any,
                    },
                ),
                convention: BuiltinConvention::Adapted,
            },
            read_line_adapter,
        ),
        (
            "WRITE-CHAR",
            Builtin {
                lambda_list: LambdaList::with_optional(CHARACTER_REQUIRED, STREAM_PARAMETERS),
                convention: BuiltinConvention::Adapted,
            },
            write_char_adapter,
        ),
        (
            "WRITE-STRING",
            Builtin {
                lambda_list: LambdaList::with_rest(&[STRING_PARAMETER], ARGUMENTS_PARAMETER),
                convention: BuiltinConvention::Adapted,
            },
            write_string_adapter,
        ),
        (
            "WRITE-LINE",
            Builtin {
                lambda_list: LambdaList::with_rest(&[STRING_PARAMETER], ARGUMENTS_PARAMETER),
                convention: BuiltinConvention::Adapted,
            },
            write_line_adapter,
        ),
        (
            "TERPRI",
            Builtin {
                lambda_list: LambdaList::with_optional(&[], STREAM_PARAMETERS),
                convention: BuiltinConvention::Adapted,
            },
            terpri_adapter,
        ),
        (
            "FRESH-LINE",
            Builtin {
                lambda_list: LambdaList::with_optional(&[], STREAM_PARAMETERS),
                convention: BuiltinConvention::Adapted,
            },
            fresh_line_adapter,
        ),
        (
            "MAKE-STRING-INPUT-STREAM",
            Builtin {
                lambda_list: LambdaList::with_rest(&[STRING_PARAMETER], ARGUMENTS_PARAMETER),
                convention: BuiltinConvention::Adapted,
            },
            make_string_input_adapter,
        ),
        (
            "MAKE-STRING-OUTPUT-STREAM",
            Builtin {
                lambda_list: LambdaList::fixed(&[]),
                convention: BuiltinConvention::Direct(Arity::exact(0)),
            },
            make_string_output_adapter,
        ),
        (
            "GET-OUTPUT-STREAM-STRING",
            Builtin {
                lambda_list: LambdaList::fixed(STREAM_PARAMETERS),
                convention: BuiltinConvention::Direct(Arity::exact(1)),
            },
            get_output_stream_string_adapter,
        ),
    ];
    for (name, descriptor, function) in registrations {
        runtime.register_builtin(
            ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            BuiltinImplementation::adapted(*descriptor, *function, pass_arguments),
        )?;
    }
    Ok(())
}

fn pass_arguments(args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    (0..args.len())
        .map(|index| args.get(index).ok_or(ObjectError::TypeError))
        .collect()
}

fn stream_from_args(args: &BuiltinArgs<'_>, index: usize) -> Result<Stream, ObjectError> {
    Ok(Stream::from_word(args.required(index)?))
}

fn state_kind(ctx: &ThreadContext, state: Word) -> Result<Option<i64>, ObjectError> {
    let first = simple_vector_ref(ctx, state, 0)?.as_fixnum();
    Ok(first.filter(|value| *value == STRING_INPUT || *value == STRING_OUTPUT))
}

fn position(ctx: &ThreadContext, state: Word, index: usize) -> Result<usize, ObjectError> {
    usize::try_from(
        simple_vector_ref(ctx, state, index)?
            .as_fixnum()
            .ok_or(ObjectError::Layout)?,
    )
    .map_err(|_| ObjectError::Layout)
}

fn set_position(
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

fn next_character(ctx: &mut ThreadContext, stream: Stream) -> Result<Option<char>, ObjectError> {
    let state = stream_state(ctx, stream)?;
    if state_kind(ctx, state)?.is_some() {
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
    let pos = position(ctx, state, POSITION)?;
    let length = simple_vector_length(ctx, state)?.saturating_sub(DATA);
    if pos >= length {
        return Ok(None);
    }
    let byte = simple_vector_ref(ctx, state, DATA + pos)?
        .as_fixnum()
        .ok_or(ObjectError::Layout)?;
    let character = char::from_u32(u32::try_from(byte).map_err(|_| ObjectError::Layout)?)
        .ok_or(ObjectError::Layout)?;
    set_position(ctx, state, POSITION, pos.saturating_add(1))?;
    Ok(Some(character))
}

fn peek_character(ctx: &mut ThreadContext, stream: Stream) -> Result<Option<char>, ObjectError> {
    let state = stream_state(ctx, stream)?;
    let pos_index = if state_kind(ctx, state)?.is_some() {
        1
    } else {
        POSITION
    };
    let pos = position(ctx, state, pos_index)?;
    let result = next_character(ctx, stream)?;
    set_position(ctx, state, pos_index, pos)?;
    Ok(result)
}

fn read_char_adapter(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = stream_from_args(args, 0)?;
    match next_character(ctx, stream)? {
        Some(character) => Ok(Word::character(u32::from(character))),
        None => Ok(args.get(2).map_or(Word::NIL, |value| value)),
    }
}

fn unread_char_adapter(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = stream_from_args(args, 1)?;
    let state = stream_state(ctx, stream)?;
    let index = if state_kind(ctx, state)?.is_some() {
        1
    } else {
        POSITION
    };
    let pos = position(ctx, state, index)?;
    if pos == 0 {
        return Ok(fail(ctx, ObjectError::TypeError));
    }
    set_position(ctx, state, index, pos - 1)?;
    Ok(args.required(0)?)
}

fn peek_char_adapter(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream_index = if args.len() > 1 { 1 } else { 0 };
    match peek_character(ctx, stream_from_args(args, stream_index)?)? {
        Some(character) => Ok(Word::character(u32::from(character))),
        None => Ok(args.get(3).map_or(Word::NIL, |value| value)),
    }
}

fn read_line_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = stream_from_args(args, 0)?;
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

fn write_to_stream(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    stream: Stream,
    character: char,
) -> Result<(), ObjectError> {
    let state = stream_state(ctx, stream)?;
    match state_kind(ctx, state)? {
        Some(STRING_OUTPUT) => {
            let count = position(ctx, state, 1)?;
            let list = simple_vector_ref(ctx, state, 2)?;
            let next = make_cons(ctx, runtime, Word::character(u32::from(character)), list)?;
            simple_vector_set(ctx, state, 2, next)?;
            set_position(ctx, state, 1, count.saturating_add(1))
        }
        Some(STRING_INPUT) => Err(ObjectError::TypeError),
        None => {
            let pos = position(ctx, state, POSITION)?;
            if DATA + pos >= simple_vector_length(ctx, state)? {
                return Err(ObjectError::TypeError);
            }
            simple_vector_set(
                ctx,
                state,
                DATA + pos,
                Word::fixnum(i64::from(character as u32)),
            )?;
            set_position(ctx, state, POSITION, pos.saturating_add(1))
        }
        Some(_) => Err(ObjectError::Layout),
    }
}

fn write_char_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let code = args.required(0)?.bits() >> 4;
    let character = char::from_u32(u32::try_from(code).map_err(|_| ObjectError::TypeError)?)
        .ok_or(ObjectError::TypeError)?;
    write_to_stream(ctx, runtime, stream_from_args(args, 1)?, character)?;
    Ok(args.required(0)?)
}

fn write_string_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let string = args.required(0)?;
    let stream = stream_from_args(args, 1)?;
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
    for index in start..end {
        write_to_stream(ctx, runtime, stream, string_ref(ctx, string, index)?)?;
    }
    Ok(string)
}

fn write_line_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let string = write_string_adapter(ctx, runtime, args, values)?;
    write_to_stream(ctx, runtime, stream_from_args(args, 1)?, '\n')?;
    Ok(string)
}

fn terpri_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    write_to_stream(ctx, runtime, stream_from_args(args, 0)?, '\n')?;
    Ok(Word::NIL)
}

fn fresh_line_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = stream_from_args(args, 0)?;
    if peek_character(ctx, stream)? == Some('\n') {
        return Ok(Word::NIL);
    }
    write_to_stream(ctx, runtime, stream, '\n')?;
    Ok(Word::TRUE)
}

fn make_string_input_adapter(
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
        .map_or(0, |value| value);
    let end = args
        .get(2)
        .and_then(Word::as_fixnum)
        .map(|value| usize::try_from(value).map_err(|_| ObjectError::TypeError))
        .transpose()?
        .map_or(length, |value| value);
    if start > end || end > length {
        return Err(ObjectError::TypeError);
    }
    let state = make_simple_vector(
        ctx,
        runtime,
        &[
            Word::fixnum(STRING_INPUT),
            Word::fixnum(i64::try_from(start).map_err(|_| ObjectError::Layout)?),
            string,
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
}

fn make_string_output_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    _args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let package = runtime.ensure_package(ctx, "COMMON-LISP")?;
    let (direction, _) = Package::from_word(package).intern(ctx, runtime, "OUTPUT")?;
    let state = make_simple_vector(
        ctx,
        runtime,
        &[Word::fixnum(STRING_OUTPUT), Word::fixnum(0), Word::NIL],
    )?;
    Ok(make_stream(
        ctx,
        runtime,
        direction,
        Word::NIL,
        Word::NIL,
        state,
        Word::NIL,
    )?
    .into())
}

fn get_output_stream_string_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = stream_from_args(args, 0)?;
    let state = stream_state(ctx, stream)?;
    if state_kind(ctx, state)? != Some(STRING_OUTPUT) {
        return Err(ObjectError::TypeError);
    }
    let mut list = simple_vector_ref(ctx, state, 2)?;
    let mut characters = Vec::new();
    while list != Word::NIL {
        characters.push(
            char::from_u32(
                u32::try_from(car(ctx, list)?.bits() >> 4).map_err(|_| ObjectError::Layout)?,
            )
            .ok_or(ObjectError::Layout)?,
        );
        list = cdr(ctx, list)?;
    }
    characters.reverse();
    simple_vector_set(ctx, state, 1, Word::fixnum(0))?;
    simple_vector_set(ctx, state, 2, Word::NIL)?;
    make_string(ctx, runtime, &characters)
}

const fn fail(ctx: &mut ThreadContext, error: ObjectError) -> Word {
    ctx.set_pending(error);
    Word::UNBOUND
}

fn text(ctx: &ThreadContext, value: Word) -> Result<String, ObjectError> {
    (0..string_length(ctx, value)?)
        .map(|index| string_ref(ctx, value, index))
        .collect()
}

fn symbol_text(ctx: &ThreadContext, value: Word) -> Result<String, ObjectError> {
    text(ctx, symbol_name(ctx, value)?)
}

fn option(
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

fn open_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let path = text(ctx, args.required(0)?)?;
    let direction = option(ctx, args, "DIRECTION")?
        .map(|word| symbol_text(ctx, word))
        .transpose()?
        .unwrap_or_else(|| "INPUT".to_owned());
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
        "IO" => fs::read(&path).unwrap_or_default(),
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

fn file_position_adapter(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let state = stream_state(ctx, Stream::from_word(args.required(0)?))?;
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

fn file_length_adapter(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let state = stream_state(ctx, Stream::from_word(args.required(0)?))?;
    Ok(Word::fixnum(
        i64::try_from(simple_vector_length(ctx, state)?.saturating_sub(DATA))
            .map_err(|_| ObjectError::Layout)?,
    ))
}

fn file_string_length_adapter(
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

fn streamp_adapter(
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

fn stream_direction_matches(
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

fn input_stream_p_adapter(
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

fn output_stream_p_adapter(
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

fn stream_element_type_adapter(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    stream_element_type(ctx, Stream::from_word(args.required(0)?))
}

fn stream_external_format_adapter(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    stream_external_format(ctx, Stream::from_word(args.required(0)?))
}
