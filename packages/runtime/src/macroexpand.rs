use ncl_compiler_front::env::{LexicalEnv, OperatorBinding, VariableOrSymbolMacro};
use ncl_compiler_front::form::UninternedTable;
use ncl_compiler_front::{MacroCaller, SymbolRef};
use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinFunctionCaller, BuiltinIdentifier,
    BuiltinImplementation, BuiltinName, BuiltinPackage, FunctionArguments, FunctionCaller,
    FunctionDesignator, HeapRootedWord, LambdaList, MultipleValues, ObjectError, ObjectRef,
    Package, Parameter, ParameterType, Runtime as ObjectRuntime, ThreadContext, Word, car,
    classify_object, symbol_function, symbol_is_macro, symbol_value,
};

const FORM: Parameter = Parameter {
    name: BuiltinName::new("FORM"),
    ty: ParameterType::Any,
};
const ENVIRONMENT: Parameter = Parameter {
    name: BuiltinName::new("ENVIRONMENT"),
    ty: ParameterType::Any,
};

pub fn register(ctx: &mut ThreadContext, runtime: &ObjectRuntime) -> Result<(), ObjectError> {
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
            && matches!(
                lexical.lookup_variable_or_symbol_macro(&name),
                Some(VariableOrSymbolMacro::SymbolMacro(_))
            )
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
        match lexical.lookup_operator(&name) {
            Some(OperatorBinding::Function) => {
                values.set(&[form.get(), Word::NIL]);
                return Ok(form.get());
            }
            Some(OperatorBinding::Macro(_)) => {
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
            None => {}
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
            && matches!(
                lexical.lookup_variable_or_symbol_macro(&name),
                Some(VariableOrSymbolMacro::SymbolMacro(_))
            )
        {
            return lexical
                .lookup_symbol_macro_source(&name)
                .ok_or(ObjectError::TypeError);
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
    match lexical.lookup_operator(&name) {
        Some(OperatorBinding::Function) | None => Ok(form.get()),
        Some(OperatorBinding::Macro(definition)) => {
            let mut caller = crate::support::RuntimeMacroCaller::new(evaluator.clone());
            caller
                .call_local_macro_in_environment(ctx, object, definition, form.get(), lexical)
                .map_err(|_| ObjectError::TypeError)
        }
    }
}

fn symbol_reference(ctx: &ThreadContext, symbol: Word) -> Result<Option<SymbolRef>, ObjectError> {
    ncl_compiler_front::form::symbol_ref(ctx, &mut UninternedTable::new(), symbol)
        .map(Some)
        .map_err(|error| match error {
            ncl_compiler_front::FrontError::Object(error) => error,
            _ => ObjectError::TypeError, // check-added-lines: allow(wildcard) FrontError is non_exhaustive.
        })
}
