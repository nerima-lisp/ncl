#![allow(missing_docs)]

use ncl_runtime::Runtime;

fn new_runtime() -> Runtime {
    Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error:?}"))
}

fn eval_text(runtime: &mut Runtime, source: &str) -> String {
    let value = runtime
        .eval(source)
        .unwrap_or_else(|error| panic!("evaluation failed for {source}: {error:?}"));
    runtime.format_result(value)
}

#[test]
fn macroexpand_1_expands_once_and_returns_expanded_p() {
    let mut runtime = new_runtime();
    eval_text(
        &mut runtime,
        "(defmacro macroexpand-outer (x) `(macroexpand-inner ,x))",
    );

    assert_eq!(
        eval_text(
            &mut runtime,
            "(equal (multiple-value-list (macroexpand-1 '(macroexpand-outer 41)))\n             '((macroexpand-inner 41) t))",
        ),
        "T",
    );
    assert_eq!(
        eval_text(
            &mut runtime,
            "(multiple-value-list (macroexpand-1 '(+ 20 22)))"
        ),
        "((+ 20 22) NIL)",
    );
}

#[test]
fn macroexpand_repeats_expansion_and_reports_any_expansion() {
    let mut runtime = new_runtime();
    eval_text(
        &mut runtime,
        "(defmacro macroexpand-outer (x) `(macroexpand-inner ,x))",
    );
    eval_text(&mut runtime, "(defmacro macroexpand-inner (x) `(+ ,x 1))");

    assert_eq!(
        eval_text(
            &mut runtime,
            "(multiple-value-list (macroexpand '(macroexpand-outer 41)))",
        ),
        "((+ 41 1) T)",
    );
    assert_eq!(
        eval_text(
            &mut runtime,
            "(multiple-value-list (macroexpand '(+ 20 22)))"
        ),
        "((+ 20 22) NIL)",
    );
}

#[test]
fn macroexpand_uses_local_macros_from_the_environment_argument() {
    let mut runtime = new_runtime();
    eval_text(
        &mut runtime,
        "(defmacro macroexpand-env-check (form &environment env)\n         (macroexpand-1 form env))",
    );
    assert_eq!(
        eval_text(&mut runtime, "(macroexpand-env-check (+ 20 22))"),
        "42",
    );
    assert_eq!(
        eval_text(
            &mut runtime,
            "(eq (macrolet ((%m () ''foo))\n            (macroexpand-env-check (%m)))\n          'foo)",
        ),
        "T",
    );
    assert_eq!(
        eval_text(
            &mut runtime,
            "(eq (macrolet ((%m () ''foo))\n            (macrolet ((%m2 (&environment env)\n                        (macroexpand-1 '(%m) env)))\n              (%m2)))\n          'foo)",
        ),
        "T",
    );
    assert_eq!(
        eval_text(
            &mut runtime,
            "(eq (macrolet ((%m () ''foo))\n            (macrolet ((%m2 (&environment env)\n                        (macroexpand '(%m) env)))\n              (%m2)))\n          'foo)",
        ),
        "T",
    );
    assert_eq!(
        eval_text(
            &mut runtime,
            "(let ((macroexpand-symbol-y 'foo))\n           (symbol-macrolet ((macroexpand-symbol-x macroexpand-symbol-y))\n             (macrolet ((%m2 (&environment env)\n                         (macroexpand-1 'macroexpand-symbol-x env)))\n               (eq (%m2) 'foo))))",
        ),
        "T",
    );
    assert_eq!(
        eval_text(
            &mut runtime,
            "(equal (macrolet ((%m () ''foo))\n              (flet ((%m () 'bar))\n                (macrolet ((%m2 (&environment env)\n                            (list 'quote (macroexpand-1 '(%m) env))))\n                  (%m2))))\n            '(%m))",
        ),
        "T",
    );
}

#[test]
fn lexical_function_and_macro_lookup_uses_the_innermost_binding() {
    let mut runtime = new_runtime();
    assert_eq!(
        eval_text(
            &mut runtime,
            "(flet ((macroexpand-order-a () 1))\n\
               (macrolet ((macroexpand-order-a () 2))\n\
                 (macroexpand-order-a)))",
        ),
        "2",
    );
    assert_eq!(
        eval_text(
            &mut runtime,
            "(macrolet ((macroexpand-order-b () 2))\n\
               (flet ((macroexpand-order-b () 1))\n\
                 (macroexpand-order-b)))",
        ),
        "1",
    );

    eval_text(
        &mut runtime,
        "(defmacro macroexpand-lookup-probe-a (&environment env)\n\
           (list 'quote\n\
                 (multiple-value-list\n\
                   (macroexpand-1 '(macroexpand-order-c) env))))",
    );
    assert_eq!(
        eval_text(
            &mut runtime,
            "(flet ((macroexpand-order-c () 1))\n\
               (macrolet ((macroexpand-order-c () 2))\n\
                 (macroexpand-lookup-probe-a)))",
        ),
        "(2 T)",
    );

    eval_text(
        &mut runtime,
        "(defmacro macroexpand-lookup-probe-b (&environment env)\n\
           (list 'quote\n\
                 (multiple-value-list\n\
                   (macroexpand-1 '(macroexpand-order-d) env))))",
    );
    assert_eq!(
        eval_text(
            &mut runtime,
            "(equal\n\
               (macrolet ((macroexpand-order-d () 2))\n\
                 (flet ((macroexpand-order-d () 1))\n\
                   (macroexpand-lookup-probe-b)))\n\
               '((macroexpand-order-d) nil))",
        ),
        "T",
    );
}


