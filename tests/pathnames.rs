//! End-to-end tests for pathname parsing, coercion, and filesystem behavior.

#![expect(
    clippy::expect_used,
    reason = "Pathname tests expect valid runtime setup and evaluation results."
)]

use ncl_runtime::Runtime;

#[test]
fn truename_missing_path_is_caught_by_ignore_errors() {
    let mut runtime = Runtime::new().expect("runtime");
    let namestring = format!("ncl-missing-path-{}.tmp", std::process::id());
    let source = format!("(ignore-errors (truename #P\"{namestring}\"))");
    let result = runtime.eval(&source).expect("ignore-errors evaluation");
    assert_eq!(runtime.format_result(result), "NIL");
}
