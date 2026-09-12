use std::fs;

use ncl_runtime::Runtime;
use rstest::rstest;

use super::EvalFn;
use super::support::evaluate_with;

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn dynamic_standard_stream_bindings_route_character_io(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((output (make-string-output-stream)))
               (let ((*standard-output* output))
                 (write-string \"x\")
                 (terpri))
               (get-output-stream-string output))"
        )
        .to_string(),
        "\"x\\n\""
    );
    assert_eq!(
        evaluate(
            "(let ((input (make-string-input-stream \"z\")))
               (let ((*standard-input* input))
                 (read-char)))"
        )
        .to_string(),
        "#\\z"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn write_string_and_write_line_honor_ranges(#[case] eval_fn: EvalFn) {
    let source = r#"(let ((output (make-string-output-stream)))
                     (let ((*standard-output* output))
                       (write-string "a😀bc" :start 1 :end 3)
                       (write-line "abcdef" output :start 2 :end 4))
                     (get-output-stream-string output))"#;
    assert_eq!(evaluate_with(eval_fn, source).to_string(), r#""😀bcd\n""#);
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn, "evaluator")]
#[case::compiled(Runtime::eval_compiled_source as EvalFn, "compiled")]
fn file_position_tracks_character_and_binary_streams(
    #[case] eval_fn: EvalFn,
    #[case] engine: &str,
) {
    let path =
        std::env::temp_dir().join(format!("ncl-file-position-{}-{engine}", std::process::id()));
    let pathname = format!("{:?}", path.to_string_lossy().to_string());
    let source = format!(
        r#"(list
             (let ((input (make-string-input-stream "abc")))
               (list (file-position input)
                     (read-char input)
                     (file-position input)
                     (file-position input :end)
                     (file-position input)
                     (file-position input :start)
                     (file-position input)))
             (let ((output (open {pathname}
                                 :direction :output
                                 :element-type '(unsigned-byte 8)
                                 :if-exists :supersede)))
               (unwind-protect
                   (list (file-position output)
                         (write-byte 7 output)
                         (file-position output)
                         (file-position output :start)
                         (file-position output))
                 (close output))))"#,
    );

    assert_eq!(
        evaluate_with(eval_fn, &source).to_string(),
        "((0 #\\a 1 T 3 T 0) (0 7 1 T 0))"
    );
    assert_eq!(
        fs::read(&path).expect("binary file must remain readable"),
        vec![7]
    );
    let _ = fs::remove_file(path);
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn file_position_rejects_invalid_and_closed_stream_positions(#[case] eval_fn: EvalFn) {
    let source = r#"(list
                     (handler-case
                         (file-position (make-string-input-stream "abc") -1)
                       (type-error () :negative-type-error)
                       (error () :negative-error))
                     (handler-case
                         (file-position
                           (make-string-input-stream "abc")
                           100000000000000000000000000000000000000000000000000000000000)
                       (type-error () :huge-type-error)
                       (error () :huge-error))
                     (let ((stream (make-string-input-stream "abc")))
                       (close stream)
                       (handler-case (file-position stream)
                         (error () :closed-query-error)))
                     (let ((stream (make-string-input-stream "abc")))
                       (close stream)
                       (handler-case (file-position stream :start)
                         (error () :closed-set-error)))
                     (file-position (make-string-input-stream "abc") 99))"#;
    assert_eq!(
        evaluate_with(eval_fn, source).to_string(),
        "(:NEGATIVE-TYPE-ERROR :HUGE-TYPE-ERROR :CLOSED-QUERY-ERROR :CLOSED-SET-ERROR NIL)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn, "evaluator")]
#[case::compiled(Runtime::eval_compiled_source as EvalFn, "compiled")]
fn stream_metadata_reports_types_formats_open_state_and_file_length(
    #[case] eval_fn: EvalFn,
    #[case] engine: &str,
) {
    let path = std::env::temp_dir().join(format!(
        "ncl-stream-metadata-{}-{engine}",
        std::process::id()
    ));
    let pathname = format!("{:?}", path.to_string_lossy().to_string());
    fs::write(&path, b"abc").expect("metadata fixture must be writable");
    let source = format!(
        r#"(let ((input (make-string-input-stream "abc"))
                  (binary (open {pathname} :element-type '(unsigned-byte 8))))
             (unwind-protect
                 (list (open-stream-p input)
                       (stream-element-type input)
                       (stream-external-format input)
                       (file-length binary)
                       (stream-element-type binary)
                       (stream-external-format binary)
                       (handler-case (file-length input)
                         (type-error () :type-error))
                       (progn (close input) (open-stream-p input)))
               (close binary)))"#,
    );

    assert_eq!(
        evaluate_with(eval_fn, &source).to_string(),
        "(T CHARACTER NIL 3 (UNSIGNED-BYTE 8) :DEFAULT :TYPE-ERROR NIL)"
    );
    let _ = fs::remove_file(path);
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn, "evaluator")]
#[case::compiled(Runtime::eval_compiled_source as EvalFn, "compiled")]
fn output_controls_flush_and_clear_pending_file_output(
    #[case] eval_fn: EvalFn,
    #[case] engine: &str,
) {
    let path = std::env::temp_dir().join(format!(
        "ncl-output-controls-{}-{engine}",
        std::process::id()
    ));
    let pathname = format!("{:?}", path.to_string_lossy().to_string());
    let source = format!(
        r#"(let ((output (open {pathname}
                              :direction :output
                              :if-exists :supersede)))
             (write-string "new" output)
             (force-output output)
             (write-string "!" output)
             (clear-output output)
             (finish-output output)
             (close output)
             :done)"#,
    );

    assert_eq!(evaluate_with(eval_fn, &source).to_string(), ":DONE");
    assert_eq!(
        fs::read_to_string(&path).expect("flushed file must remain readable"),
        "new"
    );
    let _ = fs::remove_file(path);
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn, "evaluator")]
#[case::compiled(Runtime::eval_compiled_source as EvalFn, "compiled")]
fn with_open_file_aborts_file_output_after_non_local_exit(
    #[case] eval_fn: EvalFn,
    #[case] engine: &str,
) {
    let path = std::env::temp_dir().join(format!(
        "ncl-with-open-file-abort-{}-{engine}",
        std::process::id()
    ));
    let pathname = format!("{:?}", path.to_string_lossy().to_string());
    fs::write(&path, "keep").expect("initial file write must succeed");
    let source = format!(
        r#"(catch 'escaped
             (with-open-file (stream {pathname}
                              :direction :output
                              :if-exists :overwrite)
               (write-string "new" stream)
               (throw 'escaped :escaped)))"#,
    );

    assert_eq!(evaluate_with(eval_fn, &source).to_string(), ":ESCAPED");
    assert_eq!(
        fs::read_to_string(&path).expect("file must remain readable"),
        "keep"
    );
    let _ = fs::remove_file(path);
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn, "evaluator")]
#[case::compiled(Runtime::eval_compiled_source as EvalFn, "compiled")]
fn file_output_modes_preserve_overwrite_position_and_supersede_contents(
    #[case] eval_fn: EvalFn,
    #[case] engine: &str,
) {
    let path = std::env::temp_dir().join(format!(
        "ncl-file-output-modes-{}-{engine}",
        std::process::id()
    ));
    let pathname = format!("{:?}", path.to_string_lossy().to_string());
    fs::write(&path, "abc").expect("initial file write must succeed");
    let overwrite = format!(
        r#"(with-open-file (stream {pathname}
                            :direction :output
                            :if-exists :overwrite)
             (write-string "Z" stream))"#,
    );
    assert_eq!(evaluate_with(eval_fn, &overwrite).to_string(), "\"Z\"");
    assert_eq!(
        fs::read_to_string(&path).expect("file must remain readable"),
        "Zbc"
    );

    fs::write(&path, "abc").expect("reset file write must succeed");
    let supersede = format!(
        r#"(with-open-file (stream {pathname}
                            :direction :output
                            :if-exists :supersede)
             (write-string "Z" stream))"#,
    );
    assert_eq!(evaluate_with(eval_fn, &supersede).to_string(), "\"Z\"");
    assert_eq!(
        fs::read_to_string(&path).expect("file must remain readable"),
        "Z"
    );
    let _ = fs::remove_file(path);
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn, "evaluator")]
#[case::compiled(Runtime::eval_compiled_source as EvalFn, "compiled")]
fn binary_file_streams_support_write_append_and_read_byte(
    #[case] eval_fn: EvalFn,
    #[case] engine: &str,
) {
    let path = std::env::temp_dir().join(format!(
        "ncl-binary-file-streams-{}-{engine}",
        std::process::id()
    ));
    let pathname = format!("{:?}", path.to_string_lossy().to_string());
    let source = format!(
        r#"(let ((output (open {pathname}
                              :direction :output
                              :element-type '(unsigned-byte 8)
                              :if-exists :supersede)))
             (write-byte 65 output)
             (write-byte 0 output)
             (write-byte 255 output)
             (close output)
             (let ((io (open {pathname}
                              :direction :io
                              :element-type '(unsigned-byte 8)
                              :if-exists :append)))
               (write-byte 7 io)
               (close io))
             (let ((input (open {pathname}
                                :element-type '(unsigned-byte 8))))
               (unwind-protect
                   (list (input-stream-p input)
                         (output-stream-p input)
                         (read-byte input)
                         (read-byte input)
                         (read-byte input)
                         (read-byte input)
                         (read-byte input nil :eof))
                 (close input))))"#,
    );

    assert_eq!(
        evaluate_with(eval_fn, &source).to_string(),
        "(T NIL 65 0 255 7 :EOF)"
    );
    assert_eq!(
        fs::read(&path).expect("binary file must remain readable"),
        vec![65, 0, 255, 7]
    );
    let _ = fs::remove_file(path);
}
