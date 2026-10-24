//! Runtime expansion of registered derived type specifiers.

use ncl_object::{
    BuiltinFunctionCaller, FunctionArguments, FunctionCaller, FunctionDesignator, HeapRootedWord,
    MultipleValues, ObjectError, ObjectRef, Package, Runtime, ThreadContext, Word, car, cdr,
    classify_object, make_cons, string_length, string_ref, symbol_name, symbol_plist,
};

use crate::TypeError;

const EXPANSION_LIMIT: usize = 128;

/// Calls a registered DEFTYPE expander with its whole type form.
pub trait DeftypeExpanderCaller {
    /// Invoke the expander with the function object and complete type form.
    fn call_expander(
        &mut self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        function: Word,
        whole_form: Word,
    ) -> Result<Word, ObjectError>;
}

struct BuiltinDeftypeExpanderCaller;

impl DeftypeExpanderCaller for BuiltinDeftypeExpanderCaller {
    fn call_expander(
        &mut self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        function: Word,
        whole_form: Word,
    ) -> Result<Word, ObjectError> {
        let function = FunctionDesignator::try_from_word(ctx, function)?;
        let whole_form = HeapRootedWord::new(ctx, whole_form)?;
        let arguments = [whole_form.get()];
        let mut function_caller = BuiltinFunctionCaller;
        let mut values = MultipleValues::default();
        function_caller.call_function(
            ctx,
            runtime,
            function,
            FunctionArguments::new(&arguments),
            &mut values,
        )
    }
}

struct TypeArgumentExpansion<'a> {
    operator: &'a str,
    property: Word,
    depth: usize,
    caller: &'a mut dyn DeftypeExpanderCaller,
}

/// Recursively expand registered derived type specifiers in a type form.
///
/// Expander functions are stored on their type name symbols under
/// `NCL::DEFTYPE-EXPANDER`. The original type form is passed as one argument,
/// matching the macro lambda-list calling convention used by `DEFTYPE`.
pub fn expand_deftype_specifier(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    specifier: Word,
) -> Result<Word, TypeError> {
    let mut caller = BuiltinDeftypeExpanderCaller;
    expand_deftype_specifier_with(ctx, runtime, specifier, &mut caller)
}

/// Expand a type specifier using the supplied function caller for expanders.
///
/// The runtime uses this entry point while compiling declarations, where a
/// Lisp expander must run with the runtime's native invocation context.
pub fn expand_deftype_specifier_with(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    specifier: Word,
    caller: &mut impl DeftypeExpanderCaller,
) -> Result<Word, TypeError> {
    let specifier = HeapRootedWord::new(ctx, specifier)?;
    let package = runtime.ensure_package(ctx, "NCL")?;
    let property = Package::from_word(package)
        .intern(ctx, runtime, "DEFTYPE-EXPANDER")?
        .0;
    let property = HeapRootedWord::new(ctx, property)?;
    let (expanded, _) = expand_form(ctx, runtime, specifier.get(), property.get(), 0, caller)?;
    Ok(expanded.get())
}

fn expand_form(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    form: Word,
    property: Word,
    depth: usize,
    caller: &mut dyn DeftypeExpanderCaller,
) -> Result<(HeapRootedWord, bool), TypeError> {
    let form = HeapRootedWord::new(ctx, form)?;
    let property = HeapRootedWord::new(ctx, property)?;
    if depth > EXPANSION_LIMIT {
        return Err(TypeError::InvalidSpecifier(form.get()));
    }

    if matches!(classify_object(ctx, form.get()), ObjectRef::Symbol(_)) {
        if let Some(expander) = find_expander(ctx, form.get(), property.get())? {
            let whole = list(ctx, runtime, &[form.get()])?;
            let whole = HeapRootedWord::new(ctx, whole)?;
            let expansion = call_expander(ctx, runtime, expander.get(), whole.get(), caller)?;
            return expand_form(ctx, runtime, expansion, property.get(), depth + 1, caller);
        }
        return Ok((form, false));
    }

    if !form.get().is_cons() {
        return Ok((form, false));
    }
    let operator_word = car(ctx, form.get())?;
    let operator = HeapRootedWord::new(ctx, operator_word)?;
    if !matches!(classify_object(ctx, operator.get()), ObjectRef::Symbol(_)) {
        return Ok((form, false));
    }
    if let Some(expander) = find_expander(ctx, operator.get(), property.get())? {
        let expansion = call_expander(ctx, runtime, expander.get(), form.get(), caller)?;
        return expand_form(ctx, runtime, expansion, property.get(), depth + 1, caller);
    }

    let operator_name = symbol_name_string(ctx, operator.get())?;
    let tail_word = cdr(ctx, form.get())?;
    let tail = HeapRootedWord::new(ctx, tail_word)?;
    let (expanded_tail, changed) = expand_type_arguments(
        ctx,
        runtime,
        tail.get(),
        &operator_name,
        property.get(),
        depth + 1,
        caller,
    )?;
    if !changed {
        return Ok((form, false));
    }
    let rebuilt = make_cons(ctx, runtime, operator.get(), expanded_tail.get())?;
    Ok((HeapRootedWord::new(ctx, rebuilt)?, true))
}

