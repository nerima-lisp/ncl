//! Expanders for Common Lisp definition forms.
#![allow(clippy::redundant_pub_crate)]
//!
//! These expanders lower definitions to ordinary forms so the evaluator
//! performs the definition through the runtime-owned generalized places.
#![allow(missing_docs, clippy::missing_errors_doc)]

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, FunctionObject, LambdaList, ObjectError, Runtime, ThreadContext,
    Word, make_cons, rplacd, set_symbol_constant, set_symbol_special, symbol_function,
    symbol_name, symbol_plist,
};

use crate::form::{elements, list, symbol};

const DEFINITION_PROPERTY: &str = "NCL::DEFINITION";
const CL: BuiltinPackage = BuiltinPackage::CommonLisp;

const SYMBOL: ncl_object::Parameter = ncl_object::Parameter {
    name: BuiltinName::new("SYMBOL"),
    ty: ncl_object::ParameterType::Any,
};
const PROPERTY: ncl_object::Parameter = ncl_object::Parameter {
    name: BuiltinName::new("PROPERTY"),
    ty: ncl_object::ParameterType::Any,
};
const VALUE: ncl_object::Parameter = ncl_object::Parameter {
    name: BuiltinName::new("VALUE"),
    ty: ncl_object::ParameterType::Any,
};

fn checked_symbol(ctx: &ThreadContext, value: Word) -> Result<Word, ObjectError> {
    symbol_name(ctx, value).map(|_| value)
}

fn checked_function(value: Word) -> Result<Word, ObjectError> {
    FunctionObject::try_from(value)
        .map(Word::from)
        .map_err(|_| ObjectError::TypeError)
}

fn fdefinition_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let name = checked_symbol(ctx, args.required(0)?)?;
    let function = symbol_function(ctx, name)?;
    FunctionObject::try_from(function)
        .map(Word::from)
        .map_err(|_| ObjectError::Unbound)
}

fn set_fdefinition_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let name = checked_symbol(ctx, args.required(0)?)?;
    let function = checked_function(args.required(1)?)?;
    ctx.write_object_slot(name, ncl_object::symbol_offset::FUNCTION, function)?;
    Ok(function)
}

fn macro_function_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let name = checked_symbol(ctx, args.required(0)?)?;
    let function = symbol_function(ctx, name)?;
    Ok(match FunctionObject::try_from(function) {
        Ok(function) => Word::from(function),
        Err(_) => Word::NIL,
    })
}

fn set_macro_function_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let name = checked_symbol(ctx, args.required(0)?)?;
    let function = checked_function(args.required(1)?)?;
    ctx.write_object_slot(name, ncl_object::symbol_offset::FUNCTION, function)?;
    Ok(function)
}

fn get_property(ctx: &mut ThreadContext, name: Word, property: Word) -> Result<Word, ObjectError> {
    checked_symbol(ctx, name)?;
    let mut plist = symbol_plist(ctx, name)?;
    while plist != Word::NIL {
        if !plist.is_cons() {
            return Err(ObjectError::TypeError);
        }
        let key = ncl_object::car(ctx, plist)?;
        let rest = ncl_object::cdr(ctx, plist)?;
        if !rest.is_cons() {
            return Err(ObjectError::TypeError);
        }
        if key == property {
            return ncl_object::car(ctx, rest);
        }
        plist = ncl_object::cdr(ctx, rest)?;
    }
    Ok(Word::NIL)
}

fn get_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    get_property(ctx, args.required(0)?, args.required(1)?)
}

