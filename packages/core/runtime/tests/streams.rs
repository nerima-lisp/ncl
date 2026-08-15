use ncl_runtime::{Runtime, Value};

fn evaluate(source: &str) -> Value {
    Runtime::new()
        .eval_source(source)
        .unwrap()
        .last()
        .cloned()
        .unwrap()
}

fn evaluate_compiled(source: &str) -> Value {
    Runtime::new()
        .eval_compiled_source(source)
        .expect("compiled evaluation should succeed")
        .last()
        .cloned()
        .expect("compiled source should return a value")
}

fn assert_both(source: &str, expected: &str) {
    assert_eq!(evaluate(source).to_string(), expected);
    assert_eq!(evaluate_compiled(source).to_string(), expected);
}

#[test]
fn with_output_to_string_captures_stream_writes() {
    assert_both(
        r#"(with-output-to-string (output)
             (write-string "alpha" output)
             (write-char #\space output)
             (write-string "beta" output))"#,
        r#""alpha beta""#,
    );
}

#[test]
fn with_output_to_string_appends_to_fill_pointer_vector() {
    assert_both(
        r#"(let ((buffer (make-array 10 :fill-pointer 2 :initial-contents '(#\o #\k #\? #\? #\? #\? #\? #\? #\? #\?))))
             (list
               (with-output-to-string (output buffer)
                 (write-char #\! output)
                 :done)
               (coerce (subseq buffer 0 (fill-pointer buffer)) 'string)
               (fill-pointer buffer)))"#,
        "(:DONE \"ok!\" 3)",
    );
}

#[test]
fn with_output_to_string_second_form_preserves_multiple_values() {
    assert_both(
        r#"(let ((buffer (make-array 8 :fill-pointer 0 :initial-element #\space)))
             (multiple-value-list
               (with-output-to-string (output buffer)
                 (write-string "xy" output)
                 (values :first :second))))"#,
        "(:FIRST :SECOND)",
    );
}

#[test]
fn with_input_from_string_reads_the_bound_stream() {
    assert_both(
        r#"(with-input-from-string (input "abc")
             (list (read-char input) (read-char input) (read-line input)))"#,
        r#"(#\a #\b "c")"#,
    );
}

#[test]
fn with_input_from_string_honors_start_and_end() {
    assert_both(
        r#"(with-input-from-string (input "abcdef" :start 1 :end 4)
             (list (read-char input)
                   (read-char input)
                   (read-char input)
                   (read-char input nil)))"#,
        "(#\\b #\\c #\\d NIL)",
    );
}

#[test]
fn with_input_from_string_reports_final_index() {
    assert_both(
        r#"(let ((position -1))
             (list
               (with-input-from-string (input "abcdef" :start 1 :end 5 :index position)
                 (read-char input)
                 (read-char input)
                 (values :done :ignored))
               position))"#,
        "(:DONE 3)",
    );
}

#[test]
fn with_input_from_string_does_not_update_index_after_error() {
    assert_both(
        r#"(let ((position -1))
             (list
               (ignore-errors
                 (with-input-from-string (input "abcdef" :start 1 :end 5 :index position)
                   (read-char input)
                   (error "boom")))
               position))"#,
        "(NIL -1)",
    );
}
