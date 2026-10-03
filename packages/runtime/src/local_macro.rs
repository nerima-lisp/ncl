//! `macrolet` local macro expansion (CLHS special operator `MACROLET`).
//!
//! A `macrolet` definition's expander body is already macroexpanded into
//! [`ncl_compiler_front::ast::Expr`] by the time [`call_local_macro`] runs
//! (see `packages/compiler/front/src/special/binding.rs`), so this module
//! does not re-parse source: it directly interprets that small `Expr` tree
//! against the destructured call arguments, then hands the resulting `Word`
//! back to the front end to macroexpand again.
//!
//! This deliberately supports less than a full macro lambda list: `&key`,
//! `&aux`, and nested destructuring patterns are rejected with a clear
//! [`FrontError`] rather than silently mis-binding, and the interpreter
//! covers constants, variable references, calls to named functions, `if`,
//! and `progn` (the shapes a backquote-expanded expander body actually
//! produces), rather than the full `Expr` grammar. `defmacro`'s lambda
//! lists (`ncl-lib-macros::destructuring`) are unaffected and keep full
//! CLHS 3.4.4 support, because they compile through the ordinary pipeline
//! instead of being interpreted here.
//!
//! Every `Word` this module touches is pushed into a `held` vector before
//! use and re-read from it after any call that can allocate, so a
//! collection triggered mid-expansion cannot leave a stale reference (the
//! same discipline as `ncl-lib-macros::destructuring` and `::backquote`).

use std::collections::BTreeMap;

use ncl_compiler_front::ast::{Expr, LocalMacro, Operator};
use ncl_compiler_front::lambda_list::{LambdaList, ParamName};
use ncl_compiler_front::literal::{Literal, NumberLiteral};
use ncl_compiler_front::{FrontError, SymbolRef};
use ncl_object::{
    FunctionObject, ObjectError, Package, Runtime as ObjectRuntime, ThreadContext, Word, car, cdr,
    make_cons, make_double, make_simple_vector, make_string, symbol_function,
};

use crate::support::call_macro_function;

type Held = Vec<Word>;
type EntryCodes = BTreeMap<usize, (Box<Word>, ncl_sys::RootToken)>; // check-added-lines: allow(word-table) entries are rooted code objects

fn held_get(held: &Held, index: usize) -> std::result::Result<Word, ObjectError> {
    held.get(index).copied().ok_or(ObjectError::Layout)
}

fn held_push(held: &mut Held, value: Word) -> usize {
    held.push(value);
    held.len() - 1
}

fn unsupported(detail: &str) -> FrontError {
    FrontError::MacroExpansion {
        name: SymbolRef::interned("COMMON-LISP", "MACROLET"),
        detail: detail.to_owned(),
    }
}

fn object_error(error: ObjectError) -> FrontError {
    unsupported(&error.to_string())
}

/// Run one allocating step, re-rooting `held` around it and appending the
/// result.
fn held_alloc(
    ctx: &mut ThreadContext,
    held: &mut Held,
    f: impl FnOnce(&mut ThreadContext, &[Word]) -> std::result::Result<Word, ObjectError>,
) -> std::result::Result<usize, ObjectError> {
    let (value, refreshed) = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let snapshot = roots.iter().map(|root| **root).collect::<Vec<_>>();
        let value = f(ctx, &snapshot)?;
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((value, refreshed))
    })?;
    *held = refreshed;
    Ok(held_push(held, value))
}

/// Materialize a frozen [`Literal`] back into a live `Word`.
fn materialize(
    ctx: &mut ThreadContext,
    runtime: &ObjectRuntime,
    held: &mut Held,
    literal: &Literal,
) -> Result<usize, FrontError> {
    match literal {
        Literal::Nil => Ok(held_push(held, Word::NIL)),
        Literal::T => Ok(held_push(held, Word::TRUE)),
        Literal::Character(character) => Ok(held_push(held, Word::character(*character))),
        Literal::Number(NumberLiteral::Fixnum(value)) => Ok(held_push(held, Word::fixnum(*value))),
        Literal::Number(NumberLiteral::DoubleFloat(value)) => {
            let value = *value;
            held_alloc(ctx, held, |ctx, _| {
                make_double(ctx, runtime, value).map(ncl_object::DoubleFloat::as_word)
            })
            .map_err(object_error)
        }
        Literal::Symbol(symbol_ref) => {
            let symbol_ref = symbol_ref.clone();
            held_alloc(ctx, held, |ctx, _| {
                let package_name = symbol_ref.package_name().unwrap_or("KEYWORD");
                let package = runtime
                    .find_package(ctx, package_name)
                    .ok_or(ObjectError::Layout)?;
                Package::from_word(package)
                    .intern(ctx, runtime, &symbol_ref.name)
                    .map(|(symbol, _)| symbol)
            })
            .map_err(object_error)
        }
        Literal::String(characters) => {
            let characters = characters.clone();
            held_alloc(ctx, held, |ctx, _| make_string(ctx, runtime, &characters))
                .map_err(object_error)
        }
        Literal::Cons(first_literal, rest_literal) => {
            let first_index = materialize(ctx, runtime, held, first_literal)?;
            let rest_index = materialize(ctx, runtime, held, rest_literal)?;
            held_alloc(ctx, held, |ctx, snapshot| {
                let first_word = snapshot
                    .get(first_index)
                    .copied()
                    .ok_or(ObjectError::Layout)?;
                let rest_word = snapshot
                    .get(rest_index)
                    .copied()
                    .ok_or(ObjectError::Layout)?;
                make_cons(ctx, runtime, first_word, rest_word)
            })
            .map_err(object_error)
        }
        Literal::Vector(elements) => {
            let mut indexes = Vec::with_capacity(elements.len());
            for element in elements {
                indexes.push(materialize(ctx, runtime, held, element)?);
            }
            held_alloc(ctx, held, |ctx, snapshot| {
                let values = indexes
                    .iter()
                    .map(|index| snapshot.get(*index).copied().ok_or(ObjectError::Layout))
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                make_simple_vector(ctx, runtime, &values)
            })
            .map_err(object_error)
        }
        // check-added-lines: allow(wildcard) Literal is #[non_exhaustive]; the wildcard is required to compile.
        Literal::Array { .. } | Literal::BitVector(_) | _ => Err(unsupported(
            "macrolet: this literal shape is not supported in a local macro body",
        )),
    }
}

