use super::support::MustExist;
use ncl_runtime::Runtime;

#[test]
fn compiled_with_input_from_string_binds_stream_and_reads_it() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(with-input-from-string (stream "alpha
")
                 (read-line))"#,
        )
        .must_exist();

    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), r#""alpha""#);
}

#[test]
fn compiled_with_input_from_string_honors_start_and_end_keywords() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(with-input-from-string (stream "012345" :start 2 :end 4)
                 (read-line))"#,
        )
        .must_exist();
    assert_eq!(values[0].to_string(), r#""23""#);
}

#[test]
fn evaluator_with_input_from_string_honors_start_and_end_keywords() {
    let values = Runtime::new()
        .eval_source(
            r#"(with-input-from-string (stream "012345" :start 2 :end 4)
                 (read-line))"#,
        )
        .must_exist();
    assert_eq!(values[0].to_string(), r#""23""#);
}

#[test]
fn compiled_with_output_to_string_binds_standard_output_and_returns_text() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(with-output-to-string (stream)
                 (write-string "alpha" stream)
                 (write-line " beta"))"#,
        )
        .must_exist();

    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), r#""alpha beta\n""#);
}