fn set_get_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let name = checked_symbol(ctx, args.required(0)?)?;
    let property = args.required(1)?;
    let value = args.required(2)?;
    let mut plist = symbol_plist(ctx, name)?;
    while plist != Word::NIL {
        if !plist.is_cons() {
            return Err(ObjectError::TypeError);
        }
        let rest = ncl_object::cdr(ctx, plist)?;
        if !rest.is_cons() {
            return Err(ObjectError::TypeError);
        }
        if ncl_object::car(ctx, plist)? == property {
            let value_cell = ncl_object::cdr(ctx, rest)?;
            let replacement = make_cons(ctx, runtime, value, value_cell)?;
            rplacd(ctx, rest, replacement)?;
            return Ok(value);
        }
        plist = ncl_object::cdr(ctx, rest)?;
    }
    let value_cell = make_cons(ctx, runtime, value, Word::NIL)?;
    let entry = make_cons(ctx, runtime, property, value_cell)?;
    let old_plist = symbol_plist(ctx, name)?;
    let next = make_cons(ctx, runtime, entry, old_plist)?;
    ctx.write_object_slot(name, ncl_object::symbol_offset::PLIST, next)?;
    Ok(value)
}

fn place_form(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    operator: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let operator = symbol(ctx, runtime, operator)?;
    list(ctx, runtime, &[operator, args[0]])
}

fn fdefinition_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<ncl_object::SetfExpansion, ObjectError> {
    if args.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    let store = symbol(ctx, runtime, "NCL::FDEFINITION-SET")?;
    let variable = symbol(ctx, runtime, "NCL::STORE")?;
    Ok(ncl_object::SetfExpansion {
        temporary_variables: Vec::new(),
        value_forms: Vec::new(),
        store_variables: vec![variable],
        store_form: list(ctx, runtime, &[store, args[0], variable])?,
        access_form: place_form(ctx, runtime, "FDEFINITION", args)?,
    })
}

fn macro_function_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<ncl_object::SetfExpansion, ObjectError> {
    if args.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    let store = symbol(ctx, runtime, "NCL::MACRO-FUNCTION-SET")?;
    let variable = symbol(ctx, runtime, "NCL::STORE")?;
    Ok(ncl_object::SetfExpansion {
        temporary_variables: Vec::new(),
        value_forms: Vec::new(),
        store_variables: vec![variable],
        store_form: list(ctx, runtime, &[store, args[0], variable])?,
        access_form: place_form(ctx, runtime, "MACRO-FUNCTION", args)?,
    })
}

fn get_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<ncl_object::SetfExpansion, ObjectError> {
    if args.len() != 2 {
        return Err(ObjectError::TypeError);
    }
    let store = symbol(ctx, runtime, "NCL::GET-SET")?;
    let variable = symbol(ctx, runtime, "NCL::STORE")?;
    Ok(ncl_object::SetfExpansion {
        temporary_variables: Vec::new(),
        value_forms: Vec::new(),
        store_variables: vec![variable],
        store_form: list(ctx, runtime, &[store, args[0], args[1], variable])?,
        access_form: {
            let get = symbol(ctx, runtime, "GET")?;
            list(ctx, runtime, &[get, args[0], args[1]])?
        },
    })
}

pub fn register_runtime_support(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    for (name, place) in [
        ("FDEFINITION", fdefinition_place as ncl_object::PlaceExpander),
        ("MACRO-FUNCTION", macro_function_place as ncl_object::PlaceExpander),
        ("GET", get_place as ncl_object::PlaceExpander),
    ] {
        let symbol = symbol(ctx, runtime, name)?;
        runtime.register_place_expander(ctx, symbol, place)?;
    }
    let builtins: &[(&str, ncl_object::RustBuiltin, LambdaList)] = &[
        ("FDEFINITION", fdefinition_builtin, LambdaList::fixed(&[SYMBOL])),
        ("NCL::FDEFINITION-SET", set_fdefinition_builtin, LambdaList::fixed(&[SYMBOL, VALUE])),
        ("MACRO-FUNCTION", macro_function_builtin, LambdaList::fixed(&[SYMBOL])),
        ("NCL::MACRO-FUNCTION-SET", set_macro_function_builtin, LambdaList::fixed(&[SYMBOL, VALUE])),
        ("GET", get_builtin, LambdaList::fixed(&[SYMBOL, PROPERTY])),
        ("NCL::GET-SET", set_get_builtin, LambdaList::fixed(&[SYMBOL, PROPERTY, VALUE])),
    ];
    for (builtin, callback, params) in builtins {
        let (package, builtin_name) = match (*builtin).split_once("::") {
            Some(parts) => parts,
            None => ("COMMON-LISP", *builtin),
        };
        let package = match package {
            "COMMON-LISP" => CL,
            "NCL" => BuiltinPackage::NclExt,
            _ => return Err(ObjectError::TypeError),
        };
        runtime.register_builtin(
            ctx,
            BuiltinIdentifier::new(package, BuiltinName::new(builtin_name)),
            BuiltinImplementation::direct(
                Builtin {
                    lambda_list: *params,
                    convention: BuiltinConvention::Direct(Arity::exact(
                        u8::try_from(params.required.len()).map_err(|_| ObjectError::Layout)?,
                    )),
                },
                *callback,
            ),
        )?;
    }
    Ok(())
}

