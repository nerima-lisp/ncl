use super::*;

#[test]
fn evaluates_setf_places() {
    assert_eq!(
        evaluate("(let ((xs (list 1 2 3))) (setf (car xs) 9 (nth 2 xs) 7) xs)").to_string(),
        "(9 2 7)"
    );
    assert_eq!(
        evaluate("(let ((values #(1 2))) (setf (aref values 1) 8) values)").to_string(),
        "#(1 8)"
    );
    assert_eq!(
        evaluate("(let ((text \"abc\")) (setf (char text 1) #\\X) text)").to_string(),
        "\"aXc\""
    );
    assert_eq!(
        evaluate("(let ((text \"abc\")) (setf (schar text 1) #\\Y) text)").to_string(),
        "\"aYc\""
    );
    assert_eq!(
        evaluate("(let ((values #(1 2 3))) (setf (svref values 1) 8) values)").to_string(),
        "#(1 8 3)"
    );
    assert_eq!(
        evaluate(
            "(let ((array (make-array '(2 2) :initial-element 0)))
               (setf (row-major-aref array 2) 9)
               (row-major-aref array 2))",
        )
        .to_string(),
        "9"
    );
    assert_eq!(
        evaluate(
            "(let ((array (make-array '(2 3) :initial-element 0)))
               (setf (aref array 1 0) 9)
               (list (aref array 1 0) (row-major-aref array 3)))",
        )
        .to_string(),
        "(9 9)"
    );
    assert_eq!(
        evaluate("(let ((bits #(0 1 0))) (setf (bit bits 1) 0) (bit bits 1))").to_string(),
        "0"
    );
    assert_eq!(
        evaluate("(let ((xs (list (list 1 2)))) (setf (car (nth 0 xs)) 9) xs)").to_string(),
        "((9 2))"
    );
    assert_eq!(
        evaluate("(let ((text \"abc\")) (setf (elt text 1) #\\X) text)").to_string(),
        "\"aXc\""
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list 1 2 3 4 5)))
               (setf (subseq xs 1 4) #(9 8 7))
               xs)",
        )
        .to_string(),
        "(1 9 8 7 5)"
    );
    assert_eq!(
        evaluate(
            "(let ((text \"abcde\"))
               (setf (subseq text 1 4) '(#\\X #\\Y #\\Z))
               text)",
        )
        .to_string(),
        "\"aXYZe\""
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list 1 2 3 4)))
               (setf (subseq xs 1 3) '(9))
               xs)",
        )
        .to_string(),
        "(1 9 3 4)"
    );
    assert_eq!(
        evaluate("(let ((plist (list :a 1))) (setf (getf plist :a) 2) plist)").to_string(),
        "(:A 2)"
    );
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *setf-symbol-value-target* 1)
               (list
                 (setf (symbol-value '*setf-symbol-value-target*) 7)
                 (symbol-value '*setf-symbol-value-target*)))",
        )
        .to_string(),
        "(7 7)"
    );
}

#[test]
fn evaluates_table_driven_property_and_hash_places() {
    let cases = [
        (
            "(let ((plist (list :a 1))) (setf (getf plist :a) 2) plist)",
            "(:A 2)",
        ),
        (
            "(let ((plist (list :a 1))) (setf (getf plist :b) 2) plist)",
            "(:A 1 :B 2)",
        ),
        (
            "(let ((table (make-hash-table))) (setf (gethash :key table) 42) (gethash :key table))",
            "42",
        ),
        (
            "(let ((table (make-hash-table))) (setf (gethash :key table) 42) (setf (gethash :key table) 43) (gethash :key table))",
            "43",
        ),
    ];

    for (source, expected) in cases {
        assert_eq!(evaluate(source).to_string(), expected);
    }
}

#[test]
fn evaluates_setf_aliases_and_sequence_places_from_shared_cases() {
    assert_value_cases(
        evaluate,
        &[
            (
                "(let ((xs (list 1 2))) (setf (first xs) 9 (rest xs) '(3 4)) xs)",
                "(9 3 4)",
            ),
            (
                "(let ((xs (list 1 2 3))) (setf (cdr xs) '(8 9)) xs)",
                "(1 8 9)",
            ),
            (
                "(let ((xs (list 1 2 3))) (setf (nth 0 xs) 7) xs)",
                "(7 2 3)",
            ),
            (
                "(let ((xs (list 1 2 3))) (setf (second xs) 8 (third xs) 9) xs)",
                "(1 8 9)",
            ),
            (
                "(let ((xs (list 1 2 3))) (setf (elt xs 1) 8) xs)",
                "(1 8 3)",
            ),
            (
                "(let ((values #(1 2 3))) (setf (elt values 1) 8) values)",
                "#(1 8 3)",
            ),
            (
                "(let ((text \"abc\")) (setf (elt text 1) #\\X) text)",
                "\"aXc\"",
            ),
        ],
    );
}

#[test]
fn evaluates_setf_sequence_boundaries_from_table_cases() {
    assert_value_cases(
        evaluate,
        &[
            (
                "(let ((xs #(1 2 3))) (setf (subseq xs 1 3) '(8 9)) xs)",
                "#(1 8 9)",
            ),
            (
                "(let ((xs (make-array '(2 2) :initial-element 0))) (setf (aref xs 1 0) 7) (setf (bit xs 0 1) 1) (list (aref xs 0 0) (aref xs 0 1) (aref xs 1 0) (aref xs 1 1)))",
                "(0 1 7 0)",
            ),
            ("(let ((xs #(0 1))) (setf (bit xs 1) 0) xs)", "#(0 0)"),
        ],
    );
}

#[test]
fn rejects_setf_sequence_boundaries_from_table_cases() {
    for source in [
        "(let ((xs #(1 2))) (setf (elt xs 2) 8))",
        "(let ((xs \"abc\")) (setf (elt xs 0) 8))",
        "(let ((xs (make-array '(2 2) :initial-element 0))) (setf (aref xs 2 0) 7))",
        "(let ((xs #(0 1))) (setf (bit xs 0) 2))",
        "(let ((xs (list 1 2))) (setf (subseq xs 2 1) '(9)))",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_simple_defsetf() {
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *defsetf-cell* 1)
               (defun defsetf-reader () *defsetf-cell*)
               (defun defsetf-writer (value) (setq *defsetf-cell* value))
               (defsetf defsetf-reader defsetf-writer)
               (setf (defsetf-reader) 42)
               (defsetf-reader))",
        )
        .to_string(),
        "42"
    );
}

