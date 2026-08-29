#[test]
fn compiled_evaluates_dotimes_and_dolist() {
    assert_eq!(
        evaluate(
            "(let ((total 0))
               (dotimes (index 4 total)
                 (setq total (+ total index))))",
        )
        .to_string(),
        "6"
    );
    assert_eq!(
        evaluate(
            "(let ((total 0))
               (dolist (item '(1 2 3) (list total item))
                 (setq total (+ total item))))",
        )
        .to_string(),
        "(6 NIL)"
    );
    assert_eq!(
        evaluate(
            "(progn
               (defmacro twice (x) `(+ ,x ,x))
               (dotimes (index 2 (twice index))
                 (twice index)))",
        )
        .to_string(),
        "4"
    );
}

#[test]
fn compiled_evaluates_prog1_and_prog2_in_order() {
    assert_eq!(
        evaluate(
            "(let ((events 0))
               (list (prog1 (setq events 1) (setq events 2)) events))",
        )
        .to_string(),
        "(1 2)"
    );
    assert_eq!(
        evaluate(
            "(let ((events 0))
               (list (prog2 (setq events 1) (setq events 2) (setq events 3)) events))",
        )
        .to_string(),
        "(2 3)"
    );
    assert_eq!(
        evaluate(
            "(let ((__ncl_prog1_value_0 9))
               (prog1 1 __ncl_prog1_value_0))",
        )
        .to_string(),
        "1"
    );
}

#[test]
fn compiled_packages_resolve_common_lisp_and_exported_symbols() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_compiled_source(
            "(defpackage :compiled-demo (:use :common-lisp) (:export :answer))
             (in-package :compiled-demo)
             (define answer 41)
             (+ answer 1)",
        )
        .must_exist();

    assert_eq!(values[3].to_string(), "42");
    assert_eq!(runtime.current_package(), "COMPILED-DEMO");

    let values = runtime
        .eval_compiled_source("(in-package :ncl-user) compiled-demo:answer")
        .must_exist();
    assert_eq!(values[1].to_string(), "41");
}

#[test]
fn compiled_packages_distinguish_external_and_internal_symbols() {
    let runtime = Runtime::new();
    let error = runtime
        .eval_compiled_source(
            "(defpackage :compiled-hidden)
             (in-package :compiled-hidden)
             (define secret 7)
             (in-package :ncl-user)
             compiled-hidden:secret",
        )
        .must_fail();

    assert!(matches!(error, RuntimeError::Package { .. }));
    assert_eq!(
        runtime
            .eval_compiled_source("compiled-hidden::secret")
            .must_exist()
            .pop()
            .must_exist()
            .to_string(),
        "7"
    );
}

#[test]
fn compiled_packages_inherit_exported_symbols_across_package_switches() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_compiled_source(
            "(defpackage :compiled-provider (:use :common-lisp)
                (:export :answer :plus-one))
             (in-package :compiled-provider)
             (define answer 41)
             (defun plus-one (value) (+ value 1))
             (defpackage :compiled-consumer
                (:use :common-lisp :compiled-provider))
             (in-package :compiled-consumer)
             (list answer (plus-one 1))",
        )
        .must_exist();

    assert_eq!(values.last().must_exist().to_string(), "(41 2)");
    assert_eq!(
        runtime
            .eval_compiled_source("(define answer 99) (list answer (plus-one 1))")
            .must_exist()
            .last()
            .must_exist()
            .to_string(),
        "(99 2)"
    );
}

#[test]
fn compiled_interns_and_finds_package_symbols() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_compiled_source(
            r#"(defpackage :symbols)
               (multiple-value-bind (symbol status) (intern "foo" :symbols)
                 (multiple-value-bind (found found-status) (find-symbol "foo" :symbols)
                   (list (eq symbol found) status found-status
                         (symbol-name found) (symbol-package found))))
               (multiple-value-bind (symbol status) (intern "foo" :keyword)
                 (list symbol status (symbol-name symbol) (symbol-package symbol)))
               (multiple-value-bind (missing status) (find-symbol "missing" :symbols)
                 (list missing status))"#,
        )
        .must_exist();

    assert_eq!(
        values[1].to_string(),
        "(T :INTERNAL :INTERNAL \"FOO\" SYMBOLS)"
    );
    assert_eq!(values[2].to_string(), "(:FOO :EXTERNAL \"FOO\" KEYWORD)");
    assert_eq!(values[3].to_string(), "(NIL NIL)");
}

#[test]
fn compiled_package_objects_support_standard_introspection() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_compiled_source(
            r#"(defpackage :package-inspect-compiled (:use :common-lisp))
               (let ((package (find-package :package-inspect-compiled)))
                 (list (packagep package)
                       (typep package 'package)
                       (package-name package)
                       (eq package (find-package "package-inspect-compiled"))
                       (find-package "missing")
                       (package-name (car (package-use-list package)))
                       (not (null (list-all-packages)))))"#,
        )
        .must_exist();

    assert_eq!(
        values.last().must_exist().to_string(),
        r#"(T T "PACKAGE-INSPECT-COMPILED" T NIL "COMMON-LISP" T)"#
    );
}

#[test]
fn compiled_package_operations_update_use_lists_and_exports() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_compiled_source(
            r#"(defpackage :package-provider-compiled-ops (:use :common-lisp))
               (in-package :package-provider-compiled-ops)
               (define answer 42)
               (export '(:answer))
               (defpackage :package-consumer-compiled-ops (:use :common-lisp))
               (use-package '(:package-provider-compiled-ops)
                            :package-consumer-compiled-ops)
               (in-package :package-consumer-compiled-ops)
               (let ((used answer))
                 (unuse-package '(:package-provider-compiled-ops)
                                :package-consumer-compiled-ops)
                 (unexport '(:answer) :package-provider-compiled-ops)
                 (export '(:answer) :package-consumer-compiled-ops)
                 (unexport '(:answer) :package-consumer-compiled-ops)
                 (list used
                       (package-name
                         (car (package-use-list
                                (find-package :package-consumer-compiled-ops))))
                       (multiple-value-bind (provider-symbol provider-status)
                           (find-symbol "ANSWER" :package-provider-compiled-ops)
                         (list (symbol-name provider-symbol) provider-status))
                       (multiple-value-bind (consumer-symbol consumer-status)
                           (find-symbol "ANSWER" :package-consumer-compiled-ops)
                         (list (symbol-name consumer-symbol) consumer-status))))"#,
        )
        .must_exist();

    assert_eq!(
        values.last().must_exist().to_string(),
        r#"(42 "COMMON-LISP" ("ANSWER" :INTERNAL) ("ANSWER" :INTERNAL))"#
    );
}

#[test]
fn compiled_package_import_shadowing_and_unintern_update_resolution() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_compiled_source(
            r#"(defpackage :package-provider-import-compiled (:use :common-lisp))
               (in-package :package-provider-import-compiled)
               (define answer 42)
               (export '(:answer))
               (defpackage :package-consumer-import-compiled (:use :common-lisp))
               (import '(package-provider-import-compiled::answer)
                       :package-consumer-import-compiled)
               (in-package :package-consumer-import-compiled)
               (define imported
                 (list answer
                       (eq (find-symbol "ANSWER"
                                        :package-consumer-import-compiled)
                           'package-provider-import-compiled::answer)))
               (shadowing-import '(package-provider-import-compiled::answer)
                                 :package-consumer-import-compiled)
               (define shadowing-imported
                 (list answer
                       (eq (find-symbol "ANSWER"
                                        :package-consumer-import-compiled)
                           'package-provider-import-compiled::answer)))
               (shadow '(:answer) :package-consumer-import-compiled)
               (define answer 7)
               (let ((shadowed answer))
                 (let ((removed
                         (unintern '(:answer)
                                   :package-consumer-import-compiled)))
                   (list imported shadowing-imported shadowed removed
                         (boundp 'answer)
                         (multiple-value-bind (symbol status)
                             (find-symbol "ANSWER"
                                          :package-consumer-import-compiled)
                           (list symbol status)))))"#,
        )
        .must_exist();

    assert_eq!(
        values.last().must_exist().to_string(),
        r"((42 T) (42 T) 7 T NIL (NIL NIL))"
    );
}

#[test]
fn compiled_defpackage_nicknames_resolve_to_the_same_package() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_compiled_source(
            r#"(defpackage :package-nickname-owner-compiled
                 (:nicknames :package-nickname-alias-compiled)
                 (:use :common-lisp)
                 (:export :answer))
               (in-package :package-nickname-alias-compiled)
               (define answer 41)
               (in-package :ncl-user)
               (list (string= (package-name
                                 (find-package :package-nickname-alias-compiled))
                              "PACKAGE-NICKNAME-OWNER-COMPILED")
                     (eq (find-package :package-nickname-alias-compiled)
                         (find-package :package-nickname-owner-compiled))
                     (eq (find-symbol "ANSWER" :package-nickname-alias-compiled)
                         (find-symbol "ANSWER" :package-nickname-owner-compiled))
                     package-nickname-alias-compiled:answer
                     package-nickname-owner-compiled:answer)"#,
        )
        .must_exist();

    assert_eq!(values.last().must_exist().to_string(), r"(T T T 41 41)");
}

#[test]
fn compiled_defpackage_nicknames_work_for_use_and_import() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_compiled_source(
            r#"(defpackage :package-nickname-source-compiled
                 (:nicknames :package-nickname-source-alias-compiled)
                 (:use :common-lisp)
                 (:export :answer))
               (in-package :package-nickname-source-compiled)
               (define answer 42)
               (defpackage :package-nickname-use-compiled
                 (:use :common-lisp :package-nickname-source-alias-compiled))
               (in-package :package-nickname-use-compiled)
               (define via-use answer)
               (defpackage :package-nickname-import-compiled
                 (:use :common-lisp)
                 (:import-from :package-nickname-source-alias-compiled :answer))
               (defpackage :package-nickname-runtime-import-compiled
                 (:use :common-lisp))
               (import '(package-nickname-source-alias-compiled:answer)
                       :package-nickname-runtime-import-compiled)
               (in-package :package-nickname-import-compiled)
               (define via-defpackage-import answer)
               (in-package :package-nickname-runtime-import-compiled)
               (define via-runtime-import answer)
               (in-package :ncl-user)
               (list package-nickname-use-compiled::via-use
                     package-nickname-import-compiled::via-defpackage-import
                     package-nickname-runtime-import-compiled::via-runtime-import
                     (eq (find-symbol "ANSWER"
                                      :package-nickname-runtime-import-compiled)
                         'package-nickname-source-compiled:answer))"#,
        )
        .must_exist();

    assert_eq!(values.last().must_exist().to_string(), r"(42 42 42 T)");
}

#[test]
fn compiled_defpackage_symbol_options_update_package_state() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_compiled_source(
            r#"(defpackage :package-provider-defpackage-compiled
                 (:use :common-lisp)
                 (:export :answer :shadowed))
               (in-package :package-provider-defpackage-compiled)
               (define answer 42)
               (define shadowed 43)
               (defpackage :package-consumer-defpackage-compiled
                 (:use :common-lisp)
                 (:shadow :local-shadow)
                 (:intern :local)
                 (:import-from :package-provider-defpackage-compiled :answer)
                 (:shadowing-import-from :package-provider-defpackage-compiled :shadowed))
               (in-package :package-consumer-defpackage-compiled)
               (define local-shadow 7)
               (define local 8)
               (list answer
                     shadowed
                     local-shadow
                     local
                     (eq (find-symbol "ANSWER"
                                      :package-consumer-defpackage-compiled)
                         'package-provider-defpackage-compiled::answer)
                     (eq (find-symbol "SHADOWED"
                                      :package-consumer-defpackage-compiled)
                         'package-provider-defpackage-compiled::shadowed)
                     (multiple-value-bind (symbol status)
                         (find-symbol "LOCAL"
                                      :package-consumer-defpackage-compiled)
                       (list (symbol-name symbol) status))
                     (multiple-value-bind (symbol status)
                         (find-symbol "LOCAL-SHADOW"
                                      :package-consumer-defpackage-compiled)
                       (list (symbol-name symbol) status)))"#,
        )
        .must_exist();

    assert_eq!(
        values.last().must_exist().to_string(),
        r#"(42 43 7 8 T T ("LOCAL" :INTERNAL) ("LOCAL-SHADOW" :INTERNAL))"#
    );
}

#[test]
fn compiled_defpackage_local_nicknames_and_documentation_work() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_compiled_source(
            r#"(defpackage :local-target-compiled
                 (:use :common-lisp)
                 (:export :answer))
               (in-package :local-target-compiled)
               (define answer 42)
               (defpackage :local-owner-compiled
                 (:use :common-lisp)
                 (:local-nicknames (:target :local-target-compiled))
                 (:documentation "local owner documentation"))
               (in-package :local-owner-compiled)
               (list target:answer
                     (documentation (find-package :local-owner-compiled) t)
                     (find-package :target))"#,
        )
        .must_exist();

    assert_eq!(
        values.last().must_exist().to_string(),
        r#"(42 "local owner documentation" NIL)"#
    );
}

#[test]
fn compiled_defpackage_size_option_is_accepted_and_validated() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_compiled_source(
            r"(defpackage :package-size-compiled
                 (:use :common-lisp)
                 (:size 0))
               (package-name (find-package :package-size-compiled))",
        )
        .must_exist();

    assert_eq!(
        values.last().must_exist().to_string(),
        "\"PACKAGE-SIZE-COMPILED\""
    );

    let error = runtime
        .eval_compiled_source("(defpackage :package-size-invalid-compiled (:size -1))")
        .must_fail();
    assert!(error.to_string().contains("defpackage :size"));
}

#[test]
fn compiled_string_streams_read_and_write() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_compiled_source(
            r#"(let ((input (make-string-input-stream "abc
rest"))
                   (output (make-string-output-stream)))
               (list (streamp input)
                     (input-stream-p input)
                     (output-stream-p output)
                     (typep output 'stream)
                     (peek-char input)
                     (read-char input)
                     (read-char input)
                     (unread-char #\b input)
                     (read-char input)
                     (read-line input)
                     (format output "~A~C" "ok" #\!)
                     (get-output-stream-string output)))"#,
        )
        .must_exist();

    assert_eq!(
        values.last().must_exist().to_string(),
        r#"(T T T T #\a #\a #\b NIL #\b "c" NIL "ok!")"#
    );
}

#[test]
fn compiled_string_streams_line_output_operations() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_compiled_source(
            r#"(let ((output (make-string-output-stream)))
               (list (write-string "head" output)
                     (fresh-line output)
                     (fresh-line output)
                     (terpri output)
                     (write-line "tail" output)
                     (get-output-stream-string output)))"#,
        )
        .must_exist();

    assert_eq!(
        values.last().must_exist().to_string(),
        r#"("head" T NIL NIL "tail" "head\n\ntail\n")"#
    );
}

#[test]
fn compiled_file_streams_round_trip_through_with_open_file() {
    let path = std::env::temp_dir().join(format!(
        "ncl-with-open-file-compiled-{}",
        std::process::id()
    ));
    let pathname = format!("{:?}", path.to_string_lossy().to_string());
    let source = format!(
        r#"(progn
               (with-open-file (stream {pathname}
                                :direction :output
                                :if-exists :supersede)
                 (write-string "hello" stream))
               (with-open-file (stream {pathname})
                 (char= (read-char stream) #\h)))"#,
    );

    assert_eq!(evaluate(&source).to_string(), "T");
    assert_eq!(std::fs::read_to_string(&path).must_exist(), "hello");
    let _ = std::fs::remove_file(path);
}

#[test]
fn compiled_file_stream_options_cover_probe_append_and_abort() {
    let path = std::env::temp_dir().join(format!(
        "ncl-file-stream-options-compiled-{}",
        std::process::id()
    ));
    let missing_path = std::env::temp_dir().join(format!(
        "ncl-file-stream-options-compiled-missing-{}",
        std::process::id()
    ));
    let pathname = format!("{:?}", path.to_string_lossy().to_string());
    let missing_pathname = format!("{:?}", missing_path.to_string_lossy().to_string());
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&missing_path);
    let source = format!(
        r#"(progn
               (with-open-file (stream {pathname}
                                :direction :output
                                :if-exists :supersede)
                 (write-string "a" stream))
               (with-open-file (stream {pathname}
                                :direction :output
                                :if-exists :append)
                 (write-string "b" stream))
               (let ((existing (open {pathname} :direction :probe))
                     (missing (open {missing_pathname} :direction :probe)))
                 (prog1 (list (streamp existing) (null missing))
                   (close existing)))
               (let ((stream (open {missing_pathname}
                                   :direction :output
                                   :if-does-not-exist :create)))
                 (write-string "discard" stream)
                 (close stream :abort t))
               (null (open {missing_pathname} :direction :probe)))"#,
    );

    assert_eq!(evaluate(&source).to_string(), "T");
    assert_eq!(std::fs::read_to_string(&path).must_exist(), "ab");
    assert!(!missing_path.exists());
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(missing_path);
}

#[test]
fn compiled_file_io_stream_reads_writes_and_appends() {
    let path = std::env::temp_dir().join(format!(
        "ncl-file-io-stream-compiled-{}",
        std::process::id()
    ));
    let pathname = format!("{:?}", path.to_string_lossy().to_string());
    std::fs::write(&path, "abc").must_exist();
    let source = format!(
        r#"(let ((stream (open {pathname}
                            :direction :io
                            :if-exists :overwrite)))
               (list (input-stream-p stream)
                     (output-stream-p stream)
                     (progn
                       (read-char stream)
                       (write-string "Z" stream)
                       (close stream)
                       t)
                     (progn
                       (let ((append-stream (open {pathname}
                                                  :direction :io
                                                  :if-exists :append)))
                         (write-string "!" append-stream)
                         (close append-stream))
                       t)
                     (with-open-file (input {pathname})
                       (string= (read-line input) "aZc!"))))"#,
    );

    assert_eq!(evaluate(&source).to_string(), "(T T T T T)");
    assert_eq!(std::fs::read_to_string(&path).must_exist(), "aZc!");
    let _ = std::fs::remove_file(path);
}

#[test]
fn compiled_file_pathname_primitives_probe_rename_delete_and_date() {
    let source_path = std::env::temp_dir().join(format!(
        "ncl-file-pathname-primitives-source-compiled-{}",
        std::process::id()
    ));
    let renamed_path = std::env::temp_dir().join(format!(
        "ncl-file-pathname-primitives-renamed-compiled-{}",
        std::process::id()
    ));
    let source = format!("{:?}", source_path.to_string_lossy().to_string());
    let renamed = format!("{:?}", renamed_path.to_string_lossy().to_string());
    let _ = std::fs::remove_file(&source_path);
    let _ = std::fs::remove_file(&renamed_path);
    std::fs::write(&source_path, "content").must_exist();
    let form = format!(
        r"(let ((original (probe-file {source})))
             (multiple-value-bind (new old-truename new-truename)
                 (rename-file {source} {renamed})
               (list (stringp original)
                     (stringp (truename {renamed}))
                     (stringp new)
                     (stringp old-truename)
                     (stringp new-truename)
                     (integerp (file-write-date {renamed}))
                     (null (probe-file {source}))
                     (stringp (probe-file {renamed}))
                     (delete-file {renamed})
                     (null (probe-file {renamed})))))",
    );

    assert_eq!(evaluate(&form).to_string(), "(T T T T T T T T T T)");
    let _ = std::fs::remove_file(source_path);
    let _ = std::fs::remove_file(renamed_path);
}

#[test]
fn compiled_evaluates_rational_literals_and_exact_arithmetic() {
    assert_eq!(
        evaluate(
            "(list 1/2 2/4 (+ 1/2 1/3) (- 3/2 1/2) (* 2/3 9/4) (/ 2/3 4/5) (+ 1 1/2) (= 1 2/2) (< 1/3 1/2) (rationalp 1/2) (rationalp 1) (typep 1/2 'ratio) (typep 1/2 'rational) (numberp 1/2) (floatp 1/2))"
        )
        .to_string(),
          "(1/2 1/2 5/6 1 3/2 5/6 3/2 T T T T T T T NIL)"
    );
}

#[test]
fn compiled_evaluates_short_circuit_and_conditional_dispatch_forms() {
    assert_eq!(
        evaluate(
            "(list
               (and)
               (and 1 2 3)
               (and nil (error \"unreachable\"))
               (or)
               (or nil nil 7)
               (or 9 (error \"unreachable\"))
               (when t 1 2 3)
               (unless nil 4 5)
               (cond (nil 1) ((= 2 2) 6) (t 7))
               (cond (nil 1))
               (case 2 ((1) 10) ((2 3) 20) (otherwise 30))
               (case 9 ((1) 10))
               (typecase 2 (string 10) (integer 20) (otherwise 30))
               (typecase 2 (string 10)))"
        )
        .to_string(),
        "(T 3 NIL NIL 7 9 3 5 6 NIL 20 NIL 20 NIL)"
    );
}

#[test]
fn compiled_tagbody_and_go_with_forward_and_backward_jumps() {
    let source = r"
        (let ((count 0))
          (tagbody
            start
            (setq count (+ count 1))
            (if (= count 3) (go done) (go start))
            done)
          count)
    ";

    assert_eq!(evaluate(source).to_string(), "3");
}

#[test]
fn compiled_unmatched_go_is_not_swallowed_by_ignore_errors() {
    let error = Runtime::new()
        .eval_compiled_source("(ignore-errors (go missing))")
        .must_fail();

    assert!(matches!(
        error,
        ncl_runtime::RuntimeError::Go {
            tag,
            target: None,
            ..
        } if tag == "MISSING"
    ));
}

#[test]
fn compiled_rejects_invalid_go_shapes_and_tags() {
    for source in ["(go)", "(go missing extra)", "(go 1.5)"] {
        assert!(
            Runtime::new().eval_compiled_source(source).is_err(),
            "{source}"
        );
    }
    assert!(
        Runtime::new()
            .eval_compiled_source("(tagbody start start)")
            .is_err()
    );
}

#[test]
fn compiled_rejects_malformed_special_forms_at_their_boundaries() {
    for source in support::MALFORMED_SPECIAL_FORMS {
        assert!(
            Runtime::new().eval_compiled_source(source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn compiled_executes_dynamic_bindings_and_multiple_value_calls() {
    assert_eq!(
        evaluate(
            "(progn
               (defvar *compiled-progv* 1)
               (list
                 (progv '(*compiled-progv* fresh-variable) '(2 3)
                   (list *compiled-progv* fresh-variable))
                 *compiled-progv*
                 (multiple-value-call #'+ (values 20 22))))",
        )
        .to_string(),
        "((2 3) 1 42)"
    );
}

#[test]
fn compiled_progv_fills_missing_values_and_rejects_non_symbols() {
    assert_eq!(
        evaluate("(progv '(first second) '(10) (list first second))").to_string(),
        "(10 NIL)"
    );
    let error = Runtime::new()
        .eval_compiled_source("(progv '(first 2) '(10 20) first)")
        .must_fail();
    assert!(
        matches!(error, RuntimeError::InvalidForm { message, .. } if message.contains("progv symbol list"))
    );
}

#[test]
fn compiled_rejects_non_list_progv_arguments() {
    let cases = [
        ("(progv 1 '(10) nil)", "LIST"),
        ("(progv '(first) 1 nil)", "LIST"),
    ];

    for (source, expected_type) in cases {
        let error = Runtime::new().eval_compiled_source(source).must_fail();
        assert!(
            matches!(error, RuntimeError::Type { ref expected, .. } if expected == expected_type),
            "{source}: {error:?}"
        );
    }
}

#[test]
fn compiled_executes_restart_case_and_handler_case_paths() {
    assert_eq!(
        evaluate(
            "(handler-case
               (restart-case
                 (invoke-restart 'use-values 20 22)
                 (use-values (left right) (+ left right)))
               (error (condition) condition))",
        )
        .to_string(),
        "42"
    );
    assert_eq!(
        evaluate(
            "(handler-case
               (/ 1 0)
               (division-by-zero (condition) 9))",
        )
        .to_string(),
        "9"
    );
}
use super::*;