/// Call a named global function (not a special operator) with already
/// materialized arguments.
fn call_named(
    ctx: &mut ThreadContext,
    runtime: &ObjectRuntime,
    entry_codes: &EntryCodes,
    held: &mut Held,
    name: &SymbolRef,
    argument_indexes: &[usize],
) -> Result<usize, FrontError> {
    let package_name = name
        .package_name()
        .ok_or_else(|| unsupported("macrolet: uninterned operator has no function cell"))?
        .to_owned();
    let function_name = name.name.clone();
    let (value, refreshed) = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let package = runtime
            .find_package(ctx, &package_name)
            .ok_or(ObjectError::Layout)?;
        let (symbol, _) = Package::from_word(package).intern(ctx, runtime, &function_name)?;
        let function = FunctionObject::try_from(symbol_function(ctx, symbol)?)?;
        let arguments = argument_indexes
            .iter()
            .map(|index| {
                roots
                    .get(*index)
                    .map(|root| **root)
                    .ok_or(ObjectError::Layout)
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let value = if runtime.builtin_descriptor(function).is_some() {
            runtime.call_builtin(ctx, function, &arguments)?
        } else {
            call_macro_function(ctx, runtime, function, &arguments, entry_codes)?
        };
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((value, refreshed))
    })
    .map_err(|error| {
        unsupported(&format!(
            "macrolet: calling {package_name}:{function_name} failed: {error}"
        ))
    })?;
    *held = refreshed;
    Ok(held_push(held, value))
}

/// Evaluate a local macro expander body form.
///
/// Supports constants, variable references bound by the destructured
/// lambda list, calls to named functions, `if`, and `progn` (the shapes a
/// backquote-expanded macro body produces). Anything else is reported, not
/// silently misinterpreted.
fn eval_expr(
    ctx: &mut ThreadContext,
    runtime: &ObjectRuntime,
    entry_codes: &EntryCodes,
    held: &mut Held,
    env: &[(SymbolRef, usize)],
    expr: &Expr,
) -> Result<usize, FrontError> {
    match expr {
        Expr::Constant(literal) => materialize(ctx, runtime, held, literal),
        Expr::Variable(name) => env
            .iter()
            .rev()
            .find(|(bound, _)| bound == name)
            .map(|(_, index)| *index)
            .ok_or_else(|| {
                unsupported(&format!(
                    "macrolet: {} is not bound by the local macro's lambda list",
                    name.name
                ))
            }),
        Expr::Progn(forms) => {
            let mut last = held_push(held, Word::NIL);
            for form in forms {
                last = eval_expr(ctx, runtime, entry_codes, held, env, form)?;
            }
            Ok(last)
        }
        Expr::If {
            test,
            then,
            otherwise,
        } => {
            let test_index = eval_expr(ctx, runtime, entry_codes, held, env, test)?;
            if held_get(held, test_index).map_err(object_error)? != Word::NIL {
                eval_expr(ctx, runtime, entry_codes, held, env, then)
            } else if let Some(otherwise) = otherwise {
                eval_expr(ctx, runtime, entry_codes, held, env, otherwise)
            } else {
                Ok(held_push(held, Word::NIL))
            }
        }
        Expr::Call {
            operator: Operator::Name(name),
            arguments,
        } => {
            let mut argument_indexes = Vec::with_capacity(arguments.len());
            for argument in arguments {
                argument_indexes.push(eval_expr(ctx, runtime, entry_codes, held, env, argument)?);
            }
            call_named(ctx, runtime, entry_codes, held, name, &argument_indexes)
        }
        // check-added-lines: allow(wildcard) Expr is #[non_exhaustive]; only a documented subset is interpreted.
        _ => Err(unsupported(
            "macrolet: this form is not supported in a local macro body",
        )),
    }
}

