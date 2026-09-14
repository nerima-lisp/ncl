//! PROGV preserves escaped symbol case across reads, writes, and dynamic unwinding.

use ncl_runtime::{Runtime, RuntimeError, Value};
use rstest::rstest;

type EvalFn = fn(&Runtime, &str) -> Result<Vec<Value>, RuntimeError>;

#[rstest]
#[case::uppercase_control("(progv '(foo) '(42) @read@)", "foo", "42")]
#[case::lowercase("(progv '(|foo|) '(42) @read@)", "|foo|", "42")]
#[case::mixed_case("(progv '(|Foo|) '(43) @read@)", "|Foo|", "43")]
#[case::distinct_names(
    "(progv '(|foo| foo |Foo|) '(1 2 3) @read@)",
    "(list |foo| foo |Foo|)",
    "(1 2 3)"
)]
#[case::nested_normal_restore(
    "(progv '(|foo| foo) '(1 2) (list (progv '(|foo| foo) '(3 4) @read@) @read@))",
    "(list |foo| foo)",
    "((3 4) (1 2))"
)]
#[case::nested_throw_restore(
    "(progv '(|foo| foo) '(1 2) (list (catch 'exit (progv '(|foo| foo) '(3 4) (throw 'exit @read@))) @read@))",
    "(list |foo| foo)",
    "((3 4) (1 2))"
)]
#[case::setq_updates_only_exact_binding(
    "(progv '(|foo| foo) '(1 2) (list @read@ (symbol-value '|foo|) (symbol-value 'foo)))",
    "(setq |foo| 9)",
    "(9 9 2)"
)]
#[case::set_updates_only_exact_binding(
    "(progv '(|foo| foo) '(1 2) (set '|foo| 9) @read@)",
    "(list |foo| foo)",
    "(9 2)"
)]
#[case::normal_exit_unbinds(
    "(list (progv '(|foo| foo) '(1 2) @read@) (boundp '|foo|) (boundp 'foo))",
    "(list |foo| foo)",
    "((1 2) NIL NIL)"
)]
#[case::throw_exit_unbinds(
    "(list (catch 'exit (progv '(|foo| foo) '(1 2) (throw 'exit @read@))) (boundp '|foo|) (boundp 'foo))",
    "(list |foo| foo)",
    "((1 2) NIL NIL)"
)]
fn progv_symbol_case(
    #[case] template: &str,
    #[case] expression: &str,
    #[case] expected: &str,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
    #[values("direct", "eval", "funcall #'eval")] entrance: &str,
) {
    let expression = if entrance == "direct" {
        expression.to_string()
    } else {
        format!("({entrance} '{expression})")
    };
    let source = template.replace("@read@", &expression);
    let values = eval(&Runtime::new(), &source)
        .unwrap_or_else(|error| panic!("evaluation failed for {source}: {error:?}"));
    assert_eq!(values.len(), 1, "source: {source}");
    assert_eq!(values[0].to_string(), expected, "source: {source}");
}

#[rstest]
fn progv_error_restores_bindings(
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
) {
    let runtime = Runtime::new();
    let result = eval(
        &runtime,
        "(progv '(|foo| foo) '(1 2) (error \"progv unwind probe\"))",
    );
    assert!(result.is_err(), "body must signal an error");
    let Ok(values) = eval(&runtime, "(list (boundp '|foo|) (boundp 'foo))") else {
        panic!("dynamic bindings must be removed after an error");
    };
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(NIL NIL)");
}
