//! Expanders for Common Lisp definition forms.
#![allow(clippy::redundant_pub_crate)]
//!
//! These expanders lower definitions to ordinary forms so the evaluator
//! performs the definition through the runtime-owned generalized places.
#![allow(missing_docs, clippy::missing_errors_doc)]

use ncl_object::{
    HeapRootedWord, HeapRootedWords, ObjectError, Runtime, ThreadContext, Word,
    set_symbol_constant, set_symbol_special,
};
use ncl_types::lambda_body::{LambdaBodyElement, LambdaBodyPrefix, split_lambda_body_prefix};

use crate::form::{elements, list, symbol};

mod deftype;
mod deftype_lambda_list;
mod macro_definition;
use deftype::deftype;
use macro_definition::{finish_function_definition, macro_function_definition};

const DEFINITION_PROPERTY: &str = "NCL::DEFINITION";
const DEFTYPE_PROPERTY: &str = "NCL::DEFTYPE-EXPANDER";
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
        "DEFTYPE" => deftype,
        "DEFCONSTANT" => defconstant,
        "DEFINE-SYMBOL-MACRO" => define_symbol_macro,
        "DEFINE-COMPILER-MACRO" => define_compiler_macro,
        "DEFSETF" => defsetf,
        "DEFINE-SETF-EXPANDER" => define_setf_expander,
        _ => return None,
    })
}

/// Return the callback for a definition macro, if `name` is one of ours.
pub(crate) fn form_elements(ctx: &mut ThreadContext, form: Word) -> Result<Vec<Word>, ObjectError> {
    elements(ctx, form)
}

pub(crate) fn required(elements: &[Word], index: usize) -> Result<Word, ObjectError> {
    elements.get(index).copied().ok_or(ObjectError::TypeError)
}

fn ensure_symbol(ctx: &ThreadContext, value: Word) -> Result<Word, ObjectError> {
    ncl_object::symbol_name(ctx, value).map(|_| value)
}

pub(crate) fn ensure_form_operator(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    parts: &[Word],
    name: &str,
) -> Result<(), ObjectError> {
    let mut actual = required(parts, 0)?;
    ncl_object::with_root(ctx, &mut actual, |ctx, actual| {
        if actual.get() != symbol(ctx, runtime, name)? {
            return Err(ObjectError::TypeError);
        }
        Ok(())
    })
}

fn ensure_list(ctx: &mut ThreadContext, value: Word) -> Result<(), ObjectError> {
    elements(ctx, value).map(|_| ())
}

pub(crate) fn quote(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    mut value: Word,
) -> Result<Word, ObjectError> {
    ncl_object::with_root(ctx, &mut value, |ctx, value| {
        let mut quote = symbol(ctx, runtime, "QUOTE")?;
        ncl_object::with_root(ctx, &mut quote, |ctx, quote| {
            list(ctx, runtime, &[quote.get(), value.get()])
        })
    })
}

fn progn_with_definition(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: Word,
    operation: Word,
) -> Result<Word, ObjectError> {
    ncl_object::with_roots(ctx, &[name, operation], |ctx, roots| {
        let mut progn = symbol(ctx, runtime, "PROGN")?;
        ncl_object::with_root(ctx, &mut progn, |ctx, progn| {
            let mut quoted_name = quote(
                ctx,
                runtime,
                roots.first().ok_or(ObjectError::TypeError)?.get(),
            )?;
            ncl_object::with_root(ctx, &mut quoted_name, |ctx, quoted_name| {
                list(
                    ctx,
                    runtime,
                    &[
                        progn.get(),
                        roots.get(1).ok_or(ObjectError::TypeError)?.get(),
                        quoted_name.get(),
                    ],
                )
            })
        })
    })
}

fn lambda_body_prefix(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    body: &[HeapRootedWord],
) -> Result<LambdaBodyPrefix, ObjectError> {
    let mut declaration = symbol(ctx, runtime, "DECLARE")?;
    ncl_object::with_root(ctx, &mut declaration, |ctx, declaration| {
        let mut elements = Vec::with_capacity(body.len());
        for form in body {
            let word = form.get();
            let element = if word.is_cons() && ncl_object::car(ctx, word)? == declaration.get() {
                LambdaBodyElement::Declaration
            } else if matches!(
                ncl_object::classify_object(ctx, word),
                ncl_object::ObjectRef::String(_)
            ) {
                LambdaBodyElement::String
            } else {
                LambdaBodyElement::Form
            };
            elements.push(element);
        }
        Ok(split_lambda_body_prefix(&elements))
    })
}

