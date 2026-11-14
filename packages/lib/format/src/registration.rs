use ncl_object::typed::{
    ConditionErrorData, FormatErrorCause, FormatErrorData, FormatParseError, FormatPrintError,
    LispError, StreamError,
};
use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, FunctionObject, LambdaList, MultipleValues, ObjectError, ObjectRef, Parameter,
    ParameterType, Runtime, ThreadContext, Word, classify_object, make_cons, make_string,
    string_length, string_ref, symbol_name, symbol_value, with_root,
};
use ncl_printer::{CharSink, PrintError, StringSink};

use crate::{FormatError, FormatErrorKind, ParseErrorKind, execute, parse};

const DESTINATION: Parameter = Parameter {
    name: BuiltinName::new("DESTINATION"),
    ty: ParameterType::Any,
};
const CONTROL: Parameter = Parameter {
    name: BuiltinName::new("CONTROL"),
    ty: ParameterType::StringDesignator,
};
const ARGUMENT: Parameter = Parameter {
    name: BuiltinName::new("ARGUMENT"),
    ty: ParameterType::Any,
};

/// Register the Common Lisp FORMAT builtin.
///
/// # Errors
///
/// Returns an object error when the builtin cannot be installed or FORMAT's
/// destination/control arguments are invalid.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    runtime.register_builtin(
        &mut ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("FORMAT")),
        BuiltinImplementation::adapted(
            Builtin {
                lambda_list: LambdaList::with_rest(&[DESTINATION, CONTROL], ARGUMENT),
                convention: BuiltinConvention::Adapted,
            },
            format_builtin,
            pass_arguments,
        ),
    )?;
    Ok(())
}

fn pass_arguments(args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    (0..args.len())
        .map(|index| args.get(index).ok_or(ObjectError::TypeError))
        .collect()
}

fn format_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut destination = args.required(0)?;
    let format_control = args.required(1)?;
    let control = string_value(ctx, format_control)?;
    let arguments = (2..args.len())
        .map(|index| args.get(index).ok_or(ObjectError::TypeError))
        .collect::<Result<Vec<_>, _>>()?;

    ncl_object::with_root(ctx, &mut destination, |ctx, destination| {
        ncl_object::with_rooted_slice(ctx, &arguments, |ctx, arguments| {
            let parsed = match parse(&control) {
                Ok(parsed) => parsed,
                Err(error) => {
                    let format_error = FormatError::new(
                        error.offset,
                        error.directive,
                        FormatErrorKind::Parse(error.kind),
                    );
                    let stream = target_stream_for_error(ctx, runtime, *destination)?;
                    return Err(format_error_to_object_error(
                        ctx,
                        runtime,
                        &format_error,
                        format_control,
                        arguments,
                        stream,
                    ));
                }
            };

            if *destination == Word::NIL {
                let mut sink = StringSink::new();
                execute(&parsed, arguments, ctx, runtime, &mut sink).map_err(|error| {
                    format_error_to_object_error(
                        ctx,
                        runtime,
                        &error,
                        format_control,
                        arguments,
                        None,
                    )
                })?;
                return make_string(
                    ctx,
                    runtime,
                    &sink.into_string().chars().collect::<Vec<_>>(),
                );
            }

            let mut stream = if *destination == Word::TRUE {
                standard_output(ctx, runtime)?
            } else if matches!(classify_object(ctx, *destination), ObjectRef::Stream(_)) {
                *destination
            } else {
                return Err(ObjectError::TypeError);
            };
            ncl_object::with_root(ctx, &mut stream, |ctx, stream| {
                let mut sink = StringSink::new();
                execute(&parsed, arguments, ctx, runtime, &mut sink).map_err(|error| {
                    format_error_to_object_error(
                        ctx,
                        runtime,
                        &error,
                        format_control,
                        arguments,
                        Some(*stream),
                    )
                })?;
                let output = sink.into_string();
                let write_error = {
                    let mut writer = WriteCharSink::new(ctx, runtime, *stream)?;
                    let mut error = None;
                    for character in output.chars() {
                        if let Err(failure) = writer.write_char(character) {
                            error = Some(failure);
                            break;
                        }
                    }
                    error
                };
                if let Some(error) = write_error {
                    if let PrintError::Object(error) = error {
                        return Err(stream_write_error_to_object_error(ctx, *stream, error));
                    }
                    let format_error = FormatError::new(0, None, FormatErrorKind::Print(error));
                    return Err(format_error_to_object_error(
                        ctx,
                        runtime,
                        &format_error,
                        format_control,
                        arguments,
                        Some(*stream),
                    ));
                }
                Ok(Word::NIL)
            })
        })
    })
}

