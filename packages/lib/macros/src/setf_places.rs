//! `setf`-place expanders for the standard Common Lisp accessors that do not
//! need bespoke expansion logic: variable assignment is handled directly in
//! [`crate::setf`], and this module covers `CAR`/`CDR`/`FIRST`/`REST`/`NTH`/`CADR`
//! (which reuse the existing `RPLACA`/`RPLACD`/`NTHCDR` builtins wrapped in a
//! `PROGN` that returns the new value) plus `SYMBOL-VALUE` (which calls the
//! existing `SET` builtin, which already returns the new value).
//!
//! The array and hash-table places call their corresponding `NCL-EXT` setter
//! builtins, which return the stored value required by `SETF`.
#![allow(clippy::missing_errors_doc)]

use ncl_object::{ObjectError, PlaceExpander, Runtime, SetfExpansion, ThreadContext, Word};

use crate::form::{list, symbol};
use crate::fresh_symbol;

/// Build `(operator arg0 arg1 ... argN)` as a fresh list, rooting `args` for
/// the duration of the allocation.
fn call_form(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    operator: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        let operator = symbol(ctx, runtime, operator)?;
        let mut values = Vec::with_capacity(roots.len() + 1);
        values.push(operator);
        values.extend(roots.iter().map(|value| **value));
        list(ctx, runtime, &values)
    })
}

/// A place expansion of the shape `(op arg...)` whose store form calls
/// `setter_operator` with the same arguments followed by the new value, and
/// which already returns that value (e.g. `NCL-EXT::AREF-SET`, `NCL-EXT::GETHASH-SET`
/// or the existing `SET` builtin for `SYMBOL-VALUE`).
fn setter_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    access_operator: &str,
    setter_operator: &str,
) -> Result<SetfExpansion, ObjectError> {
    if args.is_empty() {
        return Err(ObjectError::TypeError);
    }
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        let mut variable = symbol(ctx, runtime, "NCL::STORE")?;
        ncl_object::with_root(ctx, &mut variable, |ctx, variable| {
            let temporary_variables = roots
                .iter()
                .map(|_| fresh_symbol(ctx, runtime))
                .collect::<Result<Vec<_>, _>>()?;
            ncl_object::with_roots(ctx, &temporary_variables, |ctx, temporaries| {
                let arguments = temporaries.iter().map(|value| **value).collect::<Vec<_>>();
                let mut access_form = call_form(ctx, runtime, access_operator, &arguments)?;
                ncl_object::with_root(ctx, &mut access_form, |ctx, access_form| {
                    let mut store_arguments = arguments;
                    store_arguments.push(*variable);
                    let store_form = call_form(ctx, runtime, setter_operator, &store_arguments)?;
                    Ok(SetfExpansion {
                        temporary_variables: temporaries.iter().map(|value| **value).collect(),
                        value_forms: roots.iter().map(|value| **value).collect(),
                        store_variables: vec![*variable],
                        store_form,
                        access_form: *access_form,
                    })
                })
            })
        })
    })
}

/// A place expansion of the shape `(op target)` whose store form is
/// `(PROGN (mutator target store) store)`, for accessors (`CAR`, `CDR`, ...)
/// whose existing mutator (`RPLACA`, `RPLACD`, ...) returns the mutated cons
/// rather than the stored value.
fn mutator_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    access_operator: &str,
    mutator_operator: &str,
) -> Result<SetfExpansion, ObjectError> {
    if args.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        let mut variable = symbol(ctx, runtime, "NCL::STORE")?;
        ncl_object::with_root(ctx, &mut variable, |ctx, variable| {
            let temporary = fresh_symbol(ctx, runtime)?;
            ncl_object::with_root(ctx, &mut temporary.clone(), |ctx, temporary| {
                let target = *temporary;
                let mut access_form = call_form(ctx, runtime, access_operator, &[target])?;
                ncl_object::with_root(ctx, &mut access_form, |ctx, access_form| {
                    let mut mutate_form =
                        call_form(ctx, runtime, mutator_operator, &[target, *variable])?;
                    ncl_object::with_root(ctx, &mut mutate_form, |ctx, mutate_form| {
                        let store_form =
                            call_form(ctx, runtime, "PROGN", &[*mutate_form, *variable])?;
                        Ok(SetfExpansion {
                            temporary_variables: vec![*temporary],
                            value_forms: vec![**roots.first().ok_or(ObjectError::TypeError)?],
                            store_variables: vec![*variable],
                            store_form,
                            access_form: *access_form,
                        })
                    })
                })
            })
        })
    })
}

