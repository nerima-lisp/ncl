//! `setf`-place expanders for the standard Common Lisp accessors that do not
//! need bespoke expansion logic: variable assignment is handled directly in
//! [`crate::setf`], and this module covers `CAR`/`CDR`/`FIRST`/`REST`/`NTH`
//! (which reuse the existing `RPLACA`/`RPLACD`/`NTHCDR` builtins wrapped in a
//! `PROGN` that returns the new value) plus `SYMBOL-VALUE` (which calls the
//! existing `SET` builtin, which already returns the new value).
//!
//! The array and hash-table places call their corresponding `NCL-EXT` setter
//! builtins, which return the stored value required by `SETF`.
#![allow(clippy::missing_errors_doc)]

use ncl_object::{
    HeapRootedWord, ObjectError, PlaceExpander, Runtime, SetfExpansion, ThreadContext, Word,
};

use crate::form::{list, symbol};
use crate::fresh_symbol;

fn fresh_temporaries(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    count: usize,
) -> Result<Vec<HeapRootedWord>, ObjectError> {
    let mut temporaries = Vec::with_capacity(count);
    for _ in 0..count {
        let temporary = fresh_symbol(ctx, runtime)?;
        temporaries.push(HeapRootedWord::new(ctx, temporary)?);
    }
    Ok(temporaries)
}

