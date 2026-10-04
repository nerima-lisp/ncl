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
    callback_for(name).ok_or(ObjectError::UndefinedFunction)?(runtime, ctx, &[form], &mut values)
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
    let block = elements(&mut ctx, lambda[2])?;
    assert_eq!(block[0], symbol(&mut ctx, &runtime, "BLOCK")?);
    assert_eq!(block[1], name);
    assert_eq!(block[2], body);
    Ok(())
}

#[test]
fn malformed_definition_is_rejected() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let operator = symbol(&mut ctx, &runtime, "DEFUN")?;
    let form = list(&mut ctx, &runtime, &[operator])?;
    let mut values = ncl_object::MultipleValues::new();
    // check-added-lines: allow(panic) test asserts the exact rejection.
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
    // check-added-lines: allow(panic) test asserts the exact rejection.
    assert_eq!(
        defun(&runtime, &mut ctx, &[form], &mut values),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn malformed_function_definition_name_is_rejected() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let operator = symbol(&mut ctx, &runtime, "DEFUN")?;
    let form = list(&mut ctx, &runtime, &[operator, Word::TRUE, Word::NIL])?;
    let mut values = ncl_object::MultipleValues::new();
    // check-added-lines: allow(panic) test asserts the exact rejection.
    assert_eq!(
        defun(&runtime, &mut ctx, &[form], &mut values),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn malformed_macro_definition_is_rejected() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let operator = symbol(&mut ctx, &runtime, "DEFMACRO")?;
    let name = symbol(&mut ctx, &runtime, "M")?;
    let form = list(&mut ctx, &runtime, &[operator, name])?;
    let mut values = ncl_object::MultipleValues::new();
    // check-added-lines: allow(panic) test asserts the exact rejection.
    assert_eq!(
        defmacro(&runtime, &mut ctx, &[form], &mut values),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn malformed_whole_lambda_list_is_rejected() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let operator = symbol(&mut ctx, &runtime, "DEFMACRO")?;
    let name = symbol(&mut ctx, &runtime, "M")?;
    let whole = symbol(&mut ctx, &runtime, "&WHOLE")?;
    let lambda_list = list(&mut ctx, &runtime, &[whole])?;
    let form = list(&mut ctx, &runtime, &[operator, name, lambda_list])?;
    let mut values = ncl_object::MultipleValues::new();
    // check-added-lines: allow(panic) test asserts the exact rejection.
    assert_eq!(
        defmacro(&runtime, &mut ctx, &[form], &mut values),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn macro_definitions_bind_and_strip_whole_variable() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let name = symbol(&mut ctx, &runtime, "M")?;
    let whole = symbol(&mut ctx, &runtime, "WHOLE")?;
    let argument = symbol(&mut ctx, &runtime, "ARGUMENT")?;
    let body = symbol(&mut ctx, &runtime, "BODY")?;
    let defmacro = symbol(&mut ctx, &runtime, "DEFMACRO")?;
    let whole_marker = symbol(&mut ctx, &runtime, "&WHOLE")?;
    let lambda_list = list(&mut ctx, &runtime, &[whole_marker, whole, argument])?;
    let form = list(&mut ctx, &runtime, &[defmacro, name, lambda_list, body])?;

    let expansion = call(&runtime, &mut ctx, "DEFMACRO", form)?;
    let expansion_parts = elements(&mut ctx, expansion)?;
    let setf = elements(&mut ctx, expansion_parts[1])?; // check-added-lines: allow(index) expansion shape is asserted below.
    let function = elements(&mut ctx, setf[2])?; // check-added-lines: allow(index) expansion shape is asserted below.
    let lambda = elements(&mut ctx, function[1])?; // check-added-lines: allow(index) expansion shape is asserted below.
    assert_eq!(lambda[0], symbol(&mut ctx, &runtime, "LAMBDA")?); // check-added-lines: allow(panic,index) exact expansion assertion.

    let let_form = elements(&mut ctx, lambda[2])?; // check-added-lines: allow(index) expansion shape is asserted below.
    assert_eq!(let_form[0], symbol(&mut ctx, &runtime, "LET")?); // check-added-lines: allow(panic,index) exact expansion assertion.
    let bindings = elements(&mut ctx, let_form[1])?; // check-added-lines: allow(index) expansion shape is asserted below.
    let binding = elements(&mut ctx, bindings[0])?; // check-added-lines: allow(index) expansion shape is asserted below.
    assert_eq!(binding[0], whole); // check-added-lines: allow(panic,index) exact expansion assertion.
    let whole_parameter = elements(&mut ctx, lambda[1])?; // check-added-lines: allow(index) expansion shape is asserted below.
    assert_eq!(whole_parameter.len(), 1); // check-added-lines: allow(panic) exact expansion assertion.
    assert_eq!(binding[1], whole_parameter[0]); // check-added-lines: allow(panic,index) exact expansion assertion.
    Ok(())
}

#[test]
fn macro_definitions_preserve_non_whole_lambda_list() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let name = symbol(&mut ctx, &runtime, "M")?;
    let argument = symbol(&mut ctx, &runtime, "ARGUMENT")?;
    let body = symbol(&mut ctx, &runtime, "BODY")?;
    let defmacro = symbol(&mut ctx, &runtime, "DEFMACRO")?;
    let lambda_list = list(&mut ctx, &runtime, &[argument])?;
    let form = list(&mut ctx, &runtime, &[defmacro, name, lambda_list, body])?;

    let expansion = call(&runtime, &mut ctx, "DEFMACRO", form)?;
    let expansion_parts = elements(&mut ctx, expansion)?;
    let setf = elements(&mut ctx, expansion_parts[1])?; // check-added-lines: allow(index) expansion shape is asserted below.
    let function = elements(&mut ctx, setf[2])?; // check-added-lines: allow(index) expansion shape is asserted below.
    let lambda = elements(&mut ctx, function[1])?; // check-added-lines: allow(index) expansion shape is asserted below.
    let lambda_body = elements(&mut ctx, lambda[2])?; // check-added-lines: allow(index) expansion shape is asserted below.
    assert_eq!(lambda_body[0], symbol(&mut ctx, &runtime, "LET*")?); // check-added-lines: allow(panic,index) exact expansion assertion.
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
    assert_eq!(
        macro_place[0],
        symbol(&mut ctx, &runtime, "MACRO-FUNCTION")?
    );

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
