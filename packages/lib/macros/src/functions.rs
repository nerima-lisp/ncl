//! Small function builtins owned by the macro surface.

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, LambdaList, ObjectError, ObjectRef, Runtime, ThreadContext, Word,
    car, cdr, classify_object,
};

const VALUE: ncl_object::Parameter = ncl_object::Parameter {
    name: BuiltinName::new("VALUE"),
    ty: ncl_object::ParameterType::Any,
};

fn identity_builtin(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    args.required(0)
}

fn not_builtin(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(if args.required(0)? == Word::NIL {
        Word::TRUE
    } else {
        Word::NIL
    })
}

fn functionp_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let is_function = matches!(
        classify_object(ctx, args.required(0)?),
        ObjectRef::Function(_) | ObjectRef::Closure(_)
    );
    Ok(if is_function { Word::TRUE } else { Word::NIL })
}

#[allow(clippy::unnecessary_wraps)]
fn values_builtin(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let words = (0..args.len())
        .filter_map(|index| args.get(index))
        .collect::<Vec<_>>();
    values.set(&words);
    Ok(words.first().copied().unwrap_or(Word::NIL))
}

fn values_list_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let mut list = args.required(0)?;
    let mut result = Vec::new();
    while list != Word::NIL {
        if !list.is_cons() {
            return Err(ObjectError::TypeError);
        }
        result.push(car(ctx, list)?);
        list = cdr(ctx, list)?;
    }
    values.set(&result);
    Ok(result.first().copied().unwrap_or(Word::NIL))
}

pub fn register(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
    let direct = |function| {
        BuiltinImplementation::direct(
            Builtin {
                lambda_list: LambdaList::fixed(&[VALUE]),
                convention: BuiltinConvention::Direct(Arity::exact(1)),
            },
            function,
        )
    };
    for (name, function) in [
        ("IDENTITY", identity_builtin as ncl_object::RustBuiltin),
        ("NOT", not_builtin),
        ("FUNCTIONP", functionp_builtin),
        ("COMPILED-FUNCTION-P", functionp_builtin),
    ] {
        runtime.register_builtin(
            ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            direct(function),
        )?;
    }
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("VALUES")),
        BuiltinImplementation::adapted(
            Builtin {
                lambda_list: LambdaList::with_rest(&[], super::ENVIRONMENT),
                convention: BuiltinConvention::Adapted,
            },
            values_builtin,
            |args| {
                Ok((0..args.len())
                    .filter_map(|index| args.get(index))
                    .collect())
            },
        ),
    )?;
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("VALUES-LIST")),
        direct(values_list_builtin),
    )?;
    Ok(())
}