#[path = "setf_places_cons.rs"]
mod cons;
pub(super) use cons::standard_cons_place;

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
            let temporary_roots = fresh_temporaries(ctx, runtime, roots.len())?;
            let temporary_variables = temporary_roots
                .iter()
                .map(HeapRootedWord::get)
                .collect::<Vec<_>>();
            ncl_object::with_roots(ctx, &temporary_variables, |ctx, temporaries| {
                let access_arguments = temporaries.iter().map(|value| **value).collect::<Vec<_>>();
                let mut access_form = call_form(ctx, runtime, access_operator, &access_arguments)?;
                ncl_object::with_root(ctx, &mut access_form, |ctx, access_form| {
                    let mut store_arguments =
                        temporaries.iter().map(|value| **value).collect::<Vec<_>>();
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
                let mut access_form = call_form(ctx, runtime, access_operator, &[*temporary])?;
                ncl_object::with_root(ctx, &mut access_form, |ctx, access_form| {
                    let target = *temporary;
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
            let temporary_roots = fresh_temporaries(ctx, runtime, roots.len())?;
            let temporary_variables = temporary_roots
                .iter()
                .map(HeapRootedWord::get)
                .collect::<Vec<_>>();
            ncl_object::with_roots(ctx, &temporary_variables, |ctx, temporaries| {
                let mut access_form = call_form(
                    ctx,
                    runtime,
                    "NTH",
                    &[
                        **temporaries.first().ok_or(ObjectError::TypeError)?,
                        **temporaries.get(1).ok_or(ObjectError::TypeError)?,
                    ],
                )?;
                ncl_object::with_root(ctx, &mut access_form, |ctx, access_form| {
                    let index = **temporaries.first().ok_or(ObjectError::TypeError)?;
                    let list_arg = **temporaries.get(1).ok_or(ObjectError::TypeError)?;
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

fn elt_place(
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
            let temporary_roots = fresh_temporaries(ctx, runtime, roots.len())?;
            let temporary_variables = temporary_roots
                .iter()
                .map(HeapRootedWord::get)
                .collect::<Vec<_>>();
            ncl_object::with_roots(ctx, &temporary_variables, |ctx, temporaries| {
                let mut access_form = call_form(
                    ctx,
                    runtime,
                    "ELT",
                    &[
                        **temporaries.first().ok_or(ObjectError::TypeError)?,
                        **temporaries.get(1).ok_or(ObjectError::TypeError)?,
                    ],
                )?;
                ncl_object::with_root(ctx, &mut access_form, |ctx, access_form| {
                    let sequence = **temporaries.first().ok_or(ObjectError::TypeError)?;
                    let mut list_test = call_form(ctx, runtime, "LISTP", &[sequence])?;
                    ncl_object::with_root(ctx, &mut list_test, |ctx, list_test| {
                        let sequence = **temporaries.first().ok_or(ObjectError::TypeError)?;
                        let index = **temporaries.get(1).ok_or(ObjectError::TypeError)?;
                        let mut nthcdr_form =
                            call_form(ctx, runtime, "NTHCDR", &[index, sequence])?;
                        ncl_object::with_root(ctx, &mut nthcdr_form, |ctx, nthcdr_form| {
                            let mut mutate_form =
                                call_form(ctx, runtime, "RPLACA", &[*nthcdr_form, *variable])?;
                            ncl_object::with_root(ctx, &mut mutate_form, |ctx, mutate_form| {
                                let mut list_store_form =
                                    call_form(ctx, runtime, "PROGN", &[*mutate_form, *variable])?;
                                ncl_object::with_root(
                                    ctx,
                                    &mut list_store_form,
                                    |ctx, list_store_form| {
                                        let mut vector_store_form = call_form(
                                            ctx,
                                            runtime,
                                            "NCL-EXT::AREF-SET",
                                            &[
                                                **temporaries
                                                    .first()
                                                    .ok_or(ObjectError::TypeError)?,
                                                **temporaries
                                                    .get(1)
                                                    .ok_or(ObjectError::TypeError)?,
                                                *variable,
                                            ],
                                        )?;
                                        ncl_object::with_root(
                                            ctx,
                                            &mut vector_store_form,
                                            |ctx, vector_store_form| {
                                                let mut branch = call_form(
                                                    ctx,
                                                    runtime,
                                                    "IF",
                                                    &[
                                                        *list_test,
                                                        *list_store_form,
                                                        *vector_store_form,
                                                    ],
                                                )?;
                                                ncl_object::with_root(
                                                    ctx,
                                                    &mut branch,
                                                    |ctx, branch| {
                                                        let store_form = call_form(
                                                            ctx,
                                                            runtime,
                                                            "PROGN",
                                                            &[*access_form, *branch],
                                                        )?;
                                                        Ok(SetfExpansion {
                                                            temporary_variables: temporaries
                                                                .iter()
                                                                .map(|value| **value)
                                                                .collect(),
                                                            value_forms: roots
                                                                .iter()
                                                                .map(|value| **value)
                                                                .collect(),
                                                            store_variables: vec![*variable],
                                                            store_form,
                                                            access_form: *access_form,
                                                        })
                                                    },
                                                )
                                            },
                                        )
                                    },
                                )
                            })
                        })
                    })
                })
            })
        })
    })
}

fn subseq_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<SetfExpansion, ObjectError> {
    if !(2..=3).contains(&args.len()) {
        return Err(ObjectError::TypeError);
    }
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        let mut variable = symbol(ctx, runtime, "NCL::STORE")?;
        ncl_object::with_root(ctx, &mut variable, |ctx, variable| {
            let temporary_roots = fresh_temporaries(ctx, runtime, roots.len())?;
            let temporary_variables = temporary_roots
                .iter()
                .map(HeapRootedWord::get)
                .collect::<Vec<_>>();
            ncl_object::with_roots(ctx, &temporary_variables, |ctx, temporaries| {
                let arguments = temporaries.iter().map(|value| **value).collect::<Vec<_>>();
                let mut access_form = call_form(ctx, runtime, "SUBSEQ", &arguments)?;
                ncl_object::with_root(ctx, &mut access_form, |ctx, access_form| {
                    let mut start_key = symbol(ctx, runtime, "KEYWORD::START")?;
                    ncl_object::with_root(ctx, &mut start_key, |ctx, start_key| {
                        let mut end_key = symbol(ctx, runtime, "KEYWORD::END")?;
                        ncl_object::with_root(ctx, &mut end_key, |ctx, end_key| {
                            let sequence = **temporaries.first().ok_or(ObjectError::TypeError)?;
                            let start = **temporaries.get(1).ok_or(ObjectError::TypeError)?;
                            let mut replace_args = vec![sequence, *variable, *start_key, start];
                            if let Some(end) = temporaries.get(2) {
                                replace_args.push(*end_key);
                                replace_args.push(**end);
                            }
                            let mut replace_form =
                                call_form(ctx, runtime, "REPLACE", &replace_args)?;
                            ncl_object::with_root(ctx, &mut replace_form, |ctx, replace_form| {
                                let store_form =
                                    call_form(ctx, runtime, "PROGN", &[*replace_form, *variable])?;
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
    })
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

fn documentation_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<SetfExpansion, ObjectError> {
    if args.len() != 2 {
        return Err(ObjectError::TypeError);
    }
    setter_place(
        ctx,
        runtime,
        args,
        "DOCUMENTATION",
        "NCL-EXT::DOCUMENTATION-SET",
    )
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

fn sbit_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<SetfExpansion, ObjectError> {
    setter_place(ctx, runtime, args, "SBIT", "SBIT")
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
        ("ELT", elt_place as PlaceExpander),
        ("SUBSEQ", subseq_place as PlaceExpander),
        ("SYMBOL-VALUE", symbol_value_place as PlaceExpander),
        ("DOCUMENTATION", documentation_place as PlaceExpander),
        ("AREF", aref_place as PlaceExpander),
        ("SVREF", svref_place as PlaceExpander),
        ("GETHASH", gethash_place as PlaceExpander),
        ("SBIT", sbit_place as PlaceExpander),
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
