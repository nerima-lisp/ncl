use ncl_runtime::Runtime;

pub fn assert_error_contains(source: &str, expected: &str) {
    let interpreted = Runtime::new()
        .eval_source(source)
        .expect_err("interpreted evaluation should fail")
        .to_string();
    assert!(
        interpreted.contains(expected),
        "interpreted error {interpreted:?} should contain {expected:?}"
    );

    let compiled = Runtime::new()
        .eval_compiled_source(source)
        .expect_err("compiled evaluation should fail")
        .to_string();
    assert!(
        compiled.contains(expected),
        "compiled error {compiled:?} should contain {expected:?}"
    );
}
