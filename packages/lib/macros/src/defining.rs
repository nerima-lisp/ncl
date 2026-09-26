//! Expanders for Common Lisp definition forms.
#![allow(clippy::redundant_pub_crate)]
//!
//! The runtime does not yet expose a definition registry to this crate.  These
//! expanders therefore lower definitions to ordinary forms which can be
//! interpreted by the evaluator, instead of returning the definition form
//! unchanged.
#![allow(missing_docs, clippy::missing_errors_doc)]

use ncl_object::{
    ObjectError, Runtime, ThreadContext, Word, set_symbol_constant, set_symbol_special,
};

use crate::form::{elements, list, symbol};

const DEFINITION_PROPERTY: &str = "NCL::DEFINITION";

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
    let name = ensure_symbol(ctx, required(&parts, 1)?)?;
    let lambda_list = required(&parts, 2)?;
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
    let name = ensure_symbol(ctx, required(&parts, 1)?)?;
    let lambda_list = required(&parts, 2)?;
    function_definition(ctx, runtime, name, accessor, lambda_list, &parts[3..])
}

fn variable_definition(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    always: bool,
    constant: bool,
) -> Result<Word, ObjectError> {
    let parts = form_elements(ctx, required(args, 0)?)?;
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
    variable_definition(runtime, ctx, args, false, false)
}

fn defparameter(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    variable_definition(runtime, ctx, args, true, false)
}

fn defconstant(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    variable_definition(runtime, ctx, args, true, true)
}

fn define_symbol_macro(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let parts = form_elements(ctx, required(args, 0)?)?;
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
    let name = ensure_symbol(ctx, required(&parts, 1)?)?;
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
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    fn fixture() -> (Runtime, ThreadContext) {
        let runtime = Runtime::new().expect("runtime");
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("context");
        (runtime, ctx)
    }

    fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, form: Word) -> Word {
        let mut values = ncl_object::MultipleValues::new();
        callback_for(name).expect("callback")(runtime, ctx, &[form], &mut values).expect("expand")
    }

    #[test]
    fn definition_expansions_have_a_progn_and_quoted_name() {
        let (runtime, mut ctx) = fixture();
        let name = symbol(&mut ctx, &runtime, "X").expect("name");
        let operator = symbol(&mut ctx, &runtime, "DEFVAR").unwrap();
        let form = list(&mut ctx, &runtime, &[operator, name]).unwrap();
        let expanded = call(&runtime, &mut ctx, "DEFVAR", form);
        let parts = elements(&mut ctx, expanded).unwrap();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], symbol(&mut ctx, &runtime, "PROGN").unwrap());
        let result = elements(&mut ctx, parts[2]).unwrap();
        assert_eq!(result[0], symbol(&mut ctx, &runtime, "QUOTE").unwrap());
        assert_eq!(result[1], name);
    }

    #[test]
    fn function_definitions_preserve_lambda_body() {
        let (runtime, mut ctx) = fixture();
        let name = symbol(&mut ctx, &runtime, "F").unwrap();
        let lambda_list = symbol(&mut ctx, &runtime, "ARGS").unwrap();
        let body = symbol(&mut ctx, &runtime, "BODY").unwrap();
        let operator = symbol(&mut ctx, &runtime, "DEFUN").unwrap();
        let form = list(&mut ctx, &runtime, &[operator, name, lambda_list, body]).unwrap();
        let expanded = call(&runtime, &mut ctx, "DEFUN", form);
        let expanded_parts = elements(&mut ctx, expanded).unwrap();
        let setf = elements(&mut ctx, expanded_parts[1]).unwrap();
        let function = elements(&mut ctx, setf[2]).unwrap();
        let lambda = elements(&mut ctx, function[1]).unwrap();
        assert_eq!(lambda[1], lambda_list);
        assert_eq!(lambda[2], body);
    }

    #[test]
    fn malformed_definition_is_rejected() {
        let (runtime, mut ctx) = fixture();
        let operator = symbol(&mut ctx, &runtime, "DEFUN").unwrap();
        let form = list(&mut ctx, &runtime, &[operator]).unwrap();
        let mut values = ncl_object::MultipleValues::new();
        assert_eq!(
            defun(&runtime, &mut ctx, &[form], &mut values),
            Err(ObjectError::TypeError)
        );
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
