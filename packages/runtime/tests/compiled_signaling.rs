//! Acceptance tests for compiled conditions and restart behavior.

use ncl_runtime::Runtime;

fn eval_compiled_zero_arg(runtime: &mut Runtime, lambda: &str) -> String {
    let source = format!("(funcall (compile nil '{lambda}))");
    let value = runtime
        .eval(&source)
        .unwrap_or_else(|error| panic!("compiled form failed: {error}"));
    runtime.format_result(value)
}

#[test]
fn compiled_car_type_error_is_caught_with_its_datum() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));

    // CLHS f_car_c.htm requires TYPE-ERROR for non-lists; f_tp_err.htm preserves the datum.
    // HANDLER-CASE returns the selected clause's value (CLHS m_hand_1.htm).
    assert_eq!(
        eval_compiled_zero_arg(
            &mut runtime,
            "(lambda () (handler-case (car 1) (type-error (c) (type-error-datum c))))",
        ),
        "1",
    );
}

#[test]
fn compiled_car_of_nil_returns_nil() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));

    // CLHS f_car_c.htm gives (car nil) => NIL.
    assert_eq!(
        eval_compiled_zero_arg(&mut runtime, "(lambda () (car nil))"),
        "NIL",
    );
}

#[test]
fn compiled_handler_case_preserves_the_normal_value() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));

    // CLHS m_hand_1.htm evaluates the protected form when no condition is signaled.
    assert_eq!(
        eval_compiled_zero_arg(
            &mut runtime,
            "(lambda () (handler-case 3 (type-error () :caught)))",
        ),
        "3",
    );
}

#[test]
fn compiled_car_character_type_error_is_caught() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));

    // CLHS f_car_c.htm requires TYPE-ERROR for any non-list argument; characters are non-lists.
    assert_eq!(
        eval_compiled_zero_arg(
            &mut runtime,
            "(lambda () (handler-case (car #\\a) \
               (type-error (condition) (characterp (type-error-datum condition)))))",
        ),
        "T",
    );
}

#[test]
fn compiled_cdr_type_error_is_caught_with_its_datum() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));

    // CLHS f_car_c.htm and f_tp_err.htm: CDR signals TYPE-ERROR for non-lists and preserves datum 1.
    assert_eq!(
        eval_compiled_zero_arg(
            &mut runtime,
            "(lambda () (handler-case (cdr 1) (type-error (c) (type-error-datum c))))",
        ),
        "1",
    );
}

#[test]
fn compiled_car_type_error_preserves_the_expected_type() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));

    // CLHS f_car_c.htm: CAR accepts a LIST; f_tp_err.htm exposes the expected type.
    assert_eq!(
        eval_compiled_zero_arg(
            &mut runtime,
            "(lambda () (handler-case (car 1) (type-error (c) (eq (type-error-expected-type c) 'list))))",
        ),
        "T",
    );
}

#[test]
fn compiled_safe_funcall_arity_error_is_a_catchable_program_error() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));

    let value = runtime
        .eval(
            "(funcall \
               (compile nil \
                 '(lambda (callee) \
                    (declare (optimize (safety 1))) \
                    (handler-case (funcall callee) \
                      (program-error () :caught)))) \
               #'car)",
        )
        .unwrap_or_else(|error| panic!("compiled safe-call failed: {error}"));

    // CLHS 3.5.1.2: too few arguments in a safe call signal PROGRAM-ERROR.
    assert_eq!(runtime.format_result(value), ":CAUGHT");
}

#[test]
fn compiled_cerror_continue_restart_resumes_the_call() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));

    // CLHS f_cerror.htm: invoking CONTINUE returns from CERROR to the following form.
    assert_eq!(
        eval_compiled_zero_arg(
            &mut runtime,
            "(lambda () (handler-bind ((simple-error (lambda (condition) (continue)))) (cerror \"Continue\" \"compiled restart probe\") :continued))",
        ),
        ":CONTINUED",
    );
}
