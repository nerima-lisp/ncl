#[test]
fn compiled_evaluates_function_namespace_introspection() {
    assert_eq!(
        evaluate(
            "(progn
               (defun introspection-target (value) (+ value 1))
               (list (fboundp 'car)
                     (fboundp 'introspection-target)
                     (fboundp 'missing-function)
                     (functionp (fdefinition 'car))
                     (funcall (fdefinition 'introspection-target) 4)))",
        )
        .to_string(),
        "(T T NIL T 5)"
    );
    let error = Runtime::new()
        .eval_compiled_source("(fdefinition 'missing-function)")
        .must_fail();
    assert!(matches!(
        error,
        RuntimeError::UnboundVariable { name, .. } if name == "MISSING-FUNCTION"
    ));
}

#[test]
fn compiled_rejects_malformed_symbol_and_package_primitives_from_table_cases() {
    for source in [
        "(boundp)",
        "(boundp 1)",
        "(constantp)",
        "(symbol-value)",
        "(symbol-value 1)",
        "(fboundp)",
        "(fboundp 1)",
        "(documentation)",
        "(documentation 1 2)",
        "(list-all-packages 1)",
        "(use-package)",
        "(use-package 'foo 'bar 'baz)",
        "(import)",
        "(import 'missing-symbol)",
    ] {
        assert!(
            Runtime::new().eval_compiled_source(source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn compiled_rejects_malformed_introspection_and_property_primitives_from_table_cases() {
    for source in [
        "(find-package)",
        "(package-name)",
        "(package-use-list)",
        "(make-symbol)",
        "(gensym 1 2)",
        "(intern)",
        "(find-symbol)",
        "(subtypep)",
        "(class-of)",
        "(find-class)",
        "(class-name)",
        "(compute-restarts 1)",
        "(find-restart 1 2)",
        "(restart-name)",
        "(invoke-restart)",
        "(get)",
        "(putprop)",
        "(remprop)",
        "(symbol-plist)",
        "(set)",
        "(makunbound)",
        "(fmakunbound 1 2)",
    ] {
        assert!(
            Runtime::new().eval_compiled_source(source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn compiled_rejects_sequence_primitives_without_required_arguments_from_table_cases() {
    for source in [
        "(union)",
        "(intersection 1)",
        "(set-difference)",
        "(subsetp 1)",
        "(member 1)",
        "(assoc 1)",
        "(find 1)",
        "(position 1)",
        "(count 1)",
        "(search 1)",
        "(mismatch 1)",
        "(sort 1)",
        "(every 1)",
        "(some 1)",
        "(mapcar 1)",
    ] {
        assert!(
            Runtime::new().eval_compiled_source(source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn compiled_evaluates_compile_function() {
    assert_eq!(
        evaluate(
            "(let ((function (compile nil '(lambda (value) (+ value 1)))))
               (list (compiled-function-p function)
                     (funcall function 5)))"
        )
        .to_string(),
        "(T 6)"
    );
    assert_eq!(
        evaluate("(multiple-value-list (compile nil '(lambda () 42)))").to_string(),
        "(#<FUNCTION> NIL NIL)"
    );
    assert_eq!(
        evaluate(
            "(progn
               (compile 'compiled-compile-target '(lambda (value) (* value value)))
               (list (compiled-function-p #'compiled-compile-target)
                     (compiled-compile-target 7)))"
        )
        .to_string(),
        "(T 49)"
    );
}

#[test]
fn compiled_evaluates_load_file() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/load.lisp")
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    assert_eq!(
        evaluate(&format!(
            r#"(list (load "{path}") *NCL-LOAD-VALUE* (NCL-LOAD-TARGET 1))"#
        ))
        .to_string(),
        "(T 41 42)"
    );
}

#[test]
fn compiled_evaluates_symbol_function_and_setf() {
    assert_eq!(
        evaluate(
            "(progn
               (defun compiled-symbol-function-target (value) (+ value 2))
               (let ((name 'compiled-symbol-function-target))
                 (list (functionp (symbol-function name))
                       (funcall (symbol-function name) 5)
                       (progn
                         (setf (symbol-function name)
                               (lambda (value) (+ value 3)))
                         (funcall (symbol-function name) 5))
                       (fboundp name))))",
        )
        .to_string(),
        "(T 7 8 T)"
    );
}

#[test]
fn compiled_evaluates_numeric_predicates_and_extrema() {
    assert_eq!(
        evaluate("(list (zerop 0) (plusp 1) (minusp -1) (evenp 4) (oddp 3) (min 3 1 2) (max 3 1 2) (abs -5))").to_string(),
        "(T T T T T 1 3 5)"
    );
}

#[test]
fn compiled_evaluates_format_indentation_directive() {
    assert_eq!(
        evaluate(
            r#"(list (format nil "a~I b")
                       (format nil "a~1I b")
                       (format nil "a~:I b")
                       (format nil "a~1:I b")
                       (format nil "a~I~A" 'b))"#,
        )
        .to_string(),
        r#"("a b" "a b" "a b" "a b" "aB")"#,
    );
    for source in [
        r#"(format nil "a~1,2I b")"#,
        r#"(format nil "a~@I b")"#,
        r#"(format nil "a~:@I b")"#,
    ] {
        assert!(
            Runtime::new().eval_compiled_source(source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn compiled_evaluates_standard_list_position_accessors() {
    assert_eq!(
        evaluate("(list (second '(a b c)) (third '(a b c)) (fourth '(a b c)) (tenth '(a b c)))")
            .to_string(),
        "(B C NIL NIL)"
    );
}

#[test]
fn compiled_evaluates_atomic_type_and_equality_predicates() {
    assert_eq!(
        evaluate(
            "(list
                (null nil) (null 1)
                (atom 1) (atom '(a))
                (consp '(a)) (consp nil)
                (listp '(a b)) (listp '(a . b))
                (numberp 1) (numberp \"1\")
                (integerp 1) (integerp 1.0)
                (floatp 1.0) (rationalp 1/2)
                (stringp \"a\") (simple-string-p \"a\")
                (symbolp 'a) (packagep (find-package :cl-user))
                (functionp #'car)
                (eq 'a 'a) (eq \"a\" \"a\")
                (eql 1 1) (eql 1 1.0)
                (equal '(a (b)) '(a (b)))
                (equal '(a) '(b))
                (equalp \"AbC\" \"aBc\")
                (equalp #\\A #\\a)
                (equalp '(A #(1 2)) '(a #(1 2)))
                (equalp #(1 2) #(1 3)))",
        )
        .to_string(),
        "(T NIL T NIL T NIL T NIL T NIL T NIL T T T T T NIL T T NIL T NIL T NIL T T T NIL)",
    );
}

#[test]
fn compiled_validates_type_designator_shapes_and_bounds() {
    assert_eq!(
        evaluate(
            "(list
                (handler-case (typep 1 '(integer 0 1 2)) (error (condition) :error))
                (handler-case (typep 1 '(vector integer 1 2)) (error (condition) :error))
                (handler-case (typep 1 '(array integer (1 2) 3)) (error (condition) :error))
                (handler-case (typep 1 '(mod -1)) (error (condition) :error))
                (handler-case (typep 1 '(unsigned-byte -1)) (error (condition) :error))
                (handler-case (typep 1 '(signed-byte 65)) (error (condition) :error))
                (handler-case (typep 1 '(cons integer)) (error (condition) :error))
                (handler-case (typep 1 '(or integer)) (error (condition) :error))
                (handler-case (typep 1 '(not integer extra)) (error (condition) :error)))",
        )
        .to_string(),
        "(:ERROR :ERROR :ERROR :ERROR :ERROR T NIL T :ERROR)"
    );
}

#[test]
fn compiled_rejects_invalid_type_designator_shapes() {
    for source in [
        "(typep 1 '(integer 0 1 2))",
        "(typep 1 '(vector integer 1 2))",
        "(typep 1 '(array integer (1 2) 3))",
        "(typep 1 '(mod -1))",
        "(typep 1 '(unsigned-byte -1))",
        "(typep 1 '(not integer extra))",
        "(subtypep 'integer '(integer 0 1 2))",
        "(subtypep '(vector integer 1 2) 'vector)",
    ] {
        assert!(
            Runtime::new().eval_compiled_source(source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn compiled_covers_type_predicate_boundaries() {
    assert_eq!(
        evaluate(
            "(list
                (handler-case (endp 1) (error (condition) :error))
                (handler-case (symbol-name 1) (error (condition) :error))
                (symbol-name '#:temporary)
                (symbol-package '#:temporary)
                (symbol-package :answer)
                (symbol-package nil)
                (handler-case (typep 1 'unknown-type) (error (condition) :error))
                (handler-case (typep 1 '(unknown-type)) (error (condition) :error))
                (handler-case (typep 1 '()) (error (condition) :error))
                (handler-case (typep 1 '(not integer extra)) (error (condition) :error))
                (typep 1 '(member 2 3))
                (typep '(1 . 2) '(cons integer integer))
                (typep '(1 . x) '(cons integer integer))
                (typep #(1 2) '(vector string 2))
                (typep #(1 2) '(array integer *))
                (typep #(1 2) '(array integer (2))))",
        )
        .to_string(),
        "(:ERROR :ERROR \"TEMPORARY\" NIL KEYWORD COMMON-LISP :ERROR :ERROR NIL :ERROR NIL T NIL NIL T T)"
    );
}

#[test]
fn compiled_evaluates_subtypep() {
    let values = Runtime::new()
        .eval_compiled_source(
            r"(progn
                 (defclass subtypep-parent () ())
                 (defclass subtypep-child (subtypep-parent) ())
                 (defstruct subtypep-record value)
                 (list
                   (multiple-value-list (subtypep 'integer 'number))
                   (multiple-value-list (subtypep '(integer 0 5) '(integer -1 10)))
                   (multiple-value-list (subtypep '(integer 0 10) '(integer 1 5)))
                   (multiple-value-list (subtypep 'subtypep-child 'subtypep-parent))
                   (multiple-value-list (subtypep 'subtypep-record 'structure))
                   (multiple-value-list (subtypep 'string 'sequence))))",
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "((T T) (T T) (NIL T) (T T) (T T) (T T))"
    );
}
#[test]
fn compiled_evaluates_character_stream_output_operations() {
    assert_eq!(
        evaluate(
            r"(let ((stream (make-string-output-stream)))
                 (list (write-char #\A stream)
                       (terpri stream)
                       (fresh-line stream)
                       (get-output-stream-string stream)))",
        )
        .to_string(),
        "(#\\A NIL NIL \"A\\n\")"
    );
}

#[test]
fn compiled_evaluates_string_stream_output_operations() {
    assert_eq!(
        evaluate(
            r#"(let ((stream (make-string-output-stream)))
                 (list (write-string "ab" stream)
                       (write-line "cd" stream)
                       (get-output-stream-string stream)))"#,
        )
        .to_string(),
        r#"("ab" "cd" "abcd\n")"#
    );
}

#[test]
fn compiled_evaluates_sequence_stream_output_operations() {
    assert_eq!(
        evaluate(
            r#"(let ((stream (make-string-output-stream)))
                 (list (write-sequence '(#\A #\B #\C) stream :start 1)
                       (get-output-stream-string stream)))"#,
        )
        .to_string(),
        r#"((#\A #\B #\C) "BC")"#
    );
}

use super::*;
