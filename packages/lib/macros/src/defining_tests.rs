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
    let body_operator = symbol(&mut ctx, &runtime, "BODY")?;
    let body = list(&mut ctx, &runtime, &[body_operator])?;
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
fn function_definitions_allow_an_empty_body() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let name = symbol(&mut ctx, &runtime, "F")?;
    let operator = symbol(&mut ctx, &runtime, "DEFUN")?;
    let form = list(&mut ctx, &runtime, &[operator, name, Word::NIL])?;
    let expanded = call(&runtime, &mut ctx, "DEFUN", form)?;
    let expanded_parts = elements(&mut ctx, expanded)?;
    let setf = elements(&mut ctx, expanded_parts[1])?;
    let function = elements(&mut ctx, setf[2])?;
    let lambda = elements(&mut ctx, function[1])?;
    let block = elements(&mut ctx, lambda[2])?;
    assert_eq!(block[0], symbol(&mut ctx, &runtime, "BLOCK")?);
    assert_eq!(block[1], name);
    assert_eq!(block.len(), 2);
    Ok(())
}

#[test]
fn defun_expansion_survives_gc_stress_and_strict_forwarding() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let name = symbol(&mut ctx, &runtime, "F")?;
    let operator = symbol(&mut ctx, &runtime, "DEFUN")?;
    let parameter = symbol(&mut ctx, &runtime, "VALUE")?;
    let mut body = ncl_object::make_string(&mut ctx, &runtime, &['x'])?;
    ncl_object::with_root(&mut ctx, &mut body, |ctx, body| {
        let mut lambda_list = list(ctx, &runtime, &[parameter])?;
        ncl_object::with_root(ctx, &mut lambda_list, |ctx, lambda_list| {
            let form = list(
                ctx,
                &runtime,
                &[operator, name, lambda_list.get(), body.get()],
            )?;
            let roots = HeapRootedWords::new(ctx, &[form, body.get(), lambda_list.get()])?;
            ctx.set_strict_forwarding(true);
            ctx.set_gc_stress(true);

            let expanded = call(
                &runtime,
                ctx,
                "DEFUN",
                roots.first().ok_or(ObjectError::TypeError)?.get(),
            )?;
            let expanded = HeapRootedWord::new(ctx, expanded)?;
            ctx.collect(true)?;

            let expanded_parts = elements(ctx, expanded.get())?;
            let setf_form = expanded_parts
                .get(1)
                .copied()
                .ok_or(ObjectError::TypeError)?;
            let setf = elements(ctx, setf_form)?;
            let function_form = setf.get(2).copied().ok_or(ObjectError::TypeError)?;
            let function = elements(ctx, function_form)?;
            let lambda_form = function.get(1).copied().ok_or(ObjectError::TypeError)?;
            let lambda = elements(ctx, lambda_form)?;
            assert_eq!(
                lambda.get(1).copied().ok_or(ObjectError::TypeError)?,
                roots.get(2).ok_or(ObjectError::TypeError)?.get()
            );
            let block_form = lambda.get(2).copied().ok_or(ObjectError::TypeError)?;
            let block = elements(ctx, block_form)?;
            assert_eq!(
                block.get(2).copied().ok_or(ObjectError::TypeError)?,
                roots.get(1).ok_or(ObjectError::TypeError)?.get()
            );
            Ok(())
        })
    })
}

#[test]
fn function_definitions_keep_declarations_and_docstrings_before_the_block()
-> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let name = symbol(&mut ctx, &runtime, "F")?;
    let body = symbol(&mut ctx, &runtime, "BODY")?;
    let operator = symbol(&mut ctx, &runtime, "DEFUN")?;
    let declare = symbol(&mut ctx, &runtime, "DECLARE")?;
    let block_symbol = symbol(&mut ctx, &runtime, "BLOCK")?;
    let ignore = symbol(&mut ctx, &runtime, "IGNORE")?;
    let variable = symbol(&mut ctx, &runtime, "VALUE")?;
    let mut declaration_spec = list(&mut ctx, &runtime, &[ignore, variable])?;

    ncl_object::with_root(&mut ctx, &mut declaration_spec, |ctx, declaration_spec| {
        let mut declaration = list(ctx, &runtime, &[declare, declaration_spec.get()])?;
        ncl_object::with_root(ctx, &mut declaration, |ctx, declaration| {
            let mut docstring = ncl_object::make_string(ctx, &runtime, &['d', 'o', 'c'])?;
            ncl_object::with_root(ctx, &mut docstring, |ctx, docstring| {
                let form = list(
                    ctx,
                    &runtime,
                    &[
                        operator,
                        name,
                        Word::NIL,
                        declaration.get(),
                        docstring.get(),
                        body,
                    ],
                )?;
                let expanded = call(&runtime, ctx, "DEFUN", form)?;
                let expanded_parts = elements(ctx, expanded)?;
                let setf = elements(ctx, expanded_parts[1])?;
                let function = elements(ctx, setf[2])?;
                let lambda = elements(ctx, function[1])?;
                assert_eq!(lambda[2], declaration.get());
                assert_eq!(lambda[3], docstring.get());
                let block = elements(ctx, lambda[4])?;
                assert_eq!(block[0], block_symbol);
                assert_eq!(block[1], name);
                assert_eq!(block[2], body);
                Ok(())
            })
        })
    })
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
    assert_eq!(
        defun(&runtime, &mut ctx, &[form], &mut values),
        Err(ObjectError::TypeError)
    );
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
        "DEFTYPE",
        "DEFCONSTANT",
        "DEFINE-SYMBOL-MACRO",
        "DEFINE-COMPILER-MACRO",
        "DEFSETF",
        "DEFINE-SETF-EXPANDER",
    ] {
        assert!(callback_for(name).is_some(), "{name}");
    }
}
