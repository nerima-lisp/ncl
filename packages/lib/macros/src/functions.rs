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
        // `NULL` and `NOT` share an implementation per the standard: both
        // return `T` exactly when their argument is `NIL`.
        ("NULL", not_builtin),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Result<(Runtime, ThreadContext), ObjectError> {
        let runtime = Runtime::new()?;
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)?;
        register(&runtime, &mut ctx)?;
        Ok((runtime, ctx))
    }

    fn builtin(
        runtime: &Runtime,
        ctx: &mut ThreadContext,
        name: &str,
    ) -> Result<ncl_object::FunctionObject, ObjectError> {
        let function = runtime
            .function(ctx, "COMMON-LISP", name)
            .ok_or(ObjectError::TypeError)?;
        ncl_object::FunctionObject::try_from(function)
    }

    #[test]
    fn predicates_and_identity_return_their_expected_values() -> Result<(), ObjectError> {
        let (runtime, mut ctx) = setup()?;
        let identity = builtin(&runtime, &mut ctx, "IDENTITY")?;
        let not = builtin(&runtime, &mut ctx, "NOT")?;
        let null = builtin(&runtime, &mut ctx, "NULL")?;
        let functionp = builtin(&runtime, &mut ctx, "FUNCTIONP")?;

        assert_eq!(
            runtime.call_builtin(&mut ctx, identity, &[Word::fixnum(7)]),
            Ok(Word::fixnum(7))
        );
        assert_eq!(
            runtime.call_builtin(&mut ctx, not, &[Word::NIL]),
            Ok(Word::TRUE)
        );
        assert_eq!(
            runtime.call_builtin(&mut ctx, null, &[Word::fixnum(1)]),
            Ok(Word::NIL)
        );
        let values = builtin(&runtime, &mut ctx, "VALUES")?;
        assert_eq!(
            runtime.call_builtin(&mut ctx, functionp, &[values.as_word()]),
            Ok(Word::TRUE)
        );
        assert_eq!(
            runtime.call_builtin(&mut ctx, functionp, &[Word::fixnum(1)]),
            Ok(Word::NIL)
        );
        Ok(())
    }

    #[test]
    fn values_and_values_list_preserve_all_values() -> Result<(), ObjectError> {
        let (runtime, mut ctx) = setup()?;
        let values = builtin(&runtime, &mut ctx, "VALUES")?;
        assert_eq!(
            runtime.call_builtin(
                &mut ctx,
                values,
                &[Word::fixnum(2), Word::fixnum(4), Word::fixnum(6)],
            ),
            Ok(Word::fixnum(2))
        );
        assert_eq!(
            ctx.values(),
            &[Word::fixnum(2), Word::fixnum(4), Word::fixnum(6)]
        );
        assert_eq!(runtime.call_builtin(&mut ctx, values, &[]), Ok(Word::NIL));
        assert_eq!(ctx.values(), &[]);

        let first = ncl_object::make_cons(&mut ctx, &runtime, Word::fixnum(8), Word::NIL)?;
        let list = ncl_object::make_cons(&mut ctx, &runtime, Word::fixnum(7), first)?;
        let values_list = builtin(&runtime, &mut ctx, "VALUES-LIST")?;
        assert_eq!(
            runtime.call_builtin(&mut ctx, values_list, &[list]),
            Ok(Word::fixnum(7))
        );
        assert_eq!(ctx.values(), &[Word::fixnum(7), Word::fixnum(8)]);
        assert_eq!(
            runtime.call_builtin(&mut ctx, values_list, &[Word::fixnum(0)]),
            Err(ObjectError::TypeError)
        );
        Ok(())
    }

    #[test]
    fn required_arguments_are_checked() -> Result<(), ObjectError> {
        let (runtime, mut ctx) = setup()?;
        let identity = builtin(&runtime, &mut ctx, "IDENTITY")?;
        assert_eq!(
            runtime.call_builtin(&mut ctx, identity, &[]),
            Err(ObjectError::TypeError)
        );
        Ok(())
    }
}