fn bind_symbol_param(param: &ParamName) -> Result<&SymbolRef, FrontError> {
    match param {
        ParamName::Symbol(symbol) => Ok(symbol),
        // check-added-lines: allow(wildcard) ParamName is #[non_exhaustive]; nested patterns are the only other case.
        ParamName::Pattern(_) | _ => Err(unsupported(
            "macrolet: nested destructuring patterns are not supported for local macros",
        )),
    }
}

/// Destructure `(cdr whole-form)` against `lambda_list`, binding names into
/// `env`. `&key`/`&aux`/nested patterns are rejected explicitly; everything
/// else in CLHS 3.4.4 (`&whole`, `&environment`, `&optional` with defaults
/// and `supplied-p`, `&rest`/`&body`) is supported.
fn destructure(
    ctx: &mut ThreadContext,
    runtime: &ObjectRuntime,
    entry_codes: &EntryCodes,
    lambda_list: &LambdaList,
    held: &mut Held,
    env: &mut Vec<(SymbolRef, usize)>,
) -> Result<(), FrontError> {
    if !lambda_list.keys.is_empty() || !lambda_list.aux.is_empty() {
        return Err(unsupported(
            "macrolet: &key and &aux are not supported for local macros (defmacro supports them)",
        ));
    }
    if let Some(whole) = &lambda_list.whole {
        env.push((whole.clone(), 0));
    }
    if let Some(environment) = &lambda_list.environment {
        let index = held_push(held, Word::NIL);
        env.push((environment.clone(), index));
    }
    let arguments = {
        let form = held_get(held, 0).map_err(object_error)?;
        cdr(ctx, form).map_err(object_error)?
    };
    let mut cursor = held_push(held, arguments);
    for parameter in &lambda_list.required {
        let name = bind_symbol_param(parameter)?;
        let cursor_word = held_get(held, cursor).map_err(object_error)?;
        if !cursor_word.is_cons() {
            return Err(unsupported("macrolet: too few arguments for local macro"));
        }
        let value = car(ctx, cursor_word).map_err(object_error)?;
        let value_index = held_push(held, value);
        env.push((name.clone(), value_index));
        let tail = cdr(ctx, cursor_word).map_err(object_error)?;
        cursor = held_push(held, tail);
    }
    for optional in &lambda_list.optional {
        let name = bind_symbol_param(&optional.name)?;
        let is_present = held_get(held, cursor).map_err(object_error)?.is_cons();
        let value_index = if is_present {
            let cursor_word = held_get(held, cursor).map_err(object_error)?;
            let value = car(ctx, cursor_word).map_err(object_error)?;
            held_push(held, value)
        } else if let Some(default) = &optional.default {
            eval_expr(ctx, runtime, entry_codes, held, env, default)?
        } else {
            held_push(held, Word::NIL)
        };
        env.push((name.clone(), value_index));
        if let Some(supplied) = &optional.supplied_p {
            let supplied_name = bind_symbol_param(supplied)?;
            let index = held_push(held, if is_present { Word::TRUE } else { Word::NIL });
            env.push((supplied_name.clone(), index));
        }
        if is_present {
            let cursor_word = held_get(held, cursor).map_err(object_error)?;
            let tail = cdr(ctx, cursor_word).map_err(object_error)?;
            cursor = held_push(held, tail);
        }
    }
    let rest = lambda_list.rest.as_ref().or(lambda_list.body.as_ref());
    if let Some(rest) = rest {
        let name = bind_symbol_param(rest)?;
        env.push((name.clone(), cursor));
    } else if held_get(held, cursor).map_err(object_error)? != Word::NIL {
        return Err(unsupported("macrolet: too many arguments for local macro"));
    }
    Ok(())
}

/// Expand one call to a `macrolet`-bound local macro.
///
/// # Errors
/// Returns [`FrontError::MacroExpansion`] when the lambda list uses a
/// feature this interpreter does not support (see the module docs), the
/// call form's shape does not match the lambda list, or the expander body
/// uses a form this interpreter does not evaluate.
pub fn call_local_macro(
    ctx: &mut ThreadContext,
    runtime: &ObjectRuntime,
    entry_codes: &EntryCodes,
    definition: &LocalMacro,
    form: Word,
) -> Result<Word, FrontError> {
    let mut held: Held = vec![form];
    let mut env: Vec<(SymbolRef, usize)> = Vec::new();
    destructure(
        ctx,
        runtime,
        entry_codes,
        &definition.lambda_list,
        &mut held,
        &mut env,
    )?;
    let mut result = held_push(&mut held, Word::NIL);
    for expr in &definition.body {
        result = eval_expr(ctx, runtime, entry_codes, &mut held, &env, expr)?;
    }
    held_get(&held, result).map_err(object_error)
}

#[cfg(test)]
#[path = "tests/local_macro.rs"]
mod tests;