fn string_value(ctx: &ThreadContext, value: Word) -> Result<String, ObjectError> {
    if let ObjectRef::Character(character) = classify_object(ctx, value) {
        return char::from_u32(character)
            .map(|character| character.to_string())
            .ok_or(ObjectError::TypeError);
    }
    let value = if matches!(classify_object(ctx, value), ObjectRef::String(_)) {
        value
    } else if matches!(classify_object(ctx, value), ObjectRef::Symbol(_)) {
        symbol_name(ctx, value)?
    } else {
        return Err(ObjectError::TypeError);
    };
    (0..string_length(ctx, value)?)
        .map(|index| string_ref(ctx, value, index))
        .collect()
}

fn standard_output(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<Word, ObjectError> {
    let mut package = runtime.ensure_package(ctx, "COMMON-LISP")?;
    with_root(ctx, &mut package, |ctx, package| {
        let (mut symbol, _) =
            ncl_object::Package::from_word(*package).intern(ctx, runtime, "*STANDARD-OUTPUT*")?;
        with_root(ctx, &mut symbol, |ctx, symbol| symbol_value(ctx, *symbol))
    })
}

fn target_stream_for_error(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    destination: Word,
) -> Result<Option<Word>, ObjectError> {
    if destination == Word::NIL {
        Ok(None)
    } else if destination == Word::TRUE {
        standard_output(ctx, runtime).map(Some)
    } else if matches!(classify_object(ctx, destination), ObjectRef::Stream(_)) {
        Ok(Some(destination))
    } else {
        Ok(None)
    }
}

fn format_error_to_object_error(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    error: &FormatError,
    format_control: Word,
    arguments: &[Word],
    stream: Option<Word>,
) -> ObjectError {
    if let Some(mut stream) = stream {
        return match with_root(ctx, &mut stream, |ctx, stream| {
            Ok(record_format_error(
                ctx,
                runtime,
                error,
                format_control,
                arguments,
                Some(*stream),
            ))
        }) {
            Ok(error) | Err(error) => error,
        };
    }
    record_format_error(ctx, runtime, error, format_control, arguments, None)
}

const fn stream_write_error_to_object_error(
    ctx: &mut ThreadContext,
    stream: Word,
    error: ObjectError,
) -> ObjectError {
    ctx.set_pending_lisp_error(LispError::Condition(ConditionErrorData::StreamError {
        kind: StreamError::Io,
        stream,
    }));
    error
}

fn record_format_error(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    error: &FormatError,
    format_control: Word,
    arguments: &[Word],
    stream: Option<Word>,
) -> ObjectError {
    let format_arguments = match format_argument_list(ctx, runtime, arguments) {
        Ok(arguments) => arguments,
        Err(error) => return error,
    };
    ctx.set_pending_lisp_error(LispError::FormatError(FormatErrorData {
        cause: format_error_cause(&error.kind),
        position: error.position,
        directive: error.directive,
        format_control,
        format_arguments,
        stream,
    }));
    match &error.kind {
        FormatErrorKind::Print(PrintError::Object(error)) => *error,
        FormatErrorKind::Parse(_)
        | FormatErrorKind::MissingArgument
        | FormatErrorKind::WrongArgumentType
        | FormatErrorKind::InvalidParameter
        | FormatErrorKind::InvalidDirectiveUse
        | FormatErrorKind::ImproperList
        | FormatErrorKind::Print(
            PrintError::Sink(_) | PrintError::NotReadable | PrintError::Circularity,
        ) => ObjectError::TypeError,
    }
}

fn format_argument_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    arguments: &[Word],
) -> Result<Word, ObjectError> {
    ncl_object::with_rooted_slice(ctx, arguments, |ctx, arguments| {
        let list = ncl_object::HeapRootedWord::new(ctx, Word::NIL)?;
        for argument in arguments.iter().rev() {
            list.set(make_cons(ctx, runtime, *argument, list.get())?);
        }
        Ok(list.get())
    })
}