#[test]
fn macroexpand_arguments_are_evaluated_once() {
    let mut runtime = new_runtime();
    assert_eq!(
        eval_text(
            &mut runtime,
            "(let ((form '(+ 1 2)) (i 0) a b)\n             (and (equal (multiple-value-list\n                           (macroexpand-1 (progn (setf a (incf i)) form)\n                                          (progn (setf b (incf i)) nil)))\n                         (list form nil))\n                  (equal (list i a b) '(2 1 2))))",
        ),
        "T",
    );
}

#[test]
fn macroexpand_uses_the_macroexpand_hook_for_global_macros() {
    let mut runtime = new_runtime();
    eval_text(&mut runtime, "(defmacro macroexpand-hook-test () 42)");

    assert_eq!(
        eval_text(
            &mut runtime,
            "(let ((*macroexpand-hook*\n             (lambda (expander form environment) :hooked)))\n         (multiple-value-list (macroexpand-1 '(macroexpand-hook-test))))",
        ),
        "(:HOOKED T)",
    );
}

#[test]
fn macroexpand_uses_the_hook_for_symbol_and_local_macros() {
    let mut symbol_runtime = new_runtime();
    eval_text(
        &mut symbol_runtime,
        "(setf *macroexpand-hook*\n       (lambda (expander form environment) :symbol-hooked))",
    );
    assert_eq!(
        eval_text(
            &mut symbol_runtime,
            "(symbol-macrolet ((macroexpand-hook-symbol t))\n         (macrolet ((%m (&environment env)\n                     (macroexpand-1 'macroexpand-hook-symbol env)))\n           (%m)))",
        ),
        ":SYMBOL-HOOKED",
    );

    let mut macro_runtime = new_runtime();
    eval_text(
        &mut macro_runtime,
        "(setf *macroexpand-hook*\n       (lambda (expander form environment) :local-hooked))",
    );
    assert_eq!(
        eval_text(
            &mut macro_runtime,
            "(macrolet ((%inner () 42)\n             (%outer (&environment env) (macroexpand-1 '(%inner) env)))\n         (%outer))",
        ),
        ":LOCAL-HOOKED",
    );
}

#[test]
fn macroexpand_preserves_symbol_macro_shadowing_and_arbitrary_forms() {
    let mut runtime = new_runtime();
    assert_eq!(
        eval_text(
            &mut runtime,
            "(symbol-macrolet ((macroexpand-quoted-x 'foo))\n         (macrolet ((%m (&environment env)\n                     (list 'quote (macroexpand-1 'macroexpand-quoted-x env))))\n           (equal (%m) ''foo)))",
        ),
        "T",
    );
    assert_eq!(
        eval_text(
            &mut runtime,
            "(let ((macroexpand-shadow-x 'foo))\n         (symbol-macrolet ((macroexpand-shadow-x macroexpand-shadow-y))\n           (macrolet ((%m (&environment env)\n                       (list 'quote (macroexpand-1 'macroexpand-shadow-x env))))\n             (equal (%m) 'macroexpand-shadow-x))))",
        ),
        "T",
    );
    assert_eq!(
        eval_text(
            &mut runtime,
            "(symbol-macrolet ((macroexpand-if-x (if t 'foo 'bar)))\n         (macrolet ((%m (&environment env)\n                     (list 'quote (macroexpand-1 'macroexpand-if-x env))))\n           (equal (%m) '(if t 'foo 'bar))))",
        ),
        "T",
    );
    assert_eq!(
        eval_text(
            &mut runtime,
            "(symbol-macrolet ((macroexpand-if-numeric-x (if t 1 2)))\n         (macrolet ((%m (&environment env)\n                     (list 'quote (macroexpand-1 'macroexpand-if-numeric-x env))))\n           (equal (%m) '(if t 1 2))))",
        ),
        "T",
    );
}