#[test]
fn evaluates_defsetf_passes_place_arguments_before_value() {
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *defsetf-arguments* nil)
               (defun defsetf-argument-reader (first second) nil)
               (defun defsetf-argument-writer (&rest arguments)
                 (setq *defsetf-arguments* arguments))
               (defsetf defsetf-argument-reader defsetf-argument-writer)
               (setf (defsetf-argument-reader :first :second) :new)
               *defsetf-arguments*)",
        )
        .to_string(),
        "(:FIRST :SECOND :NEW)"
    );
}

#[test]
fn evaluates_define_setf_expander_and_get_setf_expansion() {
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *custom-setf-cell* 1)
               (define-setf-expander custom-setf-place ()
                 (values nil nil '(new-value)
                         '(progn
                            (setq *custom-setf-cell* new-value)
                            new-value)
                         '*custom-setf-cell*))
               (setf (custom-setf-place) 42)
               (multiple-value-bind (temporaries value-forms stores store-form access-form)
                   (get-setf-expansion '(custom-setf-place))
                 (list *custom-setf-cell*
                       (length temporaries)
                       (length value-forms)
                       (length stores)
                       (car stores)
                       store-form
                       access-form)))",
        )
        .to_string(),
        "(42 0 0 1 NEW-VALUE (PROGN (SETQ *CUSTOM-SETF-CELL* NEW-VALUE) NEW-VALUE) *CUSTOM-SETF-CELL*)"
    );
}