fn expand_type_arguments(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    tail: Word,
    operator: &str,
    property: Word,
    depth: usize,
    caller: &mut dyn DeftypeExpanderCaller,
) -> Result<(HeapRootedWord, bool), TypeError> {
    let tail = HeapRootedWord::new(ctx, tail)?;
    let property = HeapRootedWord::new(ctx, property)?;
    if tail.get() == Word::NIL || !tail.get().is_cons() {
        return Ok((tail, false));
    }
    let first_word = car(ctx, tail.get())?;
    let first = HeapRootedWord::new(ctx, first_word)?;
    let rest_word = cdr(ctx, tail.get())?;
    let rest = HeapRootedWord::new(ctx, rest_word)?;
    let index = 1;
    let (expanded_first, first_changed) = if operator == "FUNCTION" && index == 1 {
        expand_function_parameters(ctx, runtime, first.get(), property.get(), depth + 1, caller)?
    } else if is_type_argument(operator, index) {
        expand_form(ctx, runtime, first.get(), property.get(), depth + 1, caller)?
    } else {
        (first, false)
    };
    let mut expansion = TypeArgumentExpansion {
        operator,
        property: property.get(),
        depth,
        caller,
    };
    let (expanded_rest, rest_changed) =
        expand_type_argument_tail(ctx, runtime, rest.get(), index + 1, &mut expansion)?;
    if !first_changed && !rest_changed {
        return Ok((tail, false));
    }
    let rebuilt = make_cons(ctx, runtime, expanded_first.get(), expanded_rest.get())?;
    Ok((HeapRootedWord::new(ctx, rebuilt)?, true))
}

fn expand_type_argument_tail(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    tail: Word,
    index: usize,
    expansion: &mut TypeArgumentExpansion<'_>,
) -> Result<(HeapRootedWord, bool), TypeError> {
    let tail = HeapRootedWord::new(ctx, tail)?;
    let property = HeapRootedWord::new(ctx, expansion.property)?;
    if tail.get() == Word::NIL || !tail.get().is_cons() {
        return Ok((tail, false));
    }
    let first_word = car(ctx, tail.get())?;
    let first = HeapRootedWord::new(ctx, first_word)?;
    let rest_word = cdr(ctx, tail.get())?;
    let rest = HeapRootedWord::new(ctx, rest_word)?;
    let (expanded_first, first_changed) = if is_type_argument(expansion.operator, index) {
        expand_form(
            ctx,
            runtime,
            first.get(),
            property.get(),
            expansion.depth + 1,
            expansion.caller,
        )?
    } else {
        (first, false)
    };
    let (expanded_rest, rest_changed) =
        expand_type_argument_tail(ctx, runtime, rest.get(), index + 1, expansion)?;
    if !first_changed && !rest_changed {
        return Ok((tail, false));
    }
    let rebuilt = make_cons(ctx, runtime, expanded_first.get(), expanded_rest.get())?;
    Ok((HeapRootedWord::new(ctx, rebuilt)?, true))
}

