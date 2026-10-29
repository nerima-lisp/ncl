use ncl_compiler_front::ast::{Expr, Operator};
use ncl_compiler_front::env::LexicalEnv;
use ncl_compiler_front::{MacroCaller, SymbolRef};
use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinFunctionCaller, BuiltinIdentifier,
    BuiltinImplementation, BuiltinName, BuiltinPackage, FunctionArguments, FunctionCaller,
    FunctionDesignator, HeapRootedWord, LambdaList, MultipleValues, ObjectError, ObjectRef,
    Package, Parameter, ParameterType, Runtime as ObjectRuntime, ThreadContext, Word, car,
    classify_object, make_cons, string_length, string_ref, symbol_function, symbol_is_macro,
    symbol_name, symbol_package, symbol_value,
};

const FORM: Parameter = Parameter {
    name: BuiltinName::new("FORM"),
    ty: ParameterType::Any,
};
const ENVIRONMENT: Parameter = Parameter {
    name: BuiltinName::new("ENVIRONMENT"),
    ty: ParameterType::Any,
};

pub(crate) fn register(
    ctx: &mut ThreadContext,
    runtime: &ObjectRuntime,
) -> Result<(), ObjectError> {
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(
            BuiltinPackage::NclExt,
            BuiltinName::new("MACROEXPAND-1-ENVIRONMENT"),
        ),
        BuiltinImplementation::adapted(
            Builtin {
                lambda_list: LambdaList::fixed(&[FORM, ENVIRONMENT]),
                convention: BuiltinConvention::Adapted,
            },
            macroexpand_1_environment_builtin,
            |args| Ok(args.as_slice().to_vec()),
        )
        .with_nested_evaluation(),
    )?;
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(
            BuiltinPackage::NclExt,
            BuiltinName::new("MACROEXPAND-LEXICAL-EXPANDER"),
        ),
        BuiltinImplementation::adapted(
            Builtin {
                lambda_list: LambdaList::fixed(&[FORM, ENVIRONMENT]),
                convention: BuiltinConvention::Adapted,
            },
            macroexpand_lexical_expander_builtin,
            |args| Ok(args.as_slice().to_vec()),
        )
        .with_nested_evaluation(),
    )?;
    Ok(())
}

fn macroexpand_1_environment_builtin(
    ctx: &mut ThreadContext,
    object: &ObjectRuntime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let form = HeapRootedWord::new(ctx, args.required(0)?)?;
    let environment = HeapRootedWord::new(ctx, args.required(1)?)?;
    macroexpand_once(ctx, object, form.get(), environment.get(), values)
}

fn macroexpand_lexical_expander_builtin(
    ctx: &mut ThreadContext,
    object: &ObjectRuntime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let form = HeapRootedWord::new(ctx, args.required(0)?)?;
    let environment = HeapRootedWord::new(ctx, args.required(1)?)?;
    let lexical =
        crate::local_macro::macro_environment(environment.get()).ok_or(ObjectError::TypeError)?;
    let pointer = ctx.evaluator_runtime().ok_or(ObjectError::Layout)?;
    let evaluator = ncl_sys::with_opaque_mut(pointer, |runtime: &mut crate::Runtime| {
        runtime.evaluator.clone()
    });
    let expansion = expand_lexical_form(ctx, object, form.get(), &lexical, &evaluator)?;
    values.set(&[expansion]);
    Ok(expansion)
}

