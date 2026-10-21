#![allow(missing_docs)]

use ncl_runtime::Runtime;

#[test]
fn logical_pathname_translations_setf_round_trips_by_host() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval(
            r#"(let ((first '(("**;*.*.*" "first/")))
                   (second '(("**;*.*.*" "second/"))))
               (and
                 (equal (setf (logical-pathname-translations "HostA") first) first)
                 (equal (logical-pathname-translations "hOsTa") first)
                 (equal (setf (logical-pathname-translations "HOSTB") second) second)
                 (equal (logical-pathname-translations "hosta") first)
                 (equal (logical-pathname-translations "HostB") second)))"#,
        )
        .unwrap_or_else(|error| panic!("logical pathname translations evaluation: {error}"));

    assert_eq!(runtime.format_result(value), "T");
}

#[test]
fn logical_pathname_translations_signal_type_error_for_unknown_host() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval(
            r#"(and
                 (null (setf (logical-pathname-translations "EMPTY-HOST") nil))
                 (null (logical-pathname-translations "empty-host"))
                 (handler-case
                   (progn (logical-pathname-translations "UnKnOwN-Host") nil)
                   (type-error (condition)
                     (and
                       (equal (type-error-datum condition) "UnKnOwN-Host")
                       (eq (type-error-expected-type condition) 'string)))))"#,
        )
        .unwrap_or_else(|error| panic!("unknown logical pathname host evaluation: {error}"));

    assert_eq!(runtime.format_result(value), "T");
}

#[test]
fn logical_pathname_translations_are_runtime_local() {
    let mut first_runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    first_runtime
        .eval(
            r#"(setf (logical-pathname-translations "HOST")
                     '(("**;*.*.*" "target/")))"#,
        )
        .unwrap_or_else(|error| panic!("setting first runtime translations: {error}"));

    let mut second_runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = second_runtime
        .eval(
            r#"(handler-case
                  (progn (logical-pathname-translations "HOST") nil)
                (type-error () t))"#,
        )
        .unwrap_or_else(|error| panic!("reading second runtime translations: {error}"));

    assert_eq!(second_runtime.format_result(value), "T");
}
