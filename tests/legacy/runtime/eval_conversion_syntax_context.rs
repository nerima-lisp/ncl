//! A binding named QUOTE must not turn its initializer into quoted data.

use ncl_runtime::{Runtime, RuntimeError, Value};
use rstest::rstest;

type EvalFn = fn(&Runtime, &str) -> Result<Vec<Value>, RuntimeError>;

fn check(runtime: &Runtime, eval: EvalFn, entrance: &str, source: &str, expected: &str) {
    let source = if entrance == "direct" {
        source.to_string()
    } else {
        format!("({entrance} '{source})")
    };
    let values = eval(runtime, &source)
        .unwrap_or_else(|error| panic!("evaluation failed for {source}: {error:?}"));
    assert_eq!(values.len(), 1, "expected one top-level form: {source}");
    assert_eq!(values[0].to_string(), expected, "source: {source}");
}

#[rstest]
#[case::let_quote_symbol_initializer("(let ((x 7)) (let ((quote x)) quote))", "7")]
#[case::let_quote_executable_initializer("(let ((x 7)) (let ((quote (+ x 1))) quote))", "8")]
#[case::let_ordinary_name_control("(let ((x 7)) (let ((answer x)) answer))", "7")]
#[case::optional_quote_default("(funcall (lambda (x &optional (quote x)) quote) 7)", "7")]
#[case::optional_quote_supplied_control(
    "(funcall (lambda (x &optional (quote x)) quote) 7 9)",
    "9"
)]
#[case::optional_ordinary_name_control("(funcall (lambda (x &optional (answer x)) answer) 7)", "7")]
fn binding_syntax_context(
    #[case] source: &str,
    #[case] expected: &str,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
    #[values("direct", "eval", "funcall #'eval")] entrance: &str,
) {
    check(&Runtime::new(), eval, entrance, source, expected);
}

#[rstest]
#[case::macro_let_quote(
    "(defmacro context-binding () '(let ((x 7)) (let ((quote x)) quote)))",
    "7"
)]
#[case::macro_let_ordinary_name_control(
    "(defmacro context-binding () '(let ((x 7)) (let ((answer x)) answer)))",
    "7"
)]
#[case::macro_optional_quote(
    "(defmacro context-binding () '(funcall (lambda (x &optional (quote x)) quote) 7))",
    "7"
)]
#[case::macro_optional_ordinary_name_control(
    "(defmacro context-binding () '(funcall (lambda (x &optional (answer x)) answer) 7))",
    "7"
)]
fn macro_binding_syntax_context(
    #[case] definition: &str,
    #[case] expected: &str,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
    #[values("direct", "eval", "funcall #'eval")] entrance: &str,
) {
    let runtime = Runtime::new();
    let definitions = eval(&runtime, definition)
        .unwrap_or_else(|error| panic!("macro definition failed for {definition}: {error:?}"));
    assert_eq!(definitions.len(), 1, "expected one macro definition");
    check(&runtime, eval, entrance, "(context-binding)", expected);
}
