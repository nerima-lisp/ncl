use ncl_runtime::Runtime;

fn evaluate_interpreted(source: &str) -> Result<String, String> {
    Runtime::new()
        .eval_source(source)
        .map_err(|error| error.to_string())
        .and_then(|mut values| {
            values
                .pop()
                .map(|value| value.to_string())
                .ok_or_else(|| "evaluation returned no values".to_string())
        })
}

fn evaluate_compiled(source: &str) -> Result<String, String> {
    Runtime::new()
        .eval_compiled_source(source)
        .map_err(|error| error.to_string())
        .and_then(|mut values| {
            values
                .pop()
                .map(|value| value.to_string())
                .ok_or_else(|| "compiled evaluation returned no values".to_string())
        })
}

fn assert_interpreted_and_compiled(source: &str, expected: &str) {
    assert_eq!(
        evaluate_interpreted(source),
        Ok(expected.to_string()),
        "interpreted evaluation of {source:?}",
    );
    assert_eq!(
        evaluate_compiled(source),
        Ok(expected.to_string()),
        "compiled evaluation of {source:?}",
    );
}

fn assert_error_contains(source: &str, expected: &str) {
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

#[test]
fn normal_result_runs_cleanup_and_allows_no_cleanup_forms() {
    assert_interpreted_and_compiled("(unwind-protect 7)", "7");
    assert_interpreted_and_compiled(
        "(let ((marker nil))
           (list (unwind-protect 7 (setq marker :cleaned)) marker))",
        "(7 :CLEANED)",
    );
}

#[test]
fn protected_multiple_values_are_preserved_after_cleanup() {
    assert_interpreted_and_compiled(
        "(let ((marker nil))
           (multiple-value-bind (first second)
               (unwind-protect (values 1 2) (setq marker :cleaned))
             (list first second marker)))",
        "(1 2 :CLEANED)",
    );
}

#[test]
fn cleanup_runs_after_an_ordinary_protected_error() {
    assert_interpreted_and_compiled(
        "(let ((marker nil))
           (ignore-errors
             (unwind-protect (+ 1 \"protected\") (setq marker :cleaned)))
           marker)",
        ":CLEANED",
    );
}

#[test]
fn cleanup_runs_after_protected_return_from() {
    assert_interpreted_and_compiled(
        "(let ((marker nil))
           (list
             (block done
               (unwind-protect (return-from done 7) (setq marker :cleaned)))
             marker))",
        "(7 :CLEANED)",
    );
}

#[test]
fn cleanup_return_from_overrides_the_protected_result() {
    assert_interpreted_and_compiled("(block done (unwind-protect 1 (return-from done 9)))", "9");
}

#[test]
fn cleanup_error_overrides_the_protected_error() {
    assert_error_contains(
        r#"(unwind-protect (+ 1 "protected") cleanup-error)"#,
        "CLEANUP-ERROR",
    );
}

#[test]
fn protected_form_is_required() {
    assert_error_contains("(unwind-protect)", "at least one");
}

#[test]
fn go_runs_cleanup_before_resuming_at_tag() {
    assert_interpreted_and_compiled(
        r#"
        (let ((marker nil))
          (tagbody
            start
            (unwind-protect
                (go done)
              (setq marker :cleaned))
            done)
          marker)
        "#,
        ":CLEANED",
    );
}
