use super::*;

#[test]
fn evaluates_function_namespace_introspection() {
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
        .eval_source("(fdefinition 'missing-function)")
        .must_fail();
    assert!(matches!(
        error,
        ncl_runtime::RuntimeError::UnboundVariable { name, .. }
            if name == "MISSING-FUNCTION"
    ));
}

#[test]
fn rejects_invalid_function_introspection_arguments_from_table_cases() {
    let cases = [
        "(fboundp)",
        "(fboundp 1)",
        "(macro-function)",
        "(macro-function 1)",
        "(macro-function 'car 1)",
        "(special-operator-p)",
        "(special-operator-p 1)",
        "(compiled-function-p)",
        "(compiled-function-p 1 2)",
        "(fdefinition)",
        "(fdefinition 1)",
        "(symbol-function)",
        "(symbol-function 1)",
    ];

    for source in cases {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_symbol_creation_boundaries_from_table_cases() {
    let cases = [
        (
            r#"(symbol-name (make-symbol "temporary"))"#,
            r#""temporary""#,
        ),
        (r"(symbol-name (gensym))", r#""G0""#),
        (r#"(symbol-name (gensym "TMP"))"#, r#""TMP0""#),
        (r"(symbol-name (gensym 'prefix))", r#""PREFIX0""#),
        (
            r#"(multiple-value-list (find-symbol "missing"))"#,
            "(NIL NIL)",
        ),
    ];

    assert_value_cases(evaluate, &cases);

    for source in [
        "(make-symbol)",
        "(make-symbol 1)",
        "(gensym 1 2)",
        "(intern)",
        "(intern 1)",
        "(find-symbol)",
        "(find-symbol 1 2 3)",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_compile_function() {
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
               (compile 'compile-target '(lambda (value) (* value value)))
               (list (compiled-function-p #'compile-target)
                     (compile-target 7)))"
        )
        .to_string(),
        "(T 49)"
    );

    for source in [
        "(compile)",
        "(compile nil)",
        "(compile 1 '(lambda () 1))",
        "(compile 'missing-compile-function)",
        "(compile 'car 1)",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn compiles_an_existing_function_looked_up_by_an_escaped_exact_name() {
    assert_eq!(
        evaluate(
            "(progn
               (defun |ExactCompileFn| (value) (* value 2))
               (compile '|ExactCompileFn|)
               (funcall (function |ExactCompileFn|) 5))"
        )
        .to_string(),
        "10"
    );
}

#[test]
fn rejects_compiling_a_name_that_is_bound_to_a_non_function_value() {
    let error = Runtime::new()
        .eval_source(
            "(progn (defvar compile-non-function-value 5) (compile 'compile-non-function-value))",
        )
        .must_fail();
    assert!(matches!(
        error,
        ncl_runtime::RuntimeError::NotCallable { .. }
    ));
}

#[test]
fn evaluates_load_file() {
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

    for source in [
        "(load)",
        "(load 1)",
        "(load \"missing-file-for-test.lisp\")",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_symbol_function_and_setf() {
    assert_eq!(
        evaluate(
            "(progn
               (defun symbol-function-target (value) (+ value 2))
               (let ((name 'symbol-function-target))
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
fn evaluates_symbol_values_and_property_lists_from_table_cases() {
    let cases = [
        (
            "(progn (defparameter symbol-value-target 10)\n                 (list (boundp 'symbol-value-target)\n                       (symbol-value 'symbol-value-target)\n                       (set 'symbol-value-target 11)\n                       (symbol-value 'symbol-value-target)))",
            "(T 10 11 11)",
        ),
        (
            "(let ((symbol (make-symbol \"temporary\")))\n                 (list (symbolp symbol) (symbol-name symbol)\n                       (boundp symbol) (constantp symbol)))",
            "(T \"temporary\" NIL NIL)",
        ),
        (
            "(let ((symbol 'property-target))\n                 (list (get symbol :answer)\n                       (putprop symbol 42 :answer)\n                       (get symbol :answer)\n                       (remprop symbol :answer)\n                       (get symbol :answer)\n                       (symbol-plist symbol)))",
            "(NIL 42 42 T NIL NIL)",
        ),
    ];

    assert_value_cases(evaluate, &cases);
}

#[test]
fn rejects_invalid_symbol_property_operations_from_table_cases() {
    let cases = [
        "(get)",
        "(get 1 :answer)",
        "(get 'property-target)",
        "(putprop)",
        "(putprop 1 2 :answer)",
        "(remprop)",
        "(remprop 1 :answer)",
        "(symbol-plist)",
        "(symbol-plist 1)",
    ];

    for source in cases {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_format_directives_from_table_cases() {
    let cases = [
        (
            r#"(format nil "~A/~S" "text" "text")"#,
            r#""text/\"text\"""#,
        ),
        (
            r#"(format nil "~D/~B/~O/~X" -12 10 8 255)"#,
            r#""-12/1010/10/FF""#,
        ),
        (r#"(format nil "~C/~~/~%end" #\!)"#, r#""!/~/\nend""#),
        (r#"(format nil "line~&next")"#, r#""line\nnext""#),
    ];

    assert_value_cases(evaluate, &cases);
}

#[test]
fn evaluates_numeric_predicates_and_extrema() {
    assert_value_cases(
        evaluate,
        &[(
            "(list (zerop 0) (plusp 1) (minusp -1) (evenp 4) (oddp 3) (min 3 1 2) (max 3 1 2) (abs -5))",
            "(T T T T T 1 3 5)",
        )],
    );
}

#[test]
fn rejects_invalid_integer_arithmetic_arguments() {
    for source in [
        "(mod 1 0)",
        "(rem 1 0)",
        "(ash 1 1.5)",
        "(logand 1 1.0)",
        "(gcd 1 1.0)",
        "(integer-length 1.0)",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn rejects_invalid_format_entry_arguments_and_destinations() {
    for source in [
        "(format nil)",
        "(format nil 1)",
        "(format 1 \"text\")",
        "(let ((stream (make-string-output-stream)))
           (close stream)
           (format stream \"text\"))",
    ] {
        Runtime::new().eval_source(source).must_fail();
    }
}

#[test]
fn rejects_invalid_format_directives() {
    for source in [
        r#"(format nil "~")"#,
        r#"(format nil "~Q")"#,
        r#"(format nil "~:~")"#,
        r#"(format nil "~}")"#,
        r#"(format nil "~A" )"#,
        r#"(format nil "~D" 1.5)"#,
        r#"(format nil "~[zero~;one~]")"#,
        r#"(format nil "~{~A~}" 1)"#,
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn rejects_invalid_general_float_format_parameters() {
    for source in [
        r#"(format nil "~,-1G" 1.25)"#,
        r#"(format nil "~,,-1G" 1.25)"#,
        r#"(format nil "~,,999999999999999999999999999999G" 1.25)"#,
        r#"(format nil "~:G" 1.25)"#,
        r#"(format nil "~,xG" 1.25)"#,
        r#"(format nil "~,,xG" 1.25)"#,
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_format_indentation_directive() {
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
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_standard_list_position_accessors() {
    assert_eq!(
        evaluate("(list (second '(a b c)) (third '(a b c)) (second '(a)) (third nil))").to_string(),
        "(B C NIL NIL)"
    );
}

#[test]
fn evaluates_type_designator_boundaries_and_failures() {
    assert_eq!(
        evaluate(
            "(list
                (typep nil 'null)
                (typep nil 'symbol)
                (typep #(1 2) '(vector * *))
                (typep #(1 2) '(simple-vector *))
                (typep #(0 1) '(bit-vector *))
                (typep 255 '(unsigned-byte 8))
                (typep -128 '(signed-byte 8))
                (typep 256 '(unsigned-byte 8))
                (typep -129 '(signed-byte 8))
                (typep 0 '(mod 0))
                (typep #(1 2) '(simple-vector 3))
                (typep #(0 2) 'bit-vector)
                (typep #(0 2) '(bit-vector 3))
                (typep #(1 2) '(vector integer 3))
                (typep (make-array '(2 2) :initial-element 1)
                       '(array integer (2 2)))
                (typep (make-array '(2 2) :initial-element 1)
                       '(array integer (2 3)))
                (typep (make-array '(2 2) :initial-element 1)
                       '(array string (2 2))))",
        )
        .to_string(),
        "(T T T T T T T NIL NIL NIL NIL NIL NIL NIL T NIL NIL)"
    );

    let error = Runtime::new()
        .eval_source("(typep 1 '(unknown-type 1))")
        .must_fail();
    assert!(matches!(
        error,
        ncl_runtime::RuntimeError::InvalidForm { .. }
    ));
}

#[test]
fn rejects_invalid_type_designator_shapes() {
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
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_subtypep() {
    let values = Runtime::new()
        .eval_source(
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
                   (multiple-value-list (subtypep 'string 'sequence))
                   (multiple-value-list (subtypep 'null 'symbol))
                   (multiple-value-list (subtypep '(integer 0 5) '(integer 1 10)))))",
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "((T T) (T T) (NIL T) (T T) (T T) (T T) (T T) (NIL T))"
    );
}

#[test]
fn rejects_invalid_parse_integer_arguments() {
    for source in [
        "(parse-integer)",
        "(parse-integer \"1\" :start)",
        "(parse-integer 1)",
        "(parse-integer \"1\" :unknown 1)",
        "(parse-integer \"1\" :radix 1)",
        "(parse-integer \"1\" :radix 37)",
        "(parse-integer \"1\" :start 2 :end 1)",
        "(parse-integer \"1\" :end 2)",
        "(parse-integer \"no-integer\")",
        "(parse-integer \"1x\")",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_type_predicates_and_structural_equality() {
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
        "(T NIL T NIL T NIL T NIL T NIL T NIL T T T T T NIL T T NIL T NIL T NIL T T T NIL)"
    );
}

#[test]
fn evaluates_structural_equality_boundaries_from_table_cases() {
    let cases = [
        ("(eql 1/2 1/2)", "T"),
        ("(eql 1.0 1.0)", "T"),
        ("(equal \"abc\" \"abc\")", "T"),
        ("(equal '(a . b) '(a . b))", "T"),
        ("(equal '(a . b) '(a . c))", "NIL"),
        ("(equalp 1 1.0)", "T"),
        ("(equalp 1/2 0.5)", "T"),
        ("(equalp #(1 2) #(1 2))", "T"),
        ("(equalp #(1) '(1))", "NIL"),
        (
            "(equalp (make-array 2 :initial-element 1) (make-array 2 :initial-element 1))",
            "T",
        ),
        ("(equalp '(a . b) '(a . b))", "T"),
        ("(equalp '(a . b) '(a . c))", "NIL"),
    ];

    for (source, expected) in cases {
        assert_eq!(evaluate(source).to_string(), expected, "{source}");
    }
}

#[test]
fn evaluates_collection_and_runtime_type_predicates() {
    assert_eq!(
        evaluate(
            "(let ((array (make-array 2))
                   (table (make-hash-table))
                   (stream (make-string-output-stream)))
               (list (characterp #\\a) (characterp 1)
                     (keywordp :name) (keywordp 'name)
                     (vectorp #(1 2)) (vectorp '(1 2))
                     (arrayp array) (arrayp #(1 2))
                     (hash-table-p table) (hash-table-p array)
                     (streamp stream) (streamp table)))"
        )
        .to_string(),
        "(T NIL T NIL T NIL T T T NIL T NIL)"
    );
}

#[test]
fn evaluates_stream_predicates_for_both_directions_and_non_streams() {
    assert_eq!(
        evaluate(
            "(let ((input (make-string-input-stream \"in\"))
                   (output (make-string-output-stream)))
               (list (streamp input) (streamp 1)
                     (input-stream-p input) (input-stream-p output)
                     (output-stream-p input) (output-stream-p output)
                     (input-stream-p nil) (output-stream-p nil)))"
        )
        .to_string(),
        "(T NIL T NIL NIL T NIL NIL)"
    );
}

#[test]
fn rejects_invalid_close_argument_shapes() {
    for source in [
        "(close)",
        "(close nil nil)",
        "(close nil :unknown t)",
        "(close nil :abort)",
    ] {
        Runtime::new().eval_source(source).must_fail();
    }
}

#[test]
fn rejects_invalid_file_primitive_arguments() {
    let cases = [
        "(open)",
        "(open \"missing\" :direction)",
        "(open 1)",
        "(open \"missing\" 1 :input)",
        "(open \"missing\" :unknown :error)",
        "(open \"missing\" :direction 1)",
        "(open \"missing\" :direction :unknown)",
        "(open \"missing\" :if-does-not-exist :unknown)",
        "(open \"missing\" :if-exists :unknown)",
        "(probe-file)",
        "(probe-file 1)",
        "(delete-file)",
        "(delete-file 1)",
        "(rename-file \"old\")",
        "(rename-file 1 \"new\")",
        "(truename)",
        "(truename 1)",
        "(file-write-date)",
        "(file-write-date 1)",
    ];

    for source in cases {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_symbol_package_boundaries() {
    assert_eq!(
        evaluate(
            "(let ((uninterned (make-symbol \"temporary\")))
               (list (symbol-package 'name)
                     (symbol-package :name)
                     (symbol-package nil)
                     (symbol-package uninterned)))"
        )
        .to_string(),
        "(NCL-USER KEYWORD COMMON-LISP NIL)"
    );
    assert!(Runtime::new().eval_source("(symbol-package 1)").is_err());
}

#[test]
fn rejects_invalid_package_and_method_primitive_arguments_from_table() {
    let cases = [
        "(documentation 1 2)",
        "(documentation *package*)",
        "(list-all-packages 1)",
        "(next-method-p 1)",
        "(call-next-method)",
        "(use-package)",
        "(use-package 1 2 3)",
        "(use-package 1)",
        "(export 1)",
        "(import 1)",
        "(shadow 1)",
        "(unintern 1)",
    ];

    for source in cases {
        Runtime::new().eval_source(source).must_fail();
    }
}
