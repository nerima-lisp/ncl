//! Runtime support for definition-related accessors and generalized places.
#![allow(clippy::missing_errors_doc)]

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, FunctionObject, HandleVec, LambdaList, ObjectError, Runtime,
    Scope, ThreadContext, Word, make_cons, rplaca, set_symbol_macro, symbol_function,
    symbol_is_macro, symbol_name, symbol_plist,
};

use crate::form::{list, symbol};

#[path = "definition_support_defsetf.rs"]
mod defsetf;
use defsetf::declaration_form;

#[path = "definition_support_documentation.rs"]
mod documentation;

pub fn defsetf_body_forms<'scope>(
    scope: &mut Scope<'scope>,
    runtime: &Runtime,
    definitions: &HandleVec<'scope, Word>,
) -> Result<HandleVec<'scope, Word>, ObjectError> {
    defsetf::defsetf_body_forms(scope, runtime, definitions)
}

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
const DOCUMENTATION_TYPE: ncl_object::Parameter = ncl_object::Parameter {
    name: BuiltinName::new("DOCUMENTATION-TYPE"),
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
    set_symbol_macro(ctx, name, false)?;
    Ok(function)
}

fn macro_function_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let name = checked_symbol(ctx, args.required(0)?)?;
    if !symbol_is_macro(ctx, name)? {
        return Ok(Word::NIL);
    }
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
    set_symbol_macro(ctx, name, true)?;
    Ok(function)
}

fn plist_property(
    ctx: &ThreadContext,
    mut plist: Word,
    property: Word,
) -> Result<Option<Word>, ObjectError> {
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
            return ncl_object::car(ctx, rest).map(Some);
        }
        plist = ncl_object::cdr(ctx, rest)?;
    }
    Ok(None)
}

pub fn get_property(ctx: &ThreadContext, name: Word, property: Word) -> Result<Word, ObjectError> {
    checked_symbol(ctx, name)?;
    Ok(plist_property(ctx, symbol_plist(ctx, name)?, property)?.unwrap_or(Word::NIL))
}

fn set_plist_property(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    plist: Word,
    property: Word,
    value: Word,
) -> Result<Word, ObjectError> {
    ncl_object::with_roots(ctx, &[plist, property, value], |ctx, roots| {
        let mut current = roots[0].get(); // check-added-lines: allow(index) roots has the three fixed inputs above.
        while current != Word::NIL {
            if !current.is_cons() {
                return Err(ObjectError::TypeError);
            }
            let key = ncl_object::car(ctx, current)?;
            let rest = ncl_object::cdr(ctx, current)?;
            if !rest.is_cons() {
                return Err(ObjectError::TypeError);
            }
            // check-added-lines: allow(index) roots has the three fixed inputs above.
            if key == roots[1].get() {
                rplaca(ctx, rest, roots[2].get())?; // check-added-lines: allow(index) roots has the three fixed inputs above.
                return Ok(roots[0].get()); // check-added-lines: allow(index) roots has the three fixed inputs above.
            }
            current = ncl_object::cdr(ctx, current)?;
        }
        let mut value_cell = make_cons(
            ctx,
            runtime,
            roots[2].get(), // check-added-lines: allow(index) roots has the three fixed inputs above.
            roots[0].get(), // check-added-lines: allow(index) roots has the three fixed inputs above.
        )?;
        ncl_object::with_root(ctx, &mut value_cell, |ctx, value_cell| {
            make_cons(
                ctx,
                runtime,
                roots[1].get(), // check-added-lines: allow(index) roots has the three fixed inputs above.
                *value_cell,
            )
        })
    })
}