fn macroexpand_once(
    ctx: &mut ThreadContext,
    object: &ObjectRuntime,
    form: Word,
    environment: Word,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let form = HeapRootedWord::new(ctx, form)?;
    let environment = HeapRootedWord::new(ctx, environment)?;
    let lexical = crate::local_macro::macro_environment(environment.get());
    if matches!(classify_object(ctx, form.get()), ObjectRef::Symbol(_)) {
        if let (Some(lexical), Some(name)) = (lexical.as_ref(), symbol_reference(ctx, form.get())?)
            && lexical.lookup_variable(&name).is_none()
            && lexical.lookup_symbol_macro(&name).is_some()
        {
            let expander = lexical_expander_function(ctx, object)?;
            let expansion = invoke_macroexpand_hook(
                ctx,
                object,
                expander,
                form.get(),
                environment.get(),
                values,
            )?;
            values.set(&[expansion, Word::TRUE]);
            return Ok(expansion);
        }
        values.set(&[form.get(), Word::NIL]);
        return Ok(form.get());
    }
    if !matches!(classify_object(ctx, form.get()), ObjectRef::Cons(_)) {
        values.set(&[form.get(), Word::NIL]);
        return Ok(form.get());
    }
    let operator = HeapRootedWord::new(ctx, car(ctx, form.get())?)?;
    if !matches!(classify_object(ctx, operator.get()), ObjectRef::Symbol(_)) {
        values.set(&[form.get(), Word::NIL]);
        return Ok(form.get());
    }
    let name = symbol_reference(ctx, operator.get())?.ok_or(ObjectError::TypeError)?;
    if let Some(lexical) = lexical.as_ref() {
        if lexical.lookup_function(&name) {
            values.set(&[form.get(), Word::NIL]);
            return Ok(form.get());
        }
        if lexical.lookup_macro(&name).is_some() {
            let expander = lexical_expander_function(ctx, object)?;
            let expansion = invoke_macroexpand_hook(
                ctx,
                object,
                expander,
                form.get(),
                environment.get(),
                values,
            )?;
            values.set(&[expansion, Word::TRUE]);
            return Ok(expansion);
        }
    }
    if !symbol_is_macro(ctx, operator.get())? {
        values.set(&[form.get(), Word::NIL]);
        return Ok(form.get());
    }
    let macro_function = HeapRootedWord::new(ctx, symbol_function(ctx, operator.get())?)?;
    let expansion = invoke_macroexpand_hook(
        ctx,
        object,
        macro_function.get(),
        form.get(),
        environment.get(),
        values,
    )?;
    values.set(&[expansion, Word::TRUE]);
    Ok(expansion)
}

fn lexical_expander_function(
    ctx: &mut ThreadContext,
    object: &ObjectRuntime,
) -> Result<Word, ObjectError> {
    let package = object
        .find_package(ctx, "NCL-EXT")
        .ok_or(ObjectError::Layout)?;
    let symbol = Package::from_word(package)
        .intern(ctx, object, "MACROEXPAND-LEXICAL-EXPANDER")?
        .0;
    symbol_function(ctx, symbol)
}

fn invoke_macroexpand_hook(
    ctx: &mut ThreadContext,
    object: &ObjectRuntime,
    expander: Word,
    form: Word,
    environment: Word,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let hook_package = object
        .find_package(ctx, "COMMON-LISP")
        .ok_or(ObjectError::Layout)?;
    let hook_symbol = Package::from_word(hook_package)
        .intern(ctx, object, "*MACROEXPAND-HOOK*")?
        .0;
    let hook = HeapRootedWord::new(ctx, symbol_value(ctx, hook_symbol)?)?;
    let expansion = ncl_object::with_roots(
        ctx,
        &[hook.get(), expander, form, environment],
        |ctx, roots| {
            let designator = FunctionDesignator::try_from_word(
                ctx,
                **roots.first().ok_or(ObjectError::Layout)?,
            )?;
            let arguments = [
                **roots.get(1).ok_or(ObjectError::Layout)?,
                **roots.get(2).ok_or(ObjectError::Layout)?,
                **roots.get(3).ok_or(ObjectError::Layout)?,
            ];
            let mut caller = BuiltinFunctionCaller;
            caller.call_function(
                ctx,
                object,
                designator,
                FunctionArguments::new(&arguments),
                values,
            )
        },
    )?;
    Ok(expansion)
}

fn expand_lexical_form(
    ctx: &mut ThreadContext,
    object: &ObjectRuntime,
    form: Word,
    lexical: &LexicalEnv,
    evaluator: &crate::evaluator::EvaluatorService,
) -> Result<Word, ObjectError> {
    let form = HeapRootedWord::new(ctx, form)?;
    if matches!(classify_object(ctx, form.get()), ObjectRef::Symbol(_)) {
        if let Some(name) = symbol_reference(ctx, form.get())?
            && lexical.lookup_variable(&name).is_none()
            && let Some(binding) = lexical.lookup_symbol_macro(&name)
        {
            return expression_to_word(ctx, object, &binding.expansion);
        }
        return Ok(form.get());
    }
    if !matches!(classify_object(ctx, form.get()), ObjectRef::Cons(_)) {
        return Ok(form.get());
    }
    let operator = HeapRootedWord::new(ctx, car(ctx, form.get())?)?;
    if !matches!(classify_object(ctx, operator.get()), ObjectRef::Symbol(_)) {
        return Ok(form.get());
    }
    let Some(name) = symbol_reference(ctx, operator.get())? else {
        return Ok(form.get());
    };
    if lexical.lookup_function(&name) {
        return Ok(form.get());
    }
    if let Some(definition) = lexical.lookup_macro(&name) {
        let mut caller = crate::support::RuntimeMacroCaller::new(evaluator.clone());
        return caller
            .call_local_macro_in_environment(ctx, object, definition, form.get(), lexical)
            .map_err(|_| ObjectError::TypeError);
    }
    Ok(form.get())
}

