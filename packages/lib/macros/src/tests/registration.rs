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

mod gc_stress_tests;