#[test]
fn rejects_malformed_define_setf_expansions_from_table_cases() {
    let cases = [
        "(values 1 nil '(a) nil nil)",
        "(values nil 1 '(a) nil nil)",
        "(values nil nil 1 nil nil)",
        "(values nil nil nil nil nil)",
        "(values nil nil '(a b) nil nil)",
        "(values '(a) nil '(b) nil nil)",
        "(values nil nil '(a) nil)",
        "(values nil nil '(a) nil nil 1)",
    ];
    for expansion in cases {
        let form = format!(
            "(progn\
               (define-setf-expander malformed-place () {expansion})\
               (handler-case (get-setf-expansion '(malformed-place))\
                 (error () :error)))"
        );
        assert_eq!(evaluate(&form).to_string(), ":ERROR", "{expansion}");
    }
}

#[test]
fn gets_setf_expansions_for_table_driven_standard_places() {
    let cases = [
        ("(get-setf-expansion '(car cell))", "(1 1 1)"),
        ("(get-setf-expansion '(nth 1 cell))", "(2 2 1)"),
        ("(get-setf-expansion 'symbol)", "(0 0 1)"),
    ];
    for (source, expected) in cases {
        let form = format!(
            "(multiple-value-bind (temporaries values stores store-form access-form) {source}
               (list (length temporaries) (length values) (length stores)))"
        );
        assert_eq!(evaluate(&form).to_string(), expected, "{source}");
    }
}

#[test]
fn evaluates_define_modify_macro_on_generalized_place() {
    assert_eq!(
        evaluate(
            "(progn
               (define-modify-macro add-to-place (&optional (delta 1)) +)
               (let ((cell (list 10)))
                 (list (add-to-place (car cell) 2)
                       (add-to-place (car cell))
                       cell)))",
        )
        .to_string(),
        "(12 13 (13))"
    );
    assert_eq!(
        evaluate(
            "(progn
               (define-modify-macro add-to-nested-place (&optional (delta 1)) +)
               (let ((cells (list (list 10))))
                 (list (add-to-nested-place (car (nth 0 cells)) 2)
                       cells)))",
        )
        .to_string(),
        "(12 ((12)))"
    );
}

#[test]
fn generalized_assignment_rejects_malformed_places_and_arguments() {
    for source in support::MALFORMED_GENERALIZED_ASSIGNMENT_FORMS {
        Runtime::eval_source(&Runtime::new(), source).must_fail();
    }
}

#[test]
fn packages_resolve_common_lisp_and_exported_symbols() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            "(defpackage :demo (:use :common-lisp) (:export :answer))
             (in-package :demo)
             (define answer 41)
             (+ answer 1)",
        )
        .must_exist();

    assert_eq!(values[3].to_string(), "42");
    assert_eq!(runtime.current_package(), "DEMO");

    let values = runtime
        .eval_source("(in-package :ncl-user) demo:answer")
        .must_exist();
    assert_eq!(values[1].to_string(), "41");
}

#[test]
fn packages_distinguish_external_and_internal_symbols() {
    let runtime = Runtime::new();
    let error = runtime
        .eval_source(
            "(defpackage :hidden)
             (in-package :hidden)
             (define secret 7)
             (in-package :ncl-user)
             hidden:secret",
        )
        .must_fail();

    assert!(matches!(error, ncl_runtime::RuntimeError::Package { .. }));
    assert_eq!(
        runtime
            .eval_source("hidden::secret")
            .must_exist()
            .pop()
            .must_exist()
            .to_string(),
        "7"
    );
}

#[test]
fn packages_inherit_exported_symbols_across_package_switches() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            "(defpackage :provider (:use :common-lisp) (:export :answer :plus-one))
             (in-package :provider)
             (define answer 41)
             (defun plus-one (value) (+ value 1))
             (defpackage :consumer (:use :common-lisp :provider))
             (in-package :consumer)
             (list answer (plus-one 1))",
        )
        .must_exist();

    assert_eq!(values.last().must_exist().to_string(), "(41 2)");
    assert_eq!(
        runtime
            .eval_source("(define answer 99) (list answer (plus-one 1))")
            .must_exist()
            .last()
            .must_exist()
            .to_string(),
        "(99 2)"
    );
}