fn set_property(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: Word,
    property: Word,
    value: Word,
) -> Result<(), ObjectError> {
    checked_symbol(ctx, name)?;
    ncl_object::with_roots(ctx, &[name, property, value], |ctx, roots| {
        let plist = symbol_plist(ctx, roots[0].get())?; // check-added-lines: allow(index) roots has the three fixed inputs above.
        let mut plist = set_plist_property(
            ctx,
            runtime,
            plist,
            roots[1].get(), // check-added-lines: allow(index) roots has the three fixed inputs above.
            roots[2].get(), // check-added-lines: allow(index) roots has the three fixed inputs above.
        )?;
        ncl_object::with_root(ctx, &mut plist, |ctx, plist| {
            ctx.write_object_slot(
                roots[0].get(), // check-added-lines: allow(index) roots has the three fixed inputs above.
                ncl_object::symbol_offset::PLIST,
                *plist,
            )
        })
    })
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
        let name = roots[0].get(); // check-added-lines: allow(index) roots has the four fixed inputs above.
        let property = roots[1].get(); // check-added-lines: allow(index) roots has the four fixed inputs above.
        let value = roots[2].get(); // check-added-lines: allow(index) roots has the four fixed inputs above.
        let old_plist = roots[3].get(); // check-added-lines: allow(index) roots has the four fixed inputs above.
        let mut value_cell = make_cons(ctx, runtime, value, old_plist)?;
        ncl_object::with_root(ctx, &mut value_cell, |ctx, value_cell| {
            let next = make_cons(ctx, runtime, property, *value_cell)?;
            ctx.write_object_slot(name, ncl_object::symbol_offset::PLIST, next)?;
            Ok(value)
        })
    })
}

fn place_form(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    operator: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    ncl_object::with_roots(ctx, args, |ctx, args| {
        let value = args.first().copied().ok_or(ObjectError::TypeError)?;
        let operator = symbol(ctx, runtime, operator)?;
        list(ctx, runtime, &[operator, *value])
    })
}

fn definition_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    accessor: &str,
    setter: &str,
    optional_second_argument: bool,
) -> Result<ncl_object::SetfExpansion, ObjectError> {
    if args.len() != 1 && !(optional_second_argument && args.len() == 2) {
        return Err(ObjectError::TypeError);
    }
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        let mut temporary_roots = Vec::with_capacity(roots.len());
        for _ in roots {
            let temporary = crate::fresh_symbol(ctx, runtime)?;
            temporary_roots.push(ncl_object::HeapRootedWord::new(ctx, temporary)?);
        }
        let temporary_variables = temporary_roots
            .iter()
            .map(ncl_object::HeapRootedWord::get)
            .collect::<Vec<_>>();
        ncl_object::with_roots(ctx, &temporary_variables, |ctx, temporaries| {
            let first_temporary = temporaries.first().ok_or(ObjectError::TypeError)?.get();
            let mut setter = symbol(ctx, runtime, setter)?;
            ncl_object::with_root(ctx, &mut setter, |ctx, setter| {
                let mut variable = symbol(ctx, runtime, "NCL::STORE")?;
                ncl_object::with_root(ctx, &mut variable, |ctx, variable| {
                    let mut store_form =
                        list(ctx, runtime, &[*setter, first_temporary, *variable])?;
                    ncl_object::with_root(ctx, &mut store_form, |ctx, store_form| {
                        let access_form = place_form(ctx, runtime, accessor, &[first_temporary])?;
                        Ok(ncl_object::SetfExpansion {
                            temporary_variables: temporaries.iter().map(|value| **value).collect(),
                            value_forms: roots.iter().map(|value| **value).collect(),
                            store_variables: vec![*variable],
                            store_form: *store_form,
                            access_form,
                        })
                    })
                })
            })
        })
    })
}

fn fdefinition_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<ncl_object::SetfExpansion, ObjectError> {
    definition_place(
        ctx,
        runtime,
        args,
        "FDEFINITION",
        "NCL-EXT::FDEFINITION-SET",
        false,
    )
}

fn macro_function_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<ncl_object::SetfExpansion, ObjectError> {
    definition_place(
        ctx,
        runtime,
        args,
        "MACRO-FUNCTION",
        "NCL-EXT::MACRO-FUNCTION-SET",
        true,
    )
}

