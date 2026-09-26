//! Runtime support for definition-related accessors and generalized places.
#![allow(clippy::missing_errors_doc)]

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, FunctionObject, LambdaList, ObjectError, Runtime, ThreadContext,
    Word, make_cons, rplaca, symbol_function, symbol_name, symbol_plist,
};

use crate::form::{list, symbol};

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
    Ok(FunctionObject::try_from(function).map_or(Word::NIL, Word::from))
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
            rplaca(ctx, rest, value)?;
            return Ok(value);
        }
        plist = ncl_object::cdr(ctx, rest)?;
    }
    let old_plist = symbol_plist(ctx, name)?;
    ncl_object::with_roots(ctx, &[name, property, value, old_plist], |ctx, roots| {
        let name = *roots[0];
        let property = *roots[1];
        let value = *roots[2];
        let old_plist = *roots[3];
        let value_cell = make_cons(ctx, runtime, value, Word::NIL)?;
        let entry = make_cons(ctx, runtime, property, value_cell)?;
        let next = make_cons(ctx, runtime, entry, old_plist)?;
        ctx.write_object_slot(name, ncl_object::symbol_offset::PLIST, next)?;
        Ok(value)
    })
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

pub fn register_runtime_support(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
) -> Result<(), ObjectError> {
    for (name, place) in [
        (
            "FDEFINITION",
            fdefinition_place as ncl_object::PlaceExpander,
        ),
        (
            "MACRO-FUNCTION",
            macro_function_place as ncl_object::PlaceExpander,
        ),
        ("GET", get_place as ncl_object::PlaceExpander),
    ] {
        let symbol = symbol(ctx, runtime, name)?;
        runtime.register_place_expander(ctx, symbol, place)?;
    }
    let builtins: &[(&str, ncl_object::RustBuiltin, LambdaList)] = &[
        (
            "FDEFINITION",
            fdefinition_builtin,
            LambdaList::fixed(&[SYMBOL]),
        ),
        (
            "NCL::FDEFINITION-SET",
            set_fdefinition_builtin,
            LambdaList::fixed(&[SYMBOL, VALUE]),
        ),
        (
            "MACRO-FUNCTION",
            macro_function_builtin,
            LambdaList::fixed(&[SYMBOL]),
        ),
        (
            "NCL::MACRO-FUNCTION-SET",
            set_macro_function_builtin,
            LambdaList::fixed(&[SYMBOL, VALUE]),
        ),
        ("GET", get_builtin, LambdaList::fixed(&[SYMBOL, PROPERTY])),
        (
            "NCL::GET-SET",
            set_get_builtin,
            LambdaList::fixed(&[SYMBOL, PROPERTY, VALUE]),
        ),
    ];
    for (builtin, callback, params) in builtins {
        let (package, builtin_name) = (*builtin)
            .split_once("::")
            .map_or(("COMMON-LISP", *builtin), |parts| parts);
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