fn function_definition(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &HeapRootedWord,
    accessor: &str,
    lambda_list: &HeapRootedWord,
    body: &[HeapRootedWord],
) -> Result<Word, ObjectError> {
    let body_prefix = lambda_body_prefix(ctx, runtime, body)?;
    let preserved_body = body
        .get(..body_prefix.len())
        .ok_or(ObjectError::TypeError)?;
    let block_body = body
        .get(body_prefix.len()..)
        .ok_or(ObjectError::TypeError)?;
    let mut lambda_symbol = symbol(ctx, runtime, "LAMBDA")?;
    ncl_object::with_root(ctx, &mut lambda_symbol, |ctx, lambda_symbol| {
        let mut lambda = list(ctx, runtime, &[*lambda_symbol, lambda_list.get()])?;
        ncl_object::with_root(ctx, &mut lambda, |ctx, lambda| {
            let preserved_body = preserved_body
                .iter()
                .map(HeapRootedWord::get)
                .collect::<Vec<_>>();
            let mut lambda = append(ctx, runtime, *lambda, &preserved_body)?;
            ncl_object::with_root(ctx, &mut lambda, |ctx, lambda| {
                let mut block_symbol = symbol(ctx, runtime, "BLOCK")?;
                ncl_object::with_root(ctx, &mut block_symbol, |ctx, block_symbol| {
                    let mut block = list(ctx, runtime, &[block_symbol.get(), name.get()])?;
                    ncl_object::with_root(ctx, &mut block, |ctx, block| {
                        let block_body = block_body
                            .iter()
                            .map(HeapRootedWord::get)
                            .collect::<Vec<_>>();
                        let mut block = append(ctx, runtime, *block, &block_body)?;
                        ncl_object::with_root(ctx, &mut block, |ctx, block| {
                            let lambda = append(ctx, runtime, lambda.get(), &[block.get()])?;
                            finish_function_definition(ctx, runtime, name.get(), accessor, lambda)
                        })
                    })
                })
            })
        })
    })
}

