//! Generic definitions must escape lexical scopes without replacing local functions.

use ncl_runtime::{Runtime, RuntimeError, Value};
use rstest::rstest;

type EvalFn = fn(&Runtime, &str) -> Result<Vec<Value>, RuntimeError>;

#[rstest]
#[case::generic_and_method_inside_let(
    r"(let ((unused nil))
         (defgeneric probe-a (x))
         (defmethod probe-a ((x t)) 7)
         (probe-a nil))",
    "7"
)]
#[case::generic_and_method_escape_let(
    r"(progn
         (let ((unused nil))
           (defgeneric probe-b (x))
           (defmethod probe-b ((x t)) 7))
         (probe-b nil))",
    "7"
)]
#[case::method_captures_lexical_variable(
    r"(progn
         (defgeneric probe-c (x))
         (let ((captured 7))
           (defmethod probe-c ((x t)) captured))
         (probe-c nil))",
    "7"
)]
#[case::implicit_generic_escapes_let(
    r"(progn
         (let ((captured 7))
           (defmethod probe-d ((x t)) captured))
         (probe-d nil))",
    "7"
)]
#[case::local_function_shadows_global_generic(
    r"(let ((local-result
              (flet ((probe-e (x) 11))
                (defgeneric probe-e (x))
                (defmethod probe-e ((x t)) 7)
                (probe-e nil))))
         (list local-result (probe-e nil)))",
    "(11 7)"
)]
fn generic_bindings_are_global(
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
    #[case] source: &str,
    #[case] expected: &str,
) {
    let runtime = Runtime::new();
    let values = eval(&runtime, source)
        .unwrap_or_else(|error| panic!("evaluation failed for {source}: {error:?}"));
    assert_eq!(values.len(), 1, "expected one top-level form: {source}");
    let value = values
        .first()
        .unwrap_or_else(|| panic!("missing top-level result: {source}"));
    assert_eq!(value.to_string(), expected, "{source}");
}