#[test]
fn interns_and_finds_package_symbols() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
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
fn package_objects_support_standard_introspection() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(defpackage :package-inspect-eval (:use :common-lisp))
               (let ((package (find-package :package-inspect-eval)))
                 (list (packagep package)
                       (typep package 'package)
                       (package-name package)
                       (eq package (find-package "package-inspect-eval"))
                       (find-package "missing")
                       (package-name (car (package-use-list package)))
                       (not (null (list-all-packages)))))"#,
        )
        .must_exist();

    assert_eq!(
        values.last().must_exist().to_string(),
        r#"(T T "PACKAGE-INSPECT-EVAL" T NIL "COMMON-LISP" T)"#
    );
}

#[test]
fn package_operations_update_use_lists_and_exports() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(defpackage :package-provider-ops (:use :common-lisp))
               (in-package :package-provider-ops)
               (define answer 42)
               (export '(:answer))
               (defpackage :package-consumer-ops (:use :common-lisp))
               (use-package '(:package-provider-ops) :package-consumer-ops)
               (in-package :package-consumer-ops)
               (let ((used answer))
                 (unuse-package '(:package-provider-ops) :package-consumer-ops)
                 (unexport '(:answer) :package-provider-ops)
                 (export '(:answer) :package-consumer-ops)
                 (unexport '(:answer) :package-consumer-ops)
                 (list used
                       (package-name
                         (car (package-use-list (find-package :package-consumer-ops))))
                       (multiple-value-bind (provider-symbol provider-status)
                           (find-symbol "ANSWER" :package-provider-ops)
                         (list (symbol-name provider-symbol) provider-status))
                       (multiple-value-bind (consumer-symbol consumer-status)
                           (find-symbol "ANSWER" :package-consumer-ops)
                         (list (symbol-name consumer-symbol) consumer-status))))"#,
        )
        .must_exist();

    assert_eq!(
        values.last().must_exist().to_string(),
        r#"(42 "COMMON-LISP" ("ANSWER" :INTERNAL) ("ANSWER" :INTERNAL))"#
    );
}

#[test]
fn package_import_shadowing_and_unintern_update_resolution() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(defpackage :package-provider-import-eval (:use :common-lisp))
               (in-package :package-provider-import-eval)
               (define answer 42)
               (export '(:answer))
               (defpackage :package-consumer-import-eval (:use :common-lisp))
               (import '(package-provider-import-eval::answer)
                       :package-consumer-import-eval)
               (in-package :package-consumer-import-eval)
               (define imported
                 (list answer
                       (eq (find-symbol "ANSWER"
                                        :package-consumer-import-eval)
                           'package-provider-import-eval::answer)))
               (shadowing-import '(package-provider-import-eval::answer)
                                 :package-consumer-import-eval)
               (define shadowing-imported
                 (list answer
                       (eq (find-symbol "ANSWER"
                                        :package-consumer-import-eval)
                           'package-provider-import-eval::answer)))
               (shadow '(:answer) :package-consumer-import-eval)
               (define answer 7)
               (let ((shadowed answer))
                 (let ((removed
                         (unintern '(:answer)
                                   :package-consumer-import-eval)))
                   (list imported shadowing-imported shadowed removed
                         (boundp 'answer)
                         (multiple-value-bind (symbol status)
                             (find-symbol "ANSWER"
                                          :package-consumer-import-eval)
                           (list symbol status)))))"#,
        )
        .must_exist();

    assert_eq!(
        values.last().must_exist().to_string(),
        r"((42 T) (42 T) 7 T NIL (NIL NIL))"
    );
}

#[test]
fn defpackage_nicknames_resolve_to_the_same_package() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(defpackage :package-nickname-owner-eval
                 (:nicknames :package-nickname-alias-eval)
                 (:use :common-lisp)
                 (:export :answer))
               (in-package :package-nickname-alias-eval)
               (define answer 41)
               (in-package :ncl-user)
               (list (string= (package-name
                                 (find-package :package-nickname-alias-eval))
                              "PACKAGE-NICKNAME-OWNER-EVAL")
                     (eq (find-package :package-nickname-alias-eval)
                         (find-package :package-nickname-owner-eval))
                     (eq (find-symbol "ANSWER" :package-nickname-alias-eval)
                         (find-symbol "ANSWER" :package-nickname-owner-eval))
                     package-nickname-alias-eval:answer
                     package-nickname-owner-eval:answer)"#,
        )
        .must_exist();

    assert_eq!(values.last().must_exist().to_string(), r"(T T T 41 41)");
}

#[test]
fn defpackage_nicknames_work_for_use_and_import() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(defpackage :package-nickname-source-eval
                 (:nicknames :package-nickname-source-alias-eval)
                 (:use :common-lisp)
                 (:export :answer))
               (in-package :package-nickname-source-eval)
               (define answer 42)
               (defpackage :package-nickname-use-eval
                 (:use :common-lisp :package-nickname-source-alias-eval))
               (in-package :package-nickname-use-eval)
               (define via-use answer)
               (defpackage :package-nickname-import-eval
                 (:use :common-lisp)
                 (:import-from :package-nickname-source-alias-eval :answer))
               (defpackage :package-nickname-runtime-import-eval
                 (:use :common-lisp))
               (import '(package-nickname-source-alias-eval:answer)
                       :package-nickname-runtime-import-eval)
               (in-package :package-nickname-import-eval)
               (define via-defpackage-import answer)
               (in-package :package-nickname-runtime-import-eval)
               (define via-runtime-import answer)
               (in-package :ncl-user)
               (list package-nickname-use-eval::via-use
                     package-nickname-import-eval::via-defpackage-import
                     package-nickname-runtime-import-eval::via-runtime-import
                     (eq (find-symbol "ANSWER"
                                      :package-nickname-runtime-import-eval)
                         'package-nickname-source-eval:answer))"#,
        )
        .must_exist();

    assert_eq!(values.last().must_exist().to_string(), r"(42 42 42 T)");
}

#[test]
fn defpackage_symbol_options_update_package_state() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(defpackage :package-provider-defpackage-eval
                 (:use :common-lisp)
                 (:export :answer :shadowed))
               (in-package :package-provider-defpackage-eval)
               (define answer 42)
               (define shadowed 43)
               (defpackage :package-consumer-defpackage-eval
                 (:use :common-lisp)
                 (:shadow :local-shadow)
                 (:intern :local)
                 (:import-from :package-provider-defpackage-eval :answer)
                 (:shadowing-import-from :package-provider-defpackage-eval :shadowed))
               (in-package :package-consumer-defpackage-eval)
               (define local-shadow 7)
               (define local 8)
               (list answer
                     shadowed
                     local-shadow
                     local
                     (eq (find-symbol "ANSWER"
                                      :package-consumer-defpackage-eval)
                         'package-provider-defpackage-eval::answer)
                     (eq (find-symbol "SHADOWED"
                                      :package-consumer-defpackage-eval)
                         'package-provider-defpackage-eval::shadowed)
                     (multiple-value-bind (symbol status)
                         (find-symbol "LOCAL" :package-consumer-defpackage-eval)
                       (list (symbol-name symbol) status))
                     (multiple-value-bind (symbol status)
                         (find-symbol "LOCAL-SHADOW"
                                      :package-consumer-defpackage-eval)
                       (list (symbol-name symbol) status)))"#,
        )
        .must_exist();

    assert_eq!(
        values.last().must_exist().to_string(),
        r#"(42 43 7 8 T T ("LOCAL" :INTERNAL) ("LOCAL-SHADOW" :INTERNAL))"#
    );
}

#[test]
fn defpackage_local_nicknames_and_documentation_work() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(defpackage :local-target-eval
                 (:use :common-lisp)
                 (:export :answer))
               (in-package :local-target-eval)
               (define answer 42)
               (defpackage :local-owner-eval
                 (:use :common-lisp)
                 (:local-nicknames (:target :local-target-eval))
                 (:documentation "local owner documentation"))
               (in-package :local-owner-eval)
               (list target:answer
                     (documentation (find-package :local-owner-eval) t)
                     (find-package :target))"#,
        )
        .must_exist();

    assert_eq!(
        values.last().must_exist().to_string(),
        r#"(42 "local owner documentation" NIL)"#
    );
}

#[test]
fn defpackage_size_option_is_accepted_and_validated() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r"(defpackage :package-size-eval
                 (:use :common-lisp)
                 (:size 0))
               (package-name (find-package :package-size-eval))",
        )
        .must_exist();

    assert_eq!(
        values.last().must_exist().to_string(),
        "\"PACKAGE-SIZE-EVAL\""
    );

    let error = runtime
        .eval_source("(defpackage :package-size-invalid-eval (:size -1))")
        .must_fail();
    assert!(error.to_string().contains("defpackage :size"));
}

#[test]
fn string_streams_read_and_write() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
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
fn string_streams_line_output_operations() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
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
fn file_streams_round_trip_through_with_open_file() {
    let path = std::env::temp_dir().join(format!(
        "ncl-with-open-file-evaluator-{}",
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
fn file_stream_options_cover_probe_append_and_abort() {
    let path = std::env::temp_dir().join(format!(
        "ncl-file-stream-options-evaluator-{}",
        std::process::id()
    ));
    let missing_path = std::env::temp_dir().join(format!(
        "ncl-file-stream-options-evaluator-missing-{}",
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
fn file_io_stream_reads_writes_and_appends() {
    let path = std::env::temp_dir().join(format!(
        "ncl-file-io-stream-evaluator-{}",
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
fn file_pathname_primitives_probe_rename_delete_and_date() {
    let source_path = std::env::temp_dir().join(format!(
        "ncl-file-pathname-primitives-source-{}",
        std::process::id()
    ));
    let renamed_path = std::env::temp_dir().join(format!(
        "ncl-file-pathname-primitives-renamed-{}",
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
fn evaluates_rational_literals_and_exact_arithmetic() {
    assert_eq!(
        evaluate(
            "(list 1/2 2/4 (+ 1/2 1/3) (- 3/2 1/2) (* 2/3 9/4) (/ 2/3 4/5) (+ 1 1/2) (= 1 2/2) (< 1/3 1/2) (rationalp 1/2) (rationalp 1) (typep 1/2 'ratio) (typep 1/2 'rational) (numberp 1/2) (floatp 1/2))"
        )
        .to_string(),
          "(1/2 1/2 5/6 1 3/2 5/6 3/2 T T T T T T T NIL)"
    );
}

#[test]
fn rejects_malformed_setf_places_from_table_cases() {
    let cases = [
        "(setf)",
        "(setf (car) 1)",
        "(setf (car 1) 2)",
        "(setf (cdr nil) 1)",
        "(setf (cdr '(1)) 2)",
        "(setf (car nil) 1)",
        "(setf (first '(1)) 2)",
        "(setf (nth 0 1) 2)",
        "(setf (nth -1 (list 1)) 2)",
        "(setf (nth 4 (list 1)) 2)",
        "(setf (elt 0 1) 2)",
        "(setf (elt \"a\" 0) 1)",
        "(setf (elt \"a\" -1) #\\Z)",
        "(setf (subseq '(1) 2) '(3))",
        "(setf (subseq \"abc\" 0) '(1))",
        "(setf (subseq 1 0 1) '(2))",
        "(setf (subseq \"abc\" 0 1) 2)",
        "(setf (char 1 0) #\\X)",
        "(setf (char \"a\" 0) 1)",
        "(setf (aref #(1) 2) 3)",
        "(setf (getf '(a 1 b) 'c) 2)",
        "(setf 1 2)",
        "(setf (unknown-place) 1)",
        "(setf (slot-value 1 'missing) 2)",
        "(setf (slot-value 1) 2)",
        "(setf (symbol-value 1) 2)",
        "(setf (symbol-function 1) (lambda () 1))",
        "(setf (symbol-function 'missing-function) 1)",
        "(setf (get 1 :key) 2)",
        "(setf (gethash :key 1) 2)",
        "(setf (getf 1 :key) 2)",
        "(setf (getf '(a 1 b) :key) 2)",
        "(setf (aref #(1) 0 1) 2)",
        "(setf (row-major-aref #(1) 0 1) 2)",
        "(setf (bit #(1) 0 1) 0)",
        "(setf (aref) 1)",
        "(setf (aref (make-array '(2 2)) 0) 9)",
        "(setf (aref 5 0) 9)",
        "(setf (bit) 1)",
        "(setf (bit 5 0) 1)",
        "(setf (elt (list 1 2)) 3)",
        "(setf (elt (list 1 2) 5) 3)",
        "(setf (elt \"abc\" 5) #\\X)",
        "(setf (char \"abc\") #\\X)",
        "(setf (subseq (list 1 2)) 5)",
        "(setf (svref #(1) 0 1) 2)",
        "(setf (svref #(1 2 3) 10) 9)",
        "(setf (row-major-aref 5 0) 2)",
        "(setf (symbol-value) 1)",
        r#"(setf "text" 10)"#,
        r#"(setf ("x") 10)"#,
    ];

    for source in cases {
        Runtime::new().eval_source(source).must_fail();
    }
}

#[test]
fn rejects_accessor_setf_cases_from_table() {
    let cases = [
        r"(progn (defclass setf-accessor-point () ((x :accessor setf-accessor-x))) (setf (setf-accessor-x) 1))",
        r"(progn (defclass setf-accessor-point () ((x :accessor setf-accessor-x))) (setf (setf-accessor-x 1 2) 3))",
        r"(progn (defclass setf-accessor-point () ((x :accessor setf-accessor-x))) (setf (setf-accessor-x 1) 3))",
        r"(progn (defstruct setf-accessor-record (value 0 t)) (setf (setf-accessor-record-value 1) 2))",
        r"(progn (defstruct setf-accessor-record (value 0)) (setf (setf-accessor-record-value) 2))",
        r"(progn (defstruct setf-accessor-record (value 0)) (setf (setf-accessor-record-value 1 2) 3))",
    ];

    for source in cases {
        Runtime::new().eval_source(source).must_fail();
    }
}

#[test]
fn evaluates_subseq_setf_replacement_edge_cases() {
    assert_eq!(
        evaluate("(let ((xs nil)) (setf (subseq xs 0 0) nil) xs)").to_string(),
        "NIL"
    );
    assert_eq!(
        evaluate(r#"(let ((xs (list 1 2 3))) (setf (subseq xs 0 1) "a") xs)"#).to_string(),
        "(#\\a 2 3)"
    );
}

#[test]
fn evaluates_setf_symbol_value_and_symbol_function_for_exact_symbols() {
    assert_eq!(
        evaluate(
            "(progn (setf (symbol-value '|Setf-Exact-Value|) 42) (symbol-value '|Setf-Exact-Value|))",
        )
        .to_string(),
        "42"
    );
    assert_eq!(
        evaluate(
            "(progn (setf (symbol-function '|Setf-Exact-Fn|) (lambda () 9)) (funcall (symbol-function '|Setf-Exact-Fn|)))",
        )
        .to_string(),
        "9"
    );
}

#[test]
fn evaluates_row_major_aref_setf_on_a_vector() {
    assert_eq!(
        evaluate("(let ((v #(1 2 3))) (setf (row-major-aref v 1) 9) v)").to_string(),
        "#(1 9 3)"
    );
}

#[test]
fn evaluates_rotatef_and_shiftf_generalized_places() {
    assert_eq!(
        evaluate("(let ((a 1) (b 2) (c 3)) (list (rotatef a b c) a b c))").to_string(),
        "(NIL 3 1 2)"
    );
    assert_eq!(
        evaluate("(let ((xs (list 1 2))) (list (shiftf (car xs) (car (cdr xs)) 9) xs))")
            .to_string(),
        "(1 (2 9))"
    );
}

#[test]
fn rejects_malformed_rotatef_and_shiftf_arguments() {
    Runtime::new()
        .eval_source("(shiftf (car (list 1)))")
        .must_fail();
}

#[test]
fn evaluates_define_modify_macro_on_a_symbol_macro_place() {
    assert_eq!(
        evaluate(
            "(progn
               (define-modify-macro bump-place (&optional (delta 1)) +)
               (let ((cell (list 10)))
                 (define-symbol-macro modify-macro-symbol-alias (car cell))
                 (bump-place modify-macro-symbol-alias 5)
                 cell))",
        )
        .to_string(),
        "(15)"
    );
}

#[test]
fn evaluates_define_modify_macro_on_a_custom_setf_expander_place() {
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *custom-modify-cell* 10)
               (define-setf-expander custom-modify-place ()
                 (values nil nil '(new-value)
                         '(progn
                            (setq *custom-modify-cell* new-value)
                            new-value)
                         '*custom-modify-cell*))
               (define-modify-macro bump-custom-place (&optional (delta 1)) +)
               (bump-custom-place (custom-modify-place) 4)
               *custom-modify-cell*)",
        )
        .to_string(),
        "14"
    );
}

#[test]
fn evaluates_define_modify_macro_on_a_non_container_place() {
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *modify-symbol-value-target* 5)
               (define-modify-macro bump-symbol-value-place (&optional (delta 1)) +)
               (bump-symbol-value-place (symbol-value '*modify-symbol-value-target*) 3)
               *modify-symbol-value-target*)",
        )
        .to_string(),
        "8"
    );
}

#[test]
fn rejects_malformed_modify_macro_places_from_table_cases() {
    let cases = [
        r#"(progn (define-modify-macro bump-literal-place (&optional (delta 1)) +) (bump-literal-place "text" 1))"#,
        r#"(progn (define-modify-macro bump-listy-place (&optional (delta 1)) +) (bump-listy-place ("a" "b") 1))"#,
        "(progn
           (define-setf-expander bad-arity-place (required-arg)
             (values nil nil (list 'v) '(setq v v) 'v))
           (setf (bad-arity-place) 1))",
    ];

    for source in cases {
        Runtime::new().eval_source(source).must_fail();
    }
}

#[test]
fn evaluates_get_setf_expansion_for_a_symbol_macro_place() {
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *get-setf-expansion-alias-cell* (list 42))
               (define-symbol-macro get-setf-expansion-alias
                   (car *get-setf-expansion-alias-cell*))
               (multiple-value-bind (temporaries values stores store-form access-form)
                   (get-setf-expansion 'get-setf-expansion-alias)
                 (list (length temporaries) (length values) (length stores))))",
        )
        .to_string(),
        "(1 1 1)"
    );
}

#[test]
fn rejects_malformed_get_setf_expansion_targets() {
    Runtime::new()
        .eval_source("(get-setf-expansion 5)")
        .must_fail();
    Runtime::new()
        .eval_source("(get-setf-expansion '((5) 6))")
        .must_fail();
}

#[test]
fn evaluates_custom_setf_expander_with_a_temporary_variable() {
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *custom-setf-store* (list 0))
               (define-setf-expander custom-setf-place-with-temp (index)
                 (values (list 'idx) (list index)
                         '(new-value)
                         '(progn
                            (setf (nth idx *custom-setf-store*) new-value)
                            new-value)
                         '(nth idx *custom-setf-store*)))
               (setf (custom-setf-place-with-temp 0) 99)
               *custom-setf-store*)",
        )
        .to_string(),
        "(99)"
    );
}

#[test]
fn rejects_malformed_pushnew_cases_from_table() {
    let cases = [
        "(pushnew 1)",
        "(pushnew 1 (list 2) :test)",
        "(pushnew 1 (list 2) foo 5)",
        "(pushnew 1 (list 2) :bogus 5)",
        "(pushnew 1 (list 2) :test 5)",
        r#"(pushnew "text" (list 1 2) :key (lambda (x) (if (stringp x) (error "boom") x)))"#,
        r#"(pushnew 1 (list "existing") :key (lambda (x) (if (stringp x) (error "boom") x)))"#,
        r#"(pushnew 1 (list 2) :test (lambda (a b) (error "boom")))"#,
    ];

    for source in cases {
        Runtime::new().eval_source(source).must_fail();
    }
}