fn property_definition(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: Word,
    property: &str,
    value: Word,
) -> Result<Word, ObjectError> {
    ncl_object::with_roots(ctx, &[name, value], |ctx, roots| {
        let mut get_symbol = symbol(ctx, runtime, "GET")?;
        ncl_object::with_root(ctx, &mut get_symbol, |ctx, get_symbol| {
            let mut quoted_name = quote(
                ctx,
                runtime,
                roots.first().ok_or(ObjectError::TypeError)?.get(),
            )?;
            ncl_object::with_root(ctx, &mut quoted_name, |ctx, quoted_name| {
                let property_symbol = symbol(ctx, runtime, property)?;
                let mut quoted_property = quote(ctx, runtime, property_symbol)?;
                ncl_object::with_root(ctx, &mut quoted_property, |ctx, quoted_property| {
                    // `(get name indicator)`'s indicator is evaluated: it
                    // must be the quoted property symbol (a bare one would
                    // compile as a read of the variable of that name, which
                    // is never bound).
                    let mut place = list(
                        ctx,
                        runtime,
                        &[get_symbol.get(), quoted_name.get(), quoted_property.get()],
                    )?;
                    ncl_object::with_root(ctx, &mut place, |ctx, place| {
                        let mut setf_symbol = symbol(ctx, runtime, "SETF")?;
                        ncl_object::with_root(ctx, &mut setf_symbol, |ctx, setf_symbol| {
                            let operation = list(
                                ctx,
                                runtime,
                                &[
                                    setf_symbol.get(),
                                    place.get(),
                                    roots.get(1).ok_or(ObjectError::TypeError)?.get(),
                                ],
                            )?;
                            progn_with_definition(
                                ctx,
                                runtime,
                                roots.first().ok_or(ObjectError::TypeError)?.get(),
                                operation,
                            )
                        })
                    })
                })
            })
        })
    })
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
    let parts = HeapRootedWords::new(ctx, &parts)?;
    let mut operator = symbol(ctx, runtime, "DEFUN")?;
    ncl_object::with_root(ctx, &mut operator, |ctx, operator| {
        if parts.first().ok_or(ObjectError::TypeError)?.get() != operator.get() {
            return Err(ObjectError::TypeError);
        }
        let name = parts.get(1).ok_or(ObjectError::TypeError)?;
        ensure_symbol(ctx, name.get())?;
        let lambda_list = parts.get(2).ok_or(ObjectError::TypeError)?;
        ensure_list(ctx, lambda_list.get())?;
        let body = parts.as_slice().get(3..).ok_or(ObjectError::TypeError)?;
        function_definition(ctx, runtime, name, "FDEFINITION", lambda_list, body)
    })
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
    ncl_object::with_roots(ctx, &parts, |ctx, parts| {
        let parts = parts.iter().map(|value| **value).collect::<Vec<_>>();
        ensure_form_operator(ctx, runtime, &parts, "DEFMACRO")?;
        let name = ensure_symbol(ctx, parts.get(1).copied().ok_or(ObjectError::TypeError)?)?;
        // Unlike an ordinary lambda list (`ensure_list`, used by `defun`), a
        // macro lambda list may end in a dotted tail (an implicit `&rest`),
        // so it is not required to be a proper list here.
        let lambda_list = parts.get(2).copied().ok_or(ObjectError::TypeError)?;
        let body = parts.get(3..).ok_or(ObjectError::TypeError)?;
        macro_function_definition(ctx, runtime, name, accessor, lambda_list, body)
    })
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
    ncl_object::with_roots(ctx, &parts, |ctx, parts| {
        let parts = parts.iter().map(|value| **value).collect::<Vec<_>>();
        ensure_form_operator(ctx, runtime, &parts, operator)?;
        let name = ensure_symbol(ctx, parts.get(1).copied().ok_or(ObjectError::TypeError)?)?;
        set_symbol_special(ctx, name, true)?;
        if constant {
            set_symbol_constant(ctx, name, true)?;
        }
        let value = parts.get(2).copied().unwrap_or(Word::NIL);
        ncl_object::with_roots(ctx, &[name, value], |ctx, roots| {
            let name = **roots.first().ok_or(ObjectError::TypeError)?;
            let value = **roots.get(1).ok_or(ObjectError::TypeError)?;
            let mut setq_symbol = symbol(ctx, runtime, "SETQ")?;
            ncl_object::with_root(ctx, &mut setq_symbol, |ctx, setq_symbol| {
                let mut setq = list(ctx, runtime, &[*setq_symbol, name, value])?;
                ncl_object::with_root(ctx, &mut setq, |ctx, setq| {
                    let operation = if always || constant {
                        *setq
                    } else {
                        let mut unless_symbol = symbol(ctx, runtime, "UNLESS")?;
                        ncl_object::with_root(ctx, &mut unless_symbol, |ctx, unless_symbol| {
                            let mut boundp_symbol = symbol(ctx, runtime, "BOUNDP")?;
                            ncl_object::with_root(ctx, &mut boundp_symbol, |ctx, boundp_symbol| {
                                let mut quoted_name = quote(ctx, runtime, name)?;
                                ncl_object::with_root(ctx, &mut quoted_name, |ctx, quoted_name| {
                                    let mut boundp =
                                        list(ctx, runtime, &[*boundp_symbol, *quoted_name])?;
                                    ncl_object::with_root(ctx, &mut boundp, |ctx, boundp| {
                                        list(ctx, runtime, &[*unless_symbol, *boundp, *setq])
                                    })
                                })
                            })
                        })?
                    };
                    progn_with_definition(ctx, runtime, name, operation)
                })
            })
        })
    })
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
    ncl_object::with_roots(ctx, &parts, |ctx, parts| {
        let parts = parts.iter().map(|value| **value).collect::<Vec<_>>();
        ensure_form_operator(ctx, runtime, &parts, "DEFINE-SYMBOL-MACRO")?;
        let name = ensure_symbol(ctx, parts.get(1).copied().ok_or(ObjectError::TypeError)?)?;
        let expansion = parts.get(2).copied().ok_or(ObjectError::TypeError)?;
        property_definition(ctx, runtime, name, "SYMBOL-MACRO", expansion)
    })
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
    ncl_object::with_roots(ctx, &parts, |ctx, roots| {
        let parts = roots.iter().map(|value| value.get()).collect::<Vec<_>>();
        ensure_form_operator(ctx, runtime, &parts, "DEFSETF")?;
        let name = roots.get(1).ok_or(ObjectError::TypeError)?.get();
        ensure_symbol(ctx, name)?;
        if roots.len() < 3 {
            return Err(ObjectError::TypeError);
        }
        let mut quote_symbol = symbol(ctx, runtime, "QUOTE")?;
        ncl_object::with_root(ctx, &mut quote_symbol, |ctx, quote_symbol| {
            let definitions = roots
                .iter()
                .skip(2)
                .map(|value| value.get())
                .collect::<Vec<_>>();
            let mut definition = list(ctx, runtime, &definitions)?;
            ncl_object::with_root(ctx, &mut definition, |ctx, definition| {
                let value = list(ctx, runtime, &[quote_symbol.get(), definition.get()])?;
                property_definition(
                    ctx,
                    runtime,
                    roots.get(1).ok_or(ObjectError::TypeError)?.get(),
                    DEFINITION_PROPERTY,
                    value,
                )
            })
        })
    })
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
    deftype_adapter => deftype,
    defconstant_adapter => defconstant,
    define_symbol_macro_adapter => define_symbol_macro,
    define_compiler_macro_adapter => define_compiler_macro,
    defsetf_adapter => defsetf,
    define_setf_expander_adapter => define_setf_expander,
}

#[cfg(test)]
#[path = "defining_tests.rs"]
mod tests;