/// `(setf (nth n list) v)` expands to `(rplaca (nthcdr n list) v)` wrapped so
/// the whole store form returns `v`.
fn nth_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<SetfExpansion, ObjectError> {
    if args.len() != 2 {
        return Err(ObjectError::TypeError);
    }
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        let mut variable = symbol(ctx, runtime, "NCL::STORE")?;
        ncl_object::with_root(ctx, &mut variable, |ctx, variable| {
            let temporary_variables = roots
                .iter()
                .map(|_| fresh_symbol(ctx, runtime))
                .collect::<Result<Vec<_>, _>>()?;
            ncl_object::with_roots(ctx, &temporary_variables, |ctx, temporaries| {
                let index = **temporaries.first().ok_or(ObjectError::TypeError)?;
                let list_arg = **temporaries.get(1).ok_or(ObjectError::TypeError)?;
                let mut access_form = call_form(ctx, runtime, "NTH", &[index, list_arg])?;
                ncl_object::with_root(ctx, &mut access_form, |ctx, access_form| {
                    let mut nthcdr_form = call_form(ctx, runtime, "NTHCDR", &[index, list_arg])?;
                    ncl_object::with_root(ctx, &mut nthcdr_form, |ctx, nthcdr_form| {
                        let mut mutate_form =
                            call_form(ctx, runtime, "RPLACA", &[*nthcdr_form, *variable])?;
                        ncl_object::with_root(ctx, &mut mutate_form, |ctx, mutate_form| {
                            let store_form =
                                call_form(ctx, runtime, "PROGN", &[*mutate_form, *variable])?;
                            Ok(SetfExpansion {
                                temporary_variables: temporaries
                                    .iter()
                                    .map(|value| **value)
                                    .collect(),
                                value_forms: roots.iter().map(|value| **value).collect(),
                                store_variables: vec![*variable],
                                store_form,
                                access_form: *access_form,
                            })
                        })
                    })
                })
            })
        })
    })
}

fn cadr_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<SetfExpansion, ObjectError> {
    if args.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    let nth_args = [Word::fixnum(1), args[0]];
    nth_place(ctx, runtime, &nth_args)
}

fn car_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<SetfExpansion, ObjectError> {
    mutator_place(ctx, runtime, args, "CAR", "RPLACA")
}
fn cdr_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<SetfExpansion, ObjectError> {
    mutator_place(ctx, runtime, args, "CDR", "RPLACD")
}
fn first_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<SetfExpansion, ObjectError> {
    mutator_place(ctx, runtime, args, "FIRST", "RPLACA")
}
fn rest_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<SetfExpansion, ObjectError> {
    mutator_place(ctx, runtime, args, "REST", "RPLACD")
}
fn symbol_value_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<SetfExpansion, ObjectError> {
    setter_place(ctx, runtime, args, "SYMBOL-VALUE", "SET")
}

fn aref_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<SetfExpansion, ObjectError> {
    setter_place(ctx, runtime, args, "AREF", "NCL-EXT::AREF-SET")
}

fn svref_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<SetfExpansion, ObjectError> {
    setter_place(ctx, runtime, args, "SVREF", "NCL-EXT::SVREF-SET")
}

fn gethash_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<SetfExpansion, ObjectError> {
    setter_place(ctx, runtime, args, "GETHASH", "NCL-EXT::GETHASH-SET")
}

/// Register the place expanders defined in this module.
///
/// # Errors
///
/// Returns an object-layer error if a symbol cannot be interned or a place
/// expander cannot be registered.
pub fn register(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    for (name, place) in [
        ("CAR", car_place as PlaceExpander),
        ("CDR", cdr_place as PlaceExpander),
        ("FIRST", first_place as PlaceExpander),
        ("REST", rest_place as PlaceExpander),
        ("NTH", nth_place as PlaceExpander),
        ("CADR", cadr_place as PlaceExpander),
        ("SYMBOL-VALUE", symbol_value_place as PlaceExpander),
        ("AREF", aref_place as PlaceExpander),
        ("SVREF", svref_place as PlaceExpander),
        ("GETHASH", gethash_place as PlaceExpander),
    ] {
        let mut symbol = symbol(ctx, runtime, name)?;
        ncl_object::with_root(ctx, &mut symbol, |ctx, symbol| {
            runtime.register_place_expander(ctx, *symbol, place)
        })?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "setf_places_tests.rs"]
mod tests;
