//! Typed ANSI file-stream builtins.

#![forbid(unsafe_code)]

use std::fs;

use ncl_object::{
    classify_object, make_simple_vector, make_stream, simple_vector_length, simple_vector_ref,
    simple_vector_set, stream_direction, stream_element_type, stream_external_format, stream_state,
    string_length, string_ref, symbol_name, Arity, Builtin, BuiltinArgs, BuiltinConvention,
    BuiltinIdentifier, BuiltinImplementation, BuiltinName, BuiltinPackage, LambdaList,
    MultipleValues, ObjectError, ObjectRef, Parameter, ParameterType, Runtime, Stream,
    ThreadContext, Word,
};

const POSITION: usize = 1;
const DATA: usize = 2;
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
    Ok(())
}

fn pass_arguments(args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    (0..args.len())
        .map(|index| args.get(index).ok_or(ObjectError::TypeError))
        .collect()
}

fn fail(ctx: &mut ThreadContext, error: ObjectError) -> Word {
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
