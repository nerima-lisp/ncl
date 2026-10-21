use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, LambdaList, LispError, ObjectError, ObjectType, Package, Parameter,
    ParameterType, Readtable as ObjectReadtable, Runtime, ThreadContext, Word, symbol_value,
    with_heap_rooted_slice, with_root,
};

use crate::{ReadError, Readtable, copy_readtable_to, readtablep, standard_readtable};

const FROM_READTABLE: Parameter = Parameter {
    name: BuiltinName::new("FROM-READTABLE"),
    ty: ParameterType::Any,
};
const TO_READTABLE: Parameter = Parameter {
    name: BuiltinName::new("TO-READTABLE"),
    ty: ParameterType::Any,
};

pub fn register(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(
            BuiltinPackage::CommonLisp,
            BuiltinName::new("COPY-READTABLE"),
        ),
        BuiltinImplementation::adapted(
            Builtin {
                lambda_list: LambdaList::with_optional(&[], &[FROM_READTABLE, TO_READTABLE]),
                convention: BuiltinConvention::Adapted,
            },
            copy_readtable_builtin,
            identity_arguments,
        ),
    )?;
    Ok(())
}

fn copy_readtable_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let from = match args.get(0) {
        Some(from) => from,
        None => current_readtable(ctx, runtime)?,
    };
    let to = args.get(1).unwrap_or(Word::NIL);
    with_heap_rooted_slice(ctx, &[from, to], |ctx, arguments| {
        if arguments
            .first()
            .is_some_and(|from| from.get() == Word::NIL)
        {
            let standard =
                standard_readtable(ctx, runtime).map_err(|error| read_error_to_object(&error))?;
            let from = arguments.first().ok_or(ObjectError::Layout)?;
            from.set(standard.object().as_word());
        }
        let from_word = arguments
            .first()
            .map(ncl_object::HeapRootedWord::get)
            .ok_or(ObjectError::Layout)?;
        if !readtablep(ctx, from_word) {
            return type_error(ctx, runtime, from_word);
        }
        let to_word = arguments
            .get(1)
            .map(ncl_object::HeapRootedWord::get)
            .ok_or(ObjectError::Layout)?;
        let destination = if to_word == Word::NIL {
            None
        } else if readtablep(ctx, to_word) {
            Some(Readtable::from_object(ObjectReadtable::from_word(to_word)))
        } else {
            return type_error(ctx, runtime, to_word);
        };
        let source = Readtable::from_object(ObjectReadtable::from_word(from_word));
        copy_readtable_to(ctx, runtime, source, destination).map(|table| table.object().as_word())
    })
}

fn current_readtable(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<Word, ObjectError> {
    let mut package = runtime
        .find_package(ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    with_root(ctx, &mut package, |ctx, package| {
        let (mut symbol, _) =
            Package::from_word(package.get()).intern(ctx, runtime, "*READTABLE*")?;
        with_root(ctx, &mut symbol, |ctx, symbol| {
            symbol_value(ctx, symbol.get())
        })
    })
}

fn type_error(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    mut datum: Word,
) -> Result<Word, ObjectError> {
    let converter = runtime.lisp_error_converter().ok_or(ObjectError::Layout)?;
    with_root(ctx, &mut datum, |ctx, datum| {
        let mut condition = converter(
            ctx,
            runtime,
            LispError::TypeError {
                datum: datum.get(),
                expected: ObjectType::Readtable,
            },
        )?;
        with_root(ctx, &mut condition, |ctx, condition| {
            ctx.set_pending_condition(condition.get());
            Err(ObjectError::TypeError)
        })
    })
}

const fn read_error_to_object(error: &ReadError) -> ObjectError {
    match error {
        ReadError::Object(error) => *error,
        ReadError::UnexpectedEof
        | ReadError::UnmatchedRightParen
        | ReadError::DotWithoutCdr
        | ReadError::InvalidSymbolToken(_)
        | ReadError::PackageNotFound(_)
        | ReadError::InvalidBase(_)
        | ReadError::InvalidDigit(_)
        | ReadError::InvalidNumber(_)
        | ReadError::NumberOutOfRange
        | ReadError::FloatFormatUnavailable(_)
        | ReadError::ReadEvalDisabled
        | ReadError::ReadEvalUnavailable
        | ReadError::InvalidCharacter
        | ReadError::UnknownCharacterName(_)
        | ReadError::UndefinedDispatchMacro(_)
        | ReadError::UninvocableMacroFunction(_)
        | ReadError::InvalidFeatureExpression
        | ReadError::ArraySyntax
        | ReadError::StructureSyntax
        | ReadError::PathnameSyntax
        | ReadError::NotDispatchMacro(_) => ObjectError::TypeError,
    }
}

fn identity_arguments(args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    (0..args.len())
        .map(|index| args.get(index).ok_or(ObjectError::TypeError))
        .collect()
}