fn symbol_reference(ctx: &ThreadContext, symbol: Word) -> Result<Option<SymbolRef>, ObjectError> {
    let package = symbol_package(ctx, symbol)?;
    if package == Word::NIL {
        return Ok(None);
    }
    let package_name_word = Package::from_word(package).name(ctx)?;
    let package_name = (0..string_length(ctx, package_name_word)?)
        .map(|index| string_ref(ctx, package_name_word, index))
        .collect::<Result<String, _>>()?;
    let name_word = symbol_name(ctx, symbol)?;
    let name = (0..string_length(ctx, name_word)?)
        .map(|index| string_ref(ctx, name_word, index))
        .collect::<Result<String, _>>()?;
    Ok(Some(SymbolRef::interned(package_name, name)))
}

fn symbol_word(
    ctx: &mut ThreadContext,
    runtime: &ObjectRuntime,
    symbol: &SymbolRef,
) -> Result<Word, ObjectError> {
    let package = symbol.package_name().ok_or(ObjectError::TypeError)?;
    let package_word = runtime
        .find_package(ctx, package)
        .ok_or(ObjectError::Layout)?;
    Ok(Package::from_word(package_word)
        .intern(ctx, runtime, &symbol.name)?
        .0)
}

fn expression_to_word(
    ctx: &mut ThreadContext,
    runtime: &ObjectRuntime,
    expression: &Expr,
) -> Result<Word, ObjectError> {
    match expression {
        Expr::Constant(literal) => {
            let datum_word = crate::local_macro::macroexpand_literal(ctx, runtime, literal)
                .map_err(|_| ObjectError::TypeError)?;
            let datum = HeapRootedWord::new(ctx, datum_word)?;
            if literal.is_self_evaluating() {
                Ok(datum.get())
            } else {
                let quote_name = SymbolRef::interned("COMMON-LISP", "QUOTE");
                let quote_word = symbol_word(ctx, runtime, &quote_name)?;
                let quote = HeapRootedWord::new(ctx, quote_word)?;
                let tail = make_cons(ctx, runtime, datum.get(), Word::NIL)?;
                let tail = HeapRootedWord::new(ctx, tail)?;
                make_cons(ctx, runtime, quote.get(), tail.get())
            }
        }
        Expr::Variable(name) => symbol_word(ctx, runtime, name),
        Expr::Call {
            operator: Operator::Name(name),
            arguments,
        } => {
            let operator_word = symbol_word(ctx, runtime, name)?;
            let operator = HeapRootedWord::new(ctx, operator_word)?;
            let mut tail = HeapRootedWord::new(ctx, Word::NIL)?;
            for argument in arguments.iter().rev() {
                let argument_word = expression_to_word(ctx, runtime, argument)?;
                let argument = HeapRootedWord::new(ctx, argument_word)?;
                let pair = make_cons(ctx, runtime, argument.get(), tail.get())?;
                tail = HeapRootedWord::new(ctx, pair)?;
            }
            make_cons(ctx, runtime, operator.get(), tail.get())
        }
        Expr::If {
            test,
            then,
            otherwise,
        } => {
            let test_word = expression_to_word(ctx, runtime, test)?;
            let test = HeapRootedWord::new(ctx, test_word)?;
            let then_word = expression_to_word(ctx, runtime, then)?;
            let then = HeapRootedWord::new(ctx, then_word)?;
            let mut tail = if let Some(otherwise) = otherwise {
                let otherwise_word = expression_to_word(ctx, runtime, otherwise)?;
                let otherwise = HeapRootedWord::new(ctx, otherwise_word)?;
                let pair = make_cons(ctx, runtime, otherwise.get(), Word::NIL)?;
                HeapRootedWord::new(ctx, pair)?
            } else {
                HeapRootedWord::new(ctx, Word::NIL)?
            };
            let pair = make_cons(ctx, runtime, then.get(), tail.get())?;
            tail = HeapRootedWord::new(ctx, pair)?;
            let pair = make_cons(ctx, runtime, test.get(), tail.get())?;
            tail = HeapRootedWord::new(ctx, pair)?;
            let operator_name = SymbolRef::interned("COMMON-LISP", "IF");
            let operator_word = symbol_word(ctx, runtime, &operator_name)?;
            let operator = HeapRootedWord::new(ctx, operator_word)?;
            make_cons(ctx, runtime, operator.get(), tail.get())
        }
        _ => Err(ObjectError::TypeError),
    }
}