#[cfg(test)]
type DefinitionCallback = fn(
    &Runtime,
    &mut ThreadContext,
    &[Word],
    &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError>;

#[cfg(test)]
fn callback_for(name: &str) -> Option<DefinitionCallback> {
    Some(match name {
        "DEFUN" => defun,
        "DEFMACRO" => defmacro,
        "DEFVAR" => defvar,
        "DEFPARAMETER" => defparameter,
        "DEFCONSTANT" => defconstant,
        "DEFINE-SYMBOL-MACRO" => define_symbol_macro,
        "DEFINE-COMPILER-MACRO" => define_compiler_macro,
        "DEFSETF" => defsetf,
        "DEFINE-SETF-EXPANDER" => define_setf_expander,
        _ => return None,
    })
}

/// Return the callback for a definition macro, if `name` is one of ours.
fn form_elements(ctx: &mut ThreadContext, form: Word) -> Result<Vec<Word>, ObjectError> {
    elements(ctx, form)
}

fn required(elements: &[Word], index: usize) -> Result<Word, ObjectError> {
    elements.get(index).copied().ok_or(ObjectError::TypeError)
}

fn ensure_symbol(ctx: &ThreadContext, value: Word) -> Result<Word, ObjectError> {
    ncl_object::symbol_name(ctx, value).map(|_| value)
}

fn ensure_form_operator(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    parts: &[Word],
    name: &str,
) -> Result<(), ObjectError> {
    if required(parts, 0)? != symbol(ctx, runtime, name)? {
        return Err(ObjectError::TypeError);
    }
    Ok(())
}

fn ensure_list(ctx: &mut ThreadContext, value: Word) -> Result<(), ObjectError> {
    elements(ctx, value).map(|_| ())
}

fn quote(ctx: &mut ThreadContext, runtime: &Runtime, value: Word) -> Result<Word, ObjectError> {
    let quote = symbol(ctx, runtime, "QUOTE")?;
    list(ctx, runtime, &[quote, value])
}

fn progn_with_definition(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: Word,
    operation: Word,
) -> Result<Word, ObjectError> {
    let progn = symbol(ctx, runtime, "PROGN")?;
    let quoted_name = quote(ctx, runtime, name)?;
    list(ctx, runtime, &[progn, operation, quoted_name])
}

fn function_definition(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: Word,
    accessor: &str,
    lambda_list: Word,
    body: &[Word],
) -> Result<Word, ObjectError> {
    let lambda_symbol = symbol(ctx, runtime, "LAMBDA")?;
    let lambda = list(ctx, runtime, &[lambda_symbol, lambda_list])?;
    let lambda = append(ctx, runtime, lambda, body)?;
    let function_symbol = symbol(ctx, runtime, "FUNCTION")?;
    let function = list(ctx, runtime, &[function_symbol, lambda])?;
    let accessor_symbol = symbol(ctx, runtime, accessor)?;
    let quoted_name = quote(ctx, runtime, name)?;
    let place = list(ctx, runtime, &[accessor_symbol, quoted_name])?;
    let setf_symbol = symbol(ctx, runtime, "SETF")?;
    let operation = list(ctx, runtime, &[setf_symbol, place, function])?;
    progn_with_definition(ctx, runtime, name, operation)
}

fn property_definition(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: Word,
    property: &str,
    value: Word,
) -> Result<Word, ObjectError> {
    let get_symbol = symbol(ctx, runtime, "GET")?;
    let quoted_name = quote(ctx, runtime, name)?;
    let property_symbol = symbol(ctx, runtime, property)?;
    let place = list(ctx, runtime, &[get_symbol, quoted_name, property_symbol])?;
    let setf_symbol = symbol(ctx, runtime, "SETF")?;
    let operation = list(ctx, runtime, &[setf_symbol, place, value])?;
    progn_with_definition(ctx, runtime, name, operation)
}

fn append(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    mut head: Word,
    tail: &[Word],
) -> Result<Word, ObjectError> {
    if tail.is_empty() {
        return Ok(head);
    }
    let mut values = form_elements(ctx, head)?;
    values.extend_from_slice(tail);
    head = list(ctx, runtime, &values)?;
    Ok(head)
}

fn defun(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let parts = form_elements(ctx, required(args, 0)?)?;
    ensure_form_operator(ctx, runtime, &parts, "DEFUN")?;
    let name = ensure_symbol(ctx, required(&parts, 1)?)?;
    let lambda_list = required(&parts, 2)?;
    ensure_list(ctx, lambda_list)?;
    function_definition(ctx, runtime, name, "FDEFINITION", lambda_list, &parts[3..])
}

fn defmacro(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    defmacro_like(runtime, ctx, args, values, "MACRO-FUNCTION")
}

fn defmacro_like(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut ncl_object::MultipleValues,
    accessor: &str,
) -> Result<Word, ObjectError> {
    let parts = form_elements(ctx, required(args, 0)?)?;
    ensure_form_operator(ctx, runtime, &parts, "DEFMACRO")?;
    let name = ensure_symbol(ctx, required(&parts, 1)?)?;
    let lambda_list = required(&parts, 2)?;
    ensure_list(ctx, lambda_list)?;
    function_definition(ctx, runtime, name, accessor, lambda_list, &parts[3..])
}

fn variable_definition(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    always: bool,
    constant: bool,
    operator: &str,
) -> Result<Word, ObjectError> {
    let parts = form_elements(ctx, required(args, 0)?)?;
    ensure_form_operator(ctx, runtime, &parts, operator)?;
    let name = ensure_symbol(ctx, required(&parts, 1)?)?;
    set_symbol_special(ctx, name, true)?;
    if constant {
        set_symbol_constant(ctx, name, true)?;
    }
    let value = parts.get(2).copied().unwrap_or(Word::NIL);
    let setq_symbol = symbol(ctx, runtime, "SETQ")?;
    let setq = list(ctx, runtime, &[setq_symbol, name, value])?;
    let operation = if always || constant {
        setq
    } else {
        let unless_symbol = symbol(ctx, runtime, "UNLESS")?;
        let boundp_symbol = symbol(ctx, runtime, "BOUNDP")?;
        let quoted_name = quote(ctx, runtime, name)?;
        let boundp = list(ctx, runtime, &[boundp_symbol, quoted_name])?;
        list(ctx, runtime, &[unless_symbol, boundp, setq])?
    };
    progn_with_definition(ctx, runtime, name, operation)
}

fn defvar(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    variable_definition(runtime, ctx, args, false, false, "DEFVAR")
}

fn defparameter(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    variable_definition(runtime, ctx, args, true, false, "DEFPARAMETER")
}

fn defconstant(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    variable_definition(runtime, ctx, args, true, true, "DEFCONSTANT")
}

fn define_symbol_macro(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let parts = form_elements(ctx, required(args, 0)?)?;
    ensure_form_operator(ctx, runtime, &parts, "DEFINE-SYMBOL-MACRO")?;
    let name = ensure_symbol(ctx, required(&parts, 1)?)?;
    let expansion = required(&parts, 2)?;
    property_definition(ctx, runtime, name, "SYMBOL-MACRO", expansion)
}

fn define_compiler_macro(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    defmacro_like(runtime, ctx, args, values, "COMPILER-MACRO-FUNCTION")
}

fn defsetf(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let parts = form_elements(ctx, required(args, 0)?)?;
    ensure_form_operator(ctx, runtime, &parts, "DEFSETF")?;
    let name = ensure_symbol(ctx, required(&parts, 1)?)?;
    required(&parts, 2)?;
    let quote_symbol = symbol(ctx, runtime, "QUOTE")?;
    let definition = list(ctx, runtime, &parts[2..])?;
    let value = list(ctx, runtime, &[quote_symbol, definition])?;
    property_definition(ctx, runtime, name, DEFINITION_PROPERTY, value)
}

fn define_setf_expander(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    defsetf(runtime, ctx, args, values)
}

macro_rules! adapters {
    ($($adapter:ident => $callback:ident),+ $(,)?) => { $(
        pub fn $adapter(
            ctx: &mut ThreadContext,
            runtime: &Runtime,
            args: &ncl_object::BuiltinArgs<'_>,
            values: &mut ncl_object::MultipleValues,
        ) -> Result<Word, ObjectError> {
            let words = (0..args.len()).filter_map(|index| args.get(index)).collect::<Vec<_>>();
            $callback(runtime, ctx, &words, values)
        }
    )+ }
}

adapters! {
    defun_adapter => defun,
    defmacro_adapter => defmacro,
    defvar_adapter => defvar,
    defparameter_adapter => defparameter,
    defconstant_adapter => defconstant,
    define_symbol_macro_adapter => define_symbol_macro,
    define_compiler_macro_adapter => define_compiler_macro,
    defsetf_adapter => defsetf,
    define_setf_expander_adapter => define_setf_expander,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Result<(Runtime, ThreadContext), ObjectError> {
        let runtime = Runtime::new()?;
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)?;
        Ok((runtime, ctx))
    }

    fn call(
        runtime: &Runtime,
        ctx: &mut ThreadContext,
        name: &str,
        form: Word,
    ) -> Result<Word, ObjectError> {
        let mut values = ncl_object::MultipleValues::new();
        callback_for(name)
            .ok_or(ObjectError::UndefinedFunction)?(runtime, ctx, &[form], &mut values)
    }

    #[test]
    fn definition_expansions_have_a_progn_and_quoted_name() -> Result<(), ObjectError> {
        let (runtime, mut ctx) = fixture()?;
        let name = symbol(&mut ctx, &runtime, "X")?;
        let operator = symbol(&mut ctx, &runtime, "DEFVAR")?;
        let form = list(&mut ctx, &runtime, &[operator, name])?;
        let expanded = call(&runtime, &mut ctx, "DEFVAR", form)?;
        let parts = elements(&mut ctx, expanded)?;
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], symbol(&mut ctx, &runtime, "PROGN")?);
        let result = elements(&mut ctx, parts[2])?;
        assert_eq!(result[0], symbol(&mut ctx, &runtime, "QUOTE")?);
        assert_eq!(result[1], name);
        Ok(())
    }

    #[test]
    fn function_definitions_preserve_lambda_body() -> Result<(), ObjectError> {
        let (runtime, mut ctx) = fixture()?;
        let name = symbol(&mut ctx, &runtime, "F")?;
        let body = symbol(&mut ctx, &runtime, "BODY")?;
        let operator = symbol(&mut ctx, &runtime, "DEFUN")?;
        let form = list(&mut ctx, &runtime, &[operator, name, Word::NIL, body])?;
        let expanded = call(&runtime, &mut ctx, "DEFUN", form)?;
        let expanded_parts = elements(&mut ctx, expanded)?;
        let setf = elements(&mut ctx, expanded_parts[1])?;
        assert_eq!(setf[0], symbol(&mut ctx, &runtime, "SETF")?);
        let place = elements(&mut ctx, setf[1])?;
        assert_eq!(place[0], symbol(&mut ctx, &runtime, "FDEFINITION")?);
        let function = elements(&mut ctx, setf[2])?;
        let lambda = elements(&mut ctx, function[1])?;
        assert_eq!(lambda[1], Word::NIL);
        assert_eq!(lambda[2], body);
        Ok(())
    }

    #[test]
    fn malformed_definition_is_rejected() -> Result<(), ObjectError> {
        let (runtime, mut ctx) = fixture()?;
        let operator = symbol(&mut ctx, &runtime, "DEFUN")?;
        let form = list(&mut ctx, &runtime, &[operator])?;
        let mut values = ncl_object::MultipleValues::new();
        assert_eq!(
            defun(&runtime, &mut ctx, &[form], &mut values),
            Err(ObjectError::TypeError)
        );
        Ok(())
    }

    #[test]
    fn malformed_lambda_list_is_rejected() -> Result<(), ObjectError> {
        let (runtime, mut ctx) = fixture()?;
        let operator = symbol(&mut ctx, &runtime, "DEFUN")?;
        let name = symbol(&mut ctx, &runtime, "F")?;
        let bad_lambda_list = symbol(&mut ctx, &runtime, "ARGS")?;
        let form = list(&mut ctx, &runtime, &[operator, name, bad_lambda_list])?;
        let mut values = ncl_object::MultipleValues::new();
        assert_eq!(defun(&runtime, &mut ctx, &[form], &mut values), Err(ObjectError::TypeError));
        Ok(())
    }

    #[test]
    fn macro_and_setf_definitions_target_runtime_places() -> Result<(), ObjectError> {
        let (runtime, mut ctx) = fixture()?;
        let name = symbol(&mut ctx, &runtime, "M")?;
        let body = symbol(&mut ctx, &runtime, "BODY")?;

        let defmacro = symbol(&mut ctx, &runtime, "DEFMACRO")?;
        let macro_form = list(&mut ctx, &runtime, &[defmacro, name, Word::NIL, body])?;
        let macro_expansion = call(&runtime, &mut ctx, "DEFMACRO", macro_form)?;
        let macro_parts = elements(&mut ctx, macro_expansion)?;
        let macro_setf = elements(&mut ctx, macro_parts[1])?;
        let macro_place = elements(&mut ctx, macro_setf[1])?;
        assert_eq!(macro_place[0], symbol(&mut ctx, &runtime, "MACRO-FUNCTION")?);

        let defsetf = symbol(&mut ctx, &runtime, "DEFSETF")?;
        let accessor = symbol(&mut ctx, &runtime, "ACCESSOR")?;
        let updater = symbol(&mut ctx, &runtime, "UPDATER")?;
        let setf_form = list(&mut ctx, &runtime, &[defsetf, accessor, updater])?;
        let setf_expansion = call(&runtime, &mut ctx, "DEFSETF", setf_form)?;
        let setf_parts = elements(&mut ctx, setf_expansion)?;
        let setf_operation = elements(&mut ctx, setf_parts[1])?;
        let get_place = elements(&mut ctx, setf_operation[1])?;
        assert_eq!(get_place[0], symbol(&mut ctx, &runtime, "GET")?);
        Ok(())
    }

    #[test]
    fn callback_registry_covers_requested_definers() {
        for name in [
            "DEFUN",
            "DEFMACRO",
            "DEFVAR",
            "DEFPARAMETER",
            "DEFCONSTANT",
            "DEFINE-SYMBOL-MACRO",
            "DEFINE-COMPILER-MACRO",
            "DEFSETF",
            "DEFINE-SETF-EXPANDER",
        ] {
            assert!(callback_for(name).is_some(), "{name}");
        }
    }
}
