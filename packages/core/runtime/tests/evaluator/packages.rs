use super::{Runtime, evaluate};

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
        .unwrap();

    assert_eq!(values[3].to_string(), "42");
    assert_eq!(runtime.current_package(), "DEMO");

    let values = runtime
        .eval_source("(in-package :ncl-user) demo:answer")
        .unwrap();
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
        .unwrap_err();

    assert!(matches!(error, ncl_runtime::RuntimeError::Package { .. }));
    assert_eq!(
        runtime
            .eval_source("hidden::secret")
            .unwrap()
            .pop()
            .unwrap()
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
        .unwrap();

    assert_eq!(values.last().unwrap().to_string(), "(41 2)");
    assert_eq!(
        runtime
            .eval_source("(define answer 99) (list answer (plus-one 1))")
            .unwrap()
            .last()
            .unwrap()
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
        .unwrap();

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
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        r#"(T T "PACKAGE-INSPECT-EVAL" T NIL "COMMON-LISP" T)"#
    );
}

#[test]
fn package_metadata_lists_nicknames_shadowing_symbols_and_users() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(defpackage :package-metadata-owner-eval
                 (:use :common-lisp)
                 (:nicknames :package-metadata-alias-eval)
                 (:shadow :local-shadow))
               (defpackage :package-metadata-user-eval
                 (:use :common-lisp :package-metadata-owner-eval))
               (let ((owner (find-package :package-metadata-owner-eval)))
                 (list (package-nicknames owner)
                       (symbol-name (car (package-shadowing-symbols owner)))
                       (package-name (car (package-used-by-list owner)))))"#,
        )
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        r#"(("PACKAGE-METADATA-ALIAS-EVAL") "LOCAL-SHADOW" "PACKAGE-METADATA-USER-EVAL")"#
    );
}

#[test]
fn package_lifecycle_operations_work() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(defpackage :package-lifecycle-use-eval (:use :common-lisp))
               (let ((package (make-package
                                :package-lifecycle-made-eval
                                :nicknames '(:package-lifecycle-nickname-eval)
                                :use '(:package-lifecycle-use-eval))))
                 (let ((before (list (package-name package)
                                     (package-nicknames package)
                                     (mapcar #'package-name
                                             (package-use-list package)))))
                   (let ((renamed (rename-package
                                    package
                                    :package-lifecycle-renamed-eval
                                    '(:package-lifecycle-renamed-nickname-eval))))
                     (list before
                           (package-name renamed)
                           (package-nicknames renamed)
                           (package-name
                            (find-package :package-lifecycle-renamed-nickname-eval))
                           (find-package :package-lifecycle-made-eval)
                           (delete-package renamed)
                           (find-package :package-lifecycle-renamed-eval)
                           (find-package :package-lifecycle-renamed-nickname-eval)))))"#,
        )
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        r#"(("PACKAGE-LIFECYCLE-MADE-EVAL" ("PACKAGE-LIFECYCLE-NICKNAME-EVAL") ("PACKAGE-LIFECYCLE-USE-EVAL")) "PACKAGE-LIFECYCLE-RENAMED-EVAL" ("PACKAGE-LIFECYCLE-RENAMED-NICKNAME-EVAL") "PACKAGE-LIFECYCLE-RENAMED-EVAL" NIL T NIL NIL)"#
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
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
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
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        r#"((42 T) (42 T) 7 T NIL (NIL NIL))"#
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
        .unwrap();

    assert_eq!(values.last().unwrap().to_string(), r#"(T T T 41 41)"#);
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
        .unwrap();

    assert_eq!(values.last().unwrap().to_string(), r#"(42 42 42 T)"#);
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
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
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
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        r#"(42 "local owner documentation" NIL)"#
    );
}

#[test]
fn defpackage_size_option_is_accepted_and_validated() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(defpackage :package-size-eval
                 (:use :common-lisp)
                 (:size 0))
               (package-name (find-package :package-size-eval))"#,
        )
        .unwrap();

    assert_eq!(values.last().unwrap().to_string(), "\"PACKAGE-SIZE-EVAL\"");

    let error = runtime
        .eval_source("(defpackage :package-size-invalid-eval (:size -1))")
        .unwrap_err();
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
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
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
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
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
        pathname = pathname
    );

    assert_eq!(evaluate(&source).to_string(), "T");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello");
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
        pathname = pathname,
        missing_pathname = missing_pathname
    );

    assert_eq!(evaluate(&source).to_string(), "T");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "ab");
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
    std::fs::write(&path, "abc").unwrap();
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
        pathname = pathname
    );

    assert_eq!(evaluate(&source).to_string(), "(T T T T T)");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "aZc!");
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
    std::fs::write(&source_path, "content").unwrap();
    let form = format!(
        r#"(let ((original (probe-file {source})))
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
                     (null (probe-file {renamed})))))"#,
        source = source,
        renamed = renamed
    );

    assert_eq!(evaluate(&form).to_string(), "(T T T T T T T T T T)");
    let _ = std::fs::remove_file(source_path);
    let _ = std::fs::remove_file(renamed_path);
}
