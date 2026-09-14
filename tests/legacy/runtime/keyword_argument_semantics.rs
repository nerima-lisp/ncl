//! Positional optional arguments and leftmost keyword values agree across engines.

use ncl_runtime::{Runtime, RuntimeError, Value};
use rstest::rstest;

type EvalFn = fn(&Runtime, &str) -> Result<Vec<Value>, RuntimeError>;

#[rstest]
#[case::keyword_optional(
    "((lambda (&optional (x 1 xp) &key (y 2 yp)) (list x xp y yp)) :y)",
    "(:Y T 2 NIL)"
)]
#[case::optional_then_key("((lambda (&optional x &key y) (list x y)) :x :y 3)", "(:X 3)")]
#[case::escaped_optional(
    "((lambda (&optional x &key y) (list x y)) :|mixed| :y 3)",
    "(:|mixed| 3)"
)]
#[case::required_optional_rest(
    "((lambda (r &optional x y &rest tail &key z) (list r x y tail z)) 0 :x :y :z 4)",
    "(0 :X :Y (:Z 4) 4)"
)]
#[case::omitted_optional(
    "((lambda (&optional (x 1 xp) &key (y 2 yp)) (list x xp y yp)))",
    "(1 NIL 2 NIL)"
)]
#[case::duplicate("((lambda (&key (x 8 xp)) (list x xp)) :x 1 :x 2)", "(1 T)")]
#[case::duplicate_nil("((lambda (&key (x 8 xp)) (list x xp)) :x nil :x 2)", "(NIL T)")]
#[case::duplicate_escaped("((lambda (&key ((:|mixed| x))) x) :|mixed| 1 :|mixed| 2)", "1")]
#[case::duplicate_with_rest(
    "((lambda (&rest tail &key x) (list x tail)) :x 1 :x 2)",
    "(1 (:X 1 :X 2))"
)]
#[case::all_values_evaluated(
    "(let ((n 0)) (list ((lambda (&key x) x) :x (incf n) :x (incf n)) n))",
    "(1 2)"
)]
#[case::allow_first_true(
    "((lambda (&key x) x) :unknown 1 :allow-other-keys t :allow-other-keys nil :x 3)",
    "3"
)]
fn keyword_argument_values(
    #[case] source: &str,
    #[case] expected: &str,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
) {
    let values =
        eval(&Runtime::new(), source).unwrap_or_else(|error| panic!("{source}: {error:?}"));
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), expected, "{source}");
}

#[rstest]
#[case::optional_consumes_key_name("((lambda (&optional x &key y) (list x y)) :y 3)")]
#[case::allow_first_nil(
    "((lambda (&key x) x) :unknown 1 :allow-other-keys nil :allow-other-keys t)"
)]
fn keyword_argument_errors(
    #[case] source: &str,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
) {
    assert!(eval(&Runtime::new(), source).is_err(), "{source}");
}
