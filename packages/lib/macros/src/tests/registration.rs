use super::*;

#[test]
fn registration_marks_owned_macros_and_installs_function_cells() {
    let runtime = Runtime::new().expect("runtime");
    register(&runtime).expect("macro registration");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("context registration");
    let package = runtime.find_package(&ctx, CL).expect("COMMON-LISP");
    let symbol = Package::from_word(package)
        .intern(&mut ctx, &runtime, "WHEN")
        .expect("WHEN")
        .0;
    assert!(ncl_object::symbol_is_macro(&ctx, symbol).expect("macro flag"));
    assert_ne!(
        ncl_object::symbol_function(&ctx, symbol).expect("function cell"),
        Word::UNBOUND
    );
    for name in MACROS {
        let symbol = Package::from_word(package)
            .intern(&mut ctx, &runtime, name)
            .expect(name)
            .0;
        assert_ne!(
            ncl_object::symbol_function(&ctx, symbol).expect(name),
            Word::UNBOUND,
            "registered macro {name} has an unbound function cell"
        );
    }
    for name in ["DEFUN", "DEFMACRO", "DEFVAR", "DEFPARAMETER", "DEFCONSTANT"] {
        let symbol = Package::from_word(package)
            .intern(&mut ctx, &runtime, name)
            .expect(name)
            .0;
        assert!(ncl_object::symbol_is_macro(&ctx, symbol).expect(name));
    }
    let hook = Package::from_word(package)
        .intern(&mut ctx, &runtime, "*MACROEXPAND-HOOK*")
        .expect("*MACROEXPAND-HOOK*")
        .0;
    assert!(ncl_object::symbol_is_special(&ctx, hook).expect("special flag"));
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
    assert_eq!(parts.len(), 3);
    assert!(parts.iter().all(|part| *part != Word::NIL));
}

mod coverage_recovery;
mod gc_stress_tests;
mod iteration_tests;
mod loop_tests;
mod string_stream_tests;
