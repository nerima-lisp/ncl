use super::*;

fn builtin(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> ncl_object::FunctionObject {
    ncl_object::FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).expect(name))
        .expect("function object")
}

#[test]
fn registration_marks_owned_macros_and_installs_function_cells() {
    let runtime = Runtime::new().expect("runtime");
    register(&runtime).expect("macro registration");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("context registration");
    let package = runtime.find_package(&ctx, CL).expect("COMMON-LISP");
    let symbol = Package::from_word(package)
        .intern(&mut ctx, &runtime, "WHEN")
        .unwrap()
        .0;
    assert!(ncl_object::symbol_is_macro(&ctx, symbol).unwrap());
    assert_ne!(
        ncl_object::symbol_function(&ctx, symbol).unwrap(),
        Word::UNBOUND
    );
    for name in MACROS {
        let symbol = Package::from_word(package)
            .intern(&mut ctx, &runtime, name)
            .unwrap()
            .0;
        assert_ne!(
            ncl_object::symbol_function(&ctx, symbol).unwrap(),
            Word::UNBOUND
        );
    }
    for name in ["DEFUN", "DEFMACRO", "DEFVAR", "DEFPARAMETER", "DEFCONSTANT"] {
        let symbol = Package::from_word(package)
            .intern(&mut ctx, &runtime, name)
            .unwrap()
            .0;
        assert!(ncl_object::symbol_is_macro(&ctx, symbol).unwrap());
    }
    let hook = Package::from_word(package)
        .intern(&mut ctx, &runtime, "*MACROEXPAND-HOOK*")
        .unwrap()
        .0;
    assert!(ncl_object::symbol_is_special(&ctx, hook).unwrap());
    assert!(
        runtime
            .function(&mut ctx, CL, "GET-SETF-EXPANSION")
            .is_some()
    );
}

#[test]
fn control_macro_expansions_preserve_test_and_body_or_reject_malformed_forms() {
    let runtime = Runtime::new().unwrap();
    register(&runtime).unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let when = symbol(&mut ctx, &runtime, "WHEN").unwrap();
    let test = symbol(&mut ctx, &runtime, "TEST").unwrap();
    let body = symbol(&mut ctx, &runtime, "BODY").unwrap();
    let form = list(&mut ctx, &runtime, &[when, test, body]).unwrap();
    let callback = callback_for("WHEN").unwrap();
    let expanded = callback(
        &mut ctx,
        &runtime,
        &ncl_object::BuiltinArgs::new(&[form]),
        &mut ncl_object::MultipleValues::new(),
    )
    .unwrap();
    let parts = elements(&mut ctx, expanded).unwrap();
    assert_eq!(parts[0], symbol(&mut ctx, &runtime, "IF").unwrap());
    assert_eq!(parts[1], test);
    let body_form = elements(&mut ctx, parts[2]).unwrap();
    assert_eq!(body_form[0], symbol(&mut ctx, &runtime, "PROGN").unwrap());
    assert_eq!(body_form[1], body);
    assert_eq!(
        callback(
            &mut ctx,
            &runtime,
            &ncl_object::BuiltinArgs::new(&[when]),
            &mut ncl_object::MultipleValues::new(),
        ),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn quasiquote_expansion_emits_data_constructor_and_handles_unquote() {
    let runtime = Runtime::new().unwrap();
    register(&runtime).unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let quasiquote = symbol(&mut ctx, &runtime, "QUASIQUOTE").unwrap();
    let value = symbol(&mut ctx, &runtime, "VALUE").unwrap();
    let datum = list(&mut ctx, &runtime, &[value]).unwrap();
    let form = list(&mut ctx, &runtime, &[quasiquote, datum]).unwrap();
    let callback = callback_for("QUASIQUOTE").unwrap();
    let expanded = callback(
        &mut ctx,
        &runtime,
        &ncl_object::BuiltinArgs::new(&[form]),
        &mut ncl_object::MultipleValues::new(),
    )
    .unwrap();
    let parts = elements(&mut ctx, expanded).unwrap();
    assert_eq!(parts[0], symbol(&mut ctx, &runtime, "CONS").unwrap());
    let quoted_value = elements(&mut ctx, parts[1]).unwrap();
    assert_eq!(
        quoted_value[0],
        symbol(&mut ctx, &runtime, "QUOTE").unwrap()
    );
    assert_eq!(quoted_value[1], value);
    let quoted_nil = elements(&mut ctx, parts[2]).unwrap();
    assert_eq!(quoted_nil[0], symbol(&mut ctx, &runtime, "QUOTE").unwrap());
    assert_eq!(quoted_nil[1], Word::NIL);
}

#[test]
fn function_and_multiple_value_builtins_preserve_their_contracts() {
    let runtime = Runtime::new().expect("runtime");
    register(&runtime).expect("macro registration");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("context registration");
    let identity = builtin(&runtime, &mut ctx, "IDENTITY");
    assert_eq!(
        runtime.call_builtin(&mut ctx, identity, &[Word::fixnum(7)]),
        Ok(Word::fixnum(7))
    );
    let not = builtin(&runtime, &mut ctx, "NOT");
    assert_eq!(
        runtime.call_builtin(&mut ctx, not, &[Word::NIL]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, not, &[Word::TRUE]),
        Ok(Word::NIL)
    );
    let null = builtin(&runtime, &mut ctx, "NULL");
    assert_eq!(
        runtime.call_builtin(&mut ctx, null, &[Word::TRUE]),
        Ok(Word::NIL)
    );
    let functionp = builtin(&runtime, &mut ctx, "FUNCTIONP");
    assert_eq!(
        runtime.call_builtin(&mut ctx, functionp, &[identity.as_word()]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, functionp, &[Word::fixnum(1)]),
        Ok(Word::NIL)
    );
    let values = builtin(&runtime, &mut ctx, "VALUES");
    assert_eq!(
        runtime.call_builtin(&mut ctx, values, &[Word::fixnum(1), Word::fixnum(2)]),
        Ok(Word::fixnum(1))
    );
    assert_eq!(ctx.values(), &[Word::fixnum(1), Word::fixnum(2)]);
    let values_list = builtin(&runtime, &mut ctx, "VALUES-LIST");
    let proper = list(&mut ctx, &runtime, &[Word::fixnum(3), Word::fixnum(4)]).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, values_list, &[proper]),
        Ok(Word::fixnum(3))
    );
    assert_eq!(ctx.values(), &[Word::fixnum(3), Word::fixnum(4)]);
    let dotted =
        ncl_object::make_cons(&mut ctx, &runtime, Word::fixnum(3), Word::fixnum(4)).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, values_list, &[dotted]),
        Err(ObjectError::TypeError)
    );
}

mod coverage_recovery;
mod gc_stress_tests;
mod iteration_tests;
mod loop_tests;
mod pprint_tests;
mod string_stream_tests;