const fn format_error_cause(error: &FormatErrorKind) -> FormatErrorCause {
    match error {
        FormatErrorKind::Parse(kind) => FormatErrorCause::Parse(match kind {
            ParseErrorKind::UnterminatedDirective => FormatParseError::UnterminatedDirective,
            ParseErrorKind::InvalidParameter => FormatParseError::InvalidParameter,
            ParseErrorKind::MissingDirective => FormatParseError::MissingDirective,
            ParseErrorKind::UnknownDirective => FormatParseError::UnknownDirective,
            ParseErrorKind::UnexpectedTerminator => FormatParseError::UnexpectedTerminator,
            ParseErrorKind::UnterminatedConstruct { expected } => {
                FormatParseError::UnterminatedConstruct {
                    expected: *expected,
                }
            }
            ParseErrorKind::UnterminatedFunctionName => FormatParseError::UnterminatedFunctionName,
        }),
        FormatErrorKind::MissingArgument => FormatErrorCause::MissingArgument,
        FormatErrorKind::WrongArgumentType => FormatErrorCause::WrongArgumentType,
        FormatErrorKind::InvalidParameter => FormatErrorCause::InvalidParameter,
        FormatErrorKind::InvalidDirectiveUse => FormatErrorCause::InvalidDirectiveUse,
        FormatErrorKind::ImproperList => FormatErrorCause::ImproperList,
        FormatErrorKind::Print(PrintError::Object(error)) => {
            FormatErrorCause::Print(FormatPrintError::Object(*error))
        }
        FormatErrorKind::Print(PrintError::Sink(_)) => {
            FormatErrorCause::Print(FormatPrintError::Sink)
        }
        FormatErrorKind::Print(PrintError::NotReadable) => {
            FormatErrorCause::Print(FormatPrintError::NotReadable)
        }
        FormatErrorKind::Print(PrintError::Circularity) => {
            FormatErrorCause::Print(FormatPrintError::Circularity)
        }
    }
}

struct WriteCharSink<'a> {
    ctx: &'a mut ThreadContext,
    runtime: &'a Runtime,
    function: FunctionObject,
    stream: Word,
}

impl<'a> WriteCharSink<'a> {
    fn new(
        ctx: &'a mut ThreadContext,
        runtime: &'a Runtime,
        stream: Word,
    ) -> Result<Self, ObjectError> {
        let function = runtime
            .function(ctx, "COMMON-LISP", "WRITE-CHAR")
            .ok_or(ObjectError::UndefinedFunction)
            .and_then(FunctionObject::try_from)?;
        Ok(Self {
            ctx,
            runtime,
            function,
            stream,
        })
    }
}

impl CharSink for WriteCharSink<'_> {
    fn write_char(&mut self, character: char) -> Result<(), PrintError> {
        let character = Word::character(u32::from(character));
        ncl_object::with_root(self.ctx, &mut self.stream, |ctx, stream| {
            let args = [character, *stream];
            self.runtime
                .call_builtin(ctx, self.function, &args)
                .map(|_| ())
        })
        .map_err(PrintError::Object)
    }
}

#[cfg(test)]
#[path = "registration/tests.rs"]
mod tests;