fn expand_function_parameters(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    parameters: Word,
    property: Word,
    depth: usize,
    caller: &mut dyn DeftypeExpanderCaller,
) -> Result<(HeapRootedWord, bool), TypeError> {
    let parameters = HeapRootedWord::new(ctx, parameters)?;
    let property = HeapRootedWord::new(ctx, property)?;
    if !parameters.get().is_cons() {
        return Ok((parameters, false));
    }
    let (expanded, changed) = expand_parameter_tail(
        ctx,
        runtime,
        parameters.get(),
        property.get(),
        depth + 1,
        caller,
    )?;
    Ok((expanded, changed))
}

fn expand_parameter_tail(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    tail: Word,
    property: Word,
    depth: usize,
    caller: &mut dyn DeftypeExpanderCaller,
) -> Result<(HeapRootedWord, bool), TypeError> {
    let tail = HeapRootedWord::new(ctx, tail)?;
    let property = HeapRootedWord::new(ctx, property)?;
    if tail.get() == Word::NIL || !tail.get().is_cons() {
        return Ok((tail, false));
    }
    let first_word = car(ctx, tail.get())?;
    let first = HeapRootedWord::new(ctx, first_word)?;
    let rest_word = cdr(ctx, tail.get())?;
    let rest = HeapRootedWord::new(ctx, rest_word)?;
    let (expanded_first, first_changed) =
        expand_form(ctx, runtime, first.get(), property.get(), depth + 1, caller)?;
    let (expanded_rest, rest_changed) =
        expand_parameter_tail(ctx, runtime, rest.get(), property.get(), depth + 1, caller)?;
    if !first_changed && !rest_changed {
        return Ok((tail, false));
    }
    let rebuilt = make_cons(ctx, runtime, expanded_first.get(), expanded_rest.get())?;
    Ok((HeapRootedWord::new(ctx, rebuilt)?, true))
}

fn is_type_argument(operator: &str, index: usize) -> bool {
    (matches!(operator, "AND" | "OR" | "VALUES") && index >= 1)
        || (matches!(operator, "NOT" | "CONS") && (1..=2).contains(&index))
        || (matches!(operator, "ARRAY" | "SIMPLE-ARRAY" | "VECTOR") && index == 1)
        || (operator == "FUNCTION" && index == 2)
}

fn find_expander(
    ctx: &mut ThreadContext,
    symbol: Word,
    property: Word,
) -> Result<Option<HeapRootedWord>, TypeError> {
    let plist_word = symbol_plist(ctx, symbol)?;
    let plist = HeapRootedWord::new(ctx, plist_word)?;
    let mut cursor = plist.get();
    while cursor != Word::NIL {
        if !cursor.is_cons() {
            return Err(TypeError::InvalidForm);
        }
        let key = car(ctx, cursor)?;
        let values = cdr(ctx, cursor)?;
        if !values.is_cons() {
            return Err(TypeError::InvalidForm);
        }
        if key == property {
            let value = car(ctx, values)?;
            return Ok(Some(HeapRootedWord::new(ctx, value)?));
        }
        cursor = cdr(ctx, values)?;
    }
    Ok(None)
}

fn call_expander(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: Word,
    whole_form: Word,
    caller: &mut dyn DeftypeExpanderCaller,
) -> Result<Word, TypeError> {
    caller
        .call_expander(ctx, runtime, function, whole_form)
        .map_err(TypeError::from)
}

fn symbol_name_string(ctx: &ThreadContext, symbol: Word) -> Result<String, TypeError> {
    let name = symbol_name(ctx, symbol)?;
    let length = string_length(ctx, name)?;
    let mut value = String::with_capacity(length);
    for index in 0..length {
        value.push(string_ref(ctx, name, index)?);
    }
    Ok(value.to_ascii_uppercase())
}

fn list(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result<Word, ObjectError> {
    ncl_object::with_roots(ctx, values, |ctx, values| {
        let mut result = Word::NIL;
        for value in values.iter().rev() {
            result = ncl_object::with_root(ctx, &mut result, |ctx, result| {
                make_cons(ctx, runtime, **value, *result)
            })?;
        }
        Ok(result)
    })
}