fn get_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<ncl_object::SetfExpansion, ObjectError> {
    if args.len() != 2 {
        return Err(ObjectError::TypeError);
    }
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        let mut store = symbol(ctx, runtime, "NCL-EXT::GET-SET")?;
        ncl_object::with_root(ctx, &mut store, |ctx, store| {
            let mut variable = symbol(ctx, runtime, "NCL::STORE")?;
            ncl_object::with_root(ctx, &mut variable, |ctx, variable| {
                let temporary_variables = roots
                    .iter()
                    .map(|_| crate::fresh_symbol(ctx, runtime))
                    .collect::<Result<Vec<_>, _>>()?;
                ncl_object::with_roots(ctx, &temporary_variables, |ctx, temporaries| {
                    let first = temporaries[0].get(); // check-added-lines: allow(index) args has two inputs and creates two temporaries.
                    let second = temporaries[1].get(); // check-added-lines: allow(index) args has two inputs and creates two temporaries.
                    let mut store_form = list(ctx, runtime, &[*store, first, second, *variable])?;
                    ncl_object::with_root(ctx, &mut store_form, |ctx, store_form| {
                        let mut get = symbol(ctx, runtime, "GET")?;
                        ncl_object::with_root(ctx, &mut get, |ctx, get| {
                            let access_form = list(ctx, runtime, &[*get, first, second])?;
                            Ok(ncl_object::SetfExpansion {
                                temporary_variables: temporaries
                                    .iter()
                                    .map(|value| **value)
                                    .collect(),
                                value_forms: roots.iter().map(|value| **value).collect(),
                                store_variables: vec![*variable],
                                store_form: *store_form,
                                access_form,
                            })
                        })
                    })
                })
            })
        })
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
        let mut symbol = symbol(ctx, runtime, name)?;
        ncl_object::with_root(ctx, &mut symbol, |ctx, symbol| {
            runtime.register_place_expander(ctx, *symbol, place)
        })?;
    }
    let builtins: &[(BuiltinPackage, &str, ncl_object::RustBuiltin, LambdaList)] = &[
        (
            CL,
            "FDEFINITION",
            fdefinition_builtin,
            LambdaList::fixed(&[SYMBOL]),
        ),
        (
            BuiltinPackage::NclExt,
            "FDEFINITION-SET",
            set_fdefinition_builtin,
            LambdaList::fixed(&[SYMBOL, VALUE]),
        ),
        (
            CL,
            "MACRO-FUNCTION",
            macro_function_builtin,
            LambdaList::fixed(&[SYMBOL]),
        ),
        (
            BuiltinPackage::NclExt,
            "MACRO-FUNCTION-SET",
            set_macro_function_builtin,
            LambdaList::fixed(&[SYMBOL, VALUE]),
        ),
        (
            BuiltinPackage::NclExt,
            "DOCUMENTATION-SET",
            documentation::set_documentation_builtin,
            LambdaList::fixed(&[SYMBOL, DOCUMENTATION_TYPE, VALUE]),
        ),
        (
            CL,
            "GET",
            get_builtin,
            LambdaList::fixed(&[SYMBOL, PROPERTY]),
        ),
        (
            CL,
            "DOCUMENTATION",
            documentation::documentation_builtin,
            LambdaList::fixed(&[SYMBOL, DOCUMENTATION_TYPE]),
        ),
        (
            BuiltinPackage::NclExt,
            "GET-SET",
            set_get_builtin,
            LambdaList::fixed(&[SYMBOL, PROPERTY, VALUE]),
        ),
    ];
    for (package, builtin_name, callback, params) in builtins {
        runtime.register_builtin(
            ctx,
            BuiltinIdentifier::new(*package, BuiltinName::new(builtin_name)),
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
#[path = "definition_support_tests.rs"]
mod tests;
