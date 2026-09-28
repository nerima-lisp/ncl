use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, FunctionObject, LambdaList, MultipleValues, ObjectError, ObjectRef, Parameter,
    ParameterType, Runtime, ThreadContext, Word, classify_object, make_string, string_length,
    string_ref, symbol_name, symbol_value, with_root,
};
use ncl_printer::{CharSink, PrintError, StringSink};

use crate::{execute, parse};

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
    let control = string_value(ctx, args.required(1)?)?;
    let parsed = parse(&control).map_err(|_| ObjectError::TypeError)?;
    let arguments = (2..args.len())
        .map(|index| args.get(index).ok_or(ObjectError::TypeError))
        .collect::<Result<Vec<_>, _>>()?;

    ncl_object::with_rooted_slice(ctx, &arguments, |ctx, arguments| {
        ncl_object::with_root(ctx, &mut destination, |ctx, destination| {
            if *destination == Word::NIL {
                let mut sink = StringSink::new();
                execute(&parsed, arguments, ctx, runtime, &mut sink)
                    .map_err(|error| format_error_to_object_error(&error))?;
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
                execute(&parsed, arguments, ctx, runtime, &mut sink)
                    .map_err(|error| format_error_to_object_error(&error))?;
                let output = sink.into_string();
                let mut writer = WriteCharSink::new(ctx, runtime, *stream)?;
                for character in output.chars() {
                    writer.write_char(character).map_err(|error| match error {
                        PrintError::Object(error) => error,
                        PrintError::Sink(_) | PrintError::NotReadable | PrintError::Circularity => {
                            ObjectError::TypeError
                        }
                        _ => ObjectError::TypeError,
                    })?;
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

const fn format_error_to_object_error(error: &crate::FormatError) -> ObjectError {
    match error {
        crate::FormatError::Print(PrintError::Object(error)) => *error,
        crate::FormatError::MissingArgument { .. }
        | crate::FormatError::InvalidParameter { .. }
        | crate::FormatError::NonInteger { .. }
        | crate::FormatError::Print(_) => ObjectError::TypeError,
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
