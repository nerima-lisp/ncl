#[test]
fn compiled_supports_uninterned_symbols_and_gensym() {
    assert_evaluates_to(
        Runtime::eval_compiled_source,
        r#"(let ((symbol (make-symbol "foo")))
            (list (symbolp symbol)
                  (symbol-package symbol)
                  (symbol-name symbol)
                  (eq symbol symbol)
                  (eq '#:foo '#:foo)))"#,
        r#"(T NIL "foo" T NIL)"#,
    );
    assert_evaluates_to(
        Runtime::eval_compiled_source,
        r#"(let ((symbol (gensym "TMP")))
            (list (symbolp symbol) (symbol-package symbol) (symbol-name symbol)))"#,
        r#"(T NIL "TMP0")"#,
    );
}

#[test]
fn compiled_evaluates_equality_predicates() {
    assert_eq!(
        evaluate(
            r#"(list (eq 1 1)
                      (eql 1 1)
                      (equal '(1 (2)) '(1 (2)))
                      (equalp "Text" "text"))"#,
        )
        .to_string(),
        "(T T T T)",
    );
}

#[test]
fn compiled_evaluates_numeric_inequality() {
    assert_eq!(evaluate("(list (/= 1 2 3) (/= 1 2 1) (/= 1))").to_string(), "(T NIL T)");
}

#[test]
fn compiled_evaluates_numeric_rounding() {
    assert_eq!(
        evaluate("(list (multiple-value-list (floor 7 2)) (multiple-value-list (ceiling 7 2)) (multiple-value-list (truncate -7 2)) (multiple-value-list (round 7 2)))").to_string(),
        "((3 1) (4 -1) (-3 -1) (4 -1))",
    );
}

#[test]
fn compiled_evaluates_transcendental_and_complex_numeric_operations() {
    assert_eq!(
        evaluate("(list (sqrt 9) (sin 0) (cos 0) (exp 0) (realpart #C(2 3)) (imagpart #C(2 3)) (conjugate #C(2 3)))").to_string(),
        "(3 0.0 1.0 1.0 2 3 #C(2 -3))",
    );
}

#[test]
fn compiled_evaluates_expt() {
    assert_eq!(evaluate("(expt 2 10)").to_string(), "1024");
}

#[test]
fn compiled_evaluates_rational_conversion() {
    assert_eq!(evaluate("(rational 1.5)").to_string(), "3/2");
}

#[test]
fn compiled_evaluates_float_conversion() {
    assert_eq!(evaluate("(float 3)").to_string(), "3.0");
}

#[test]
fn compiled_evaluates_rationalize_conversion() {
    assert_eq!(evaluate("(rationalize 0.5)").to_string(), "1/2");
}

#[test]
fn compiled_evaluates_integer_square_root() {
    assert_eq!(evaluate("(isqrt 10)").to_string(), "3");
}

#[test]
fn compiled_evaluates_logarithm() {
    assert_eq!(evaluate("(log 1)").to_string(), "0.0");
}

#[test]
fn compiled_evaluates_complex_constructor() {
    assert_eq!(evaluate("(complex 1 2)").to_string(), "#C(1 2)");
}

#[test]
fn compiled_evaluates_not_and_null() {
    assert_eq!(evaluate("(list (not nil) (not 1) (null nil) (null 1))").to_string(), "(T NIL T NIL)");
}

#[test]
fn compiled_evaluates_arithmetic() {
    assert_eq!(evaluate("(+ 7 (* 6 5))").to_string(), "37");
}

#[test]
fn compiled_defines_and_expands_symbol_macros() {
    assert_eq!(
        evaluate("(define-symbol-macro answer 42) answer").to_string(),
        "42"
    );
}

#[test]
fn compiled_promotes_overflowing_arithmetic_and_large_literals_to_bignums() {
    // FR-017: the VM has its own literal-parsing path (constant_value's
    // Constant::BigInteger arm) and its own arithmetic path, independent of
    // the interpreted evaluator's. Neither was exercised by any prior test
    // in this file, despite the commit claiming both engines were verified.
    assert_eq!(
        evaluate("(+ 9223372036854775807 1)").to_string(),
        "9223372036854775808"
    );

    // A decimal literal too large for i64 must parse as a bignum directly,
    // not silently misparse as a float.
    assert_eq!(
        evaluate("99999999999999999999999999").to_string(),
        "99999999999999999999999999"
    );
    assert_eq!(
        evaluate("(typep 99999999999999999999999999 'bignum)").to_string(),
        "T"
    );
    assert_eq!(
        evaluate("(integerp 99999999999999999999999999)").to_string(),
        "T"
    );
}

#[test]
fn compiled_flet_uses_a_separate_function_namespace() {
    assert_eq!(
        evaluate(
            "(let ((twice (lambda (x) 99)))
               (flet ((twice (x) (+ x x)))
                 (list (twice 3) (funcall (function twice) 4))))",
        )
        .to_string(),
        "(6 8)",
    );
}

#[test]
fn compiled_labels_supports_mutual_recursion() {
    assert_eq!(
        evaluate(
            "(labels ((local-even (n)
                        (if (= n 0) t (local-odd (- n 1))))
                      (local-odd (n)
                        (if (= n 0) nil (local-even (- n 1)))))
               (list (local-even 6) (local-even 5)))",
        )
        .to_string(),
        "(T NIL)",
    );
}

#[test]
fn compiled_short_circuits_if_and_or() {
    assert_eq!(evaluate("(if nil (/ 1 0) 7)").to_string(), "7");
    assert_eq!(evaluate("(if t 8 (/ 1 0))").to_string(), "8");
    assert_eq!(evaluate("(and nil (/ 1 0))").to_string(), "NIL");
    assert_eq!(evaluate("(or 9 (/ 1 0))").to_string(), "9");
}

#[test]
fn compiled_closures_capture_outer_lexicals_and_mutate_them_with_setq() {
    assert_eq!(
        evaluate(
            "(let ((counter 0))
               (let ((next (lambda () (setq counter (+ counter 1)))))
                 (list (next) (next) counter)))",
        )
        .to_string(),
        "(1 2 2)"
    );
}

#[test]
fn compiled_let_is_parallel_and_let_star_is_sequential() {
    assert_eq!(
        evaluate("(let ((x 1)) (let ((x 2) (y x)) y))").to_string(),
        "1"
    );
    assert_eq!(
        evaluate("(let ((x 1)) (let* ((x 2) (y x)) y))").to_string(),
        "2"
    );
}

#[test]
fn compiled_globals_persist_across_forms() {
    let values = Runtime::new()
        .eval_compiled_source("(define answer 41) (setq answer (+ answer 1)) answer")
        .must_exist();

    assert_eq!(values[0].to_string(), "41");
    assert_eq!(values[1].to_string(), "42");
    assert_eq!(values[2].to_string(), "42");
}

#[test]
fn compiled_evaluates_defconstant_and_constantp() {
    assert_eq!(
        evaluate(
            "(progn
               (defconstant +answer+ 42)
               (list +answer+
                     (constantp '+answer+)
                     (constantp 42)
                     (constantp \"text\")))",
        )
        .to_string(),
        "(42 T T T)"
    );

    assert!(
        Runtime::new()
            .eval_compiled_source("(defconstant +answer+ 42) (setq +answer+ 7)")
            .is_err()
    );
    assert!(
        Runtime::new()
            .eval_compiled_source("(defconstant +answer+ 42) (setf (symbol-value '+answer+) 7)")
            .is_err()
    );
    assert!(
        Runtime::new()
            .eval_compiled_source("(defconstant +answer+ 42) (psetq +answer+ 7)")
            .is_err()
    );
}

#[test]
fn compiled_supports_cond_when_unless_and_dynamic_bindings() {
    let values = Runtime::new()
        .eval_compiled_source(
            "(defvar answer 1)
             (defvar answer (/ 1 0))
             (defparameter answer (+ answer 1))
             (when (= answer 2) (setq answer 3))
             (unless nil (setq answer (+ answer 1)))
             (cond ((= answer 0) 10) ((= answer 4) answer) (t 99))
             (cond ((= answer 4)))",
        )
        .must_exist();

    assert_eq!(values[0].to_string(), "1");
    assert_eq!(values[1].to_string(), "1");
    assert_eq!(values[2].to_string(), "2");
    assert_eq!(values[3].to_string(), "3");
    assert_eq!(values[4].to_string(), "4");
    assert_eq!(values[5].to_string(), "4");
    assert_eq!(values[6].to_string(), "T");
}

#[test]
fn compiled_evaluates_case_and_ecase_with_eql_keys() {
    assert_eq!(
        evaluate(
            "(let ((count 0))
               (list
                 (case (progn (incf count) 2)
                   ((1) :one)
                   ((2 3) :two)
                   (otherwise :other))
                 count
                 (case 9 ((1) :one) (t :fallback))))",
        )
        .to_string(),
        "(:TWO 1 :FALLBACK)"
    );

    let error = Runtime::new()
        .eval_compiled_source("(ecase 9 ((1) :one))")
        .must_fail();
    assert!(matches!(
        error,
        RuntimeError::InvalidForm { message, .. } if message == "ecase fell through"
    ));
}

#[test]
fn compiled_evaluates_typecase_and_etypecase_with_typep() {
    assert_eq!(
        evaluate(
            "(let ((count 0))
               (list
                 (typecase (progn (incf count) 2)
                   (integer :integer)
                   (string :string)
                   (otherwise :other))
                 count
                 (typecase \"text\"
                   (integer :integer)
                   (otherwise :other))))",
        )
        .to_string(),
        "(:INTEGER 1 :OTHER)"
    );

    let error = Runtime::new()
        .eval_compiled_source("(etypecase \"text\" (integer :integer))")
        .must_fail();
    assert!(matches!(
        error,
        RuntimeError::InvalidForm { message, .. } if message == "etypecase fell through"
    ));
}

#[test]
fn compiled_spreads_multiple_values_into_a_call() {
    assert_eq!(
        evaluate(
            "(multiple-value-call #'list
               (values 1 2)
               (values 3 4))",
        )
        .to_string(),
        "(1 2 3 4)"
    );
}

#[test]
fn compiled_supports_quasiquote_funcall_and_apply() {
    assert_eq!(
        evaluate("((lambda (value) `(item ,value ,@(list 3 4))) 2)").to_string(),
        "(ITEM 2 3 4)"
    );
    assert_eq!(
        evaluate("(funcall (lambda (x y) (+ x y)) 2 3)").to_string(),
        "5"
    );
    assert_eq!(
        evaluate("(apply (lambda (x y z) (+ x y z)) 1 '(2 3))").to_string(),
        "6"
    );
}

#[test]
fn compiled_rejects_invalid_apply_and_mapcar_arguments() {
    for source in [
        "(apply #'list 1 2)",
        "(mapcar #'list '(1 2) 3)",
        "(mapcar #'list)",
    ] {
        Runtime::new().eval_compiled_source(source).must_fail();
    }
}

#[test]
fn compiled_reports_runtime_errors_from_value_instructions() {
    for source in [
        "(funcall 42)",
        "(funcall #'missing-compiled-function)",
        "(multiple-value-call 42 1)",
    ] {
        let error = Runtime::new().eval_compiled_source(source).must_fail();
        assert!(
            matches!(
                error,
                RuntimeError::InvalidForm { .. }
                    | RuntimeError::NotCallable { .. }
                    | RuntimeError::UnboundVariable { .. }
            ),
            "{source}: {error:?}"
        );
    }
}

#[test]
fn compiled_rest_parameters_bind_lists_and_capture_lexicals() {
    let values = Runtime::new()
        .eval_compiled_source(
            "(defun collect (first &rest rest) (list first rest))
             (list (collect 1)
                   (collect 1 2 3)
                   (funcall (lambda (first &rest rest) (list first rest)) 7 8 9)
                   (apply #'collect 4 '(5 6))
                   (let ((offset 10))
                     ((lambda (first &rest rest)
                        (list (+ offset first) rest))
                      1 2 3))
                   ((lambda (&rest values) values)))",
        )
        .must_exist();

    assert_eq!(
        values[1].to_string(),
        "((1 NIL) (1 (2 3)) (7 (8 9)) (4 (5 6)) (11 (2 3)) NIL)"
    );
}

#[test]
fn compiled_optional_parameters_use_defaults_and_supplied_p() {
    let values = Runtime::new()
        .eval_compiled_source(
            "(defun describe (required &optional (optional (+ required 1) supplied-p) &rest rest)
               (list required optional supplied-p rest))
             (list (describe 4)
                   (describe 4 nil)
                   (describe 4 7 8 9))",
        )
        .must_exist();

    assert_eq!(
        values[1].to_string(),
        "((4 5 NIL NIL) (4 NIL T NIL) (4 7 T (8 9)))"
    );
}

#[test]
fn compiled_optional_parameters_evaluate_chained_defaults() {
    let values = Runtime::new()
        .eval_compiled_source(
            "(defun ordered (base
                              &optional (a (+ base 1) a-p)
                                        (b (+ a 1) b-p)
                              &rest rest)
               (list base a a-p b b-p rest))
             (list (ordered 1)
                   (ordered 1 10)
                   (ordered 1 10 20 30))",
        )
        .must_exist();

    assert_eq!(
        values[1].to_string(),
        "((1 2 NIL 3 NIL NIL) (1 10 T 11 NIL NIL) (1 10 T 20 T (30)))"
    );
}

#[test]
fn compiled_auxiliary_parameters_evaluate_sequentially_after_other_bindings() {
    let values = Runtime::new()
        .eval_compiled_source(
            "(defun describe (required
                              &optional (optional (+ required 1))
                              &rest rest
                              &aux (sum (+ required optional))
                                   (next (+ sum 1)))
               (list sum next))
             (list (describe 4)
                   (describe 4 7 8 9)
                   ((lambda (&rest values &aux (copy values)) copy) 1 2))",
        )
        .must_exist();

    assert_eq!(values[1].to_string(), "((9 10) (11 12) (1 2))");
}

#[test]
fn compiled_keyword_parameters_use_defaults_supplied_p_and_allow_other_keys() {
    let values = Runtime::new()
        .eval_compiled_source(
            "(defun describe (required
                              &optional (optional (+ required 1) optional-p)
                              &rest rest
                              &key (first (+ required optional) first-p)
                                   ((:second second-value) (+ first 1) second-p)
                              &allow-other-keys
                              &aux (total (+ first second-value)))
             (list required optional optional-p first first-p second-value second-p rest total))
             (list (describe 4 :second 20)
                   (describe 4 7 :first 30 :other 99))",
        )
        .must_exist();

    assert_eq!(
        values[1].to_string(),
        "((4 5 NIL 9 NIL 20 T (:SECOND 20) 29) (4 7 T 30 T 31 NIL (:FIRST 30 :OTHER 99) 61))"
    );
}

#[test]
fn compiled_keyword_parameters_honor_dynamic_allow_other_keys() {
    let values = Runtime::new()
        .eval_compiled_source(
            "(defun read-value (&key value) value)
             (read-value :allow-other-keys t :ignored 2 :value 3)",
        )
        .must_exist();

    assert_eq!(values[1].to_string(), "3");
}

#[test]
fn compiled_keyword_parameters_reject_unknown_and_malformed_arguments() {
    let unknown = Runtime::new()
        .eval_compiled_source("(defun read-value (&key value) value) (read-value :ignored 2)")
        .must_fail();
    assert!(matches!(
        unknown,
        RuntimeError::InvalidForm { message, .. } if message.contains("unknown keyword")
    ));

    let malformed = Runtime::new()
        .eval_compiled_source("(defun read-value (&key value) value) (read-value 'value 2)")
        .must_fail();
    assert!(matches!(
        malformed,
        RuntimeError::InvalidForm { message, .. } if message.contains("keyword")
    ));
}

#[test]
fn compiled_expands_macros_in_auxiliary_initializers() {
    assert_eq!(
        evaluate(
            "(progn
               (defmacro twice (value) `(+ ,value ,value))
               (defun doubled (value &aux (result (twice value))) result)
               (doubled 4))",
        )
        .to_string(),
        "8"
    );
}

#[test]
fn compiled_macro_lambda_lists_bind_optional_keywords_and_auxiliary_parameters() {
    let values = Runtime::new()
        .eval_compiled_source(
            "(defmacro describe (value
                                  &optional (label 'default label-p)
                                  &key (prefix 'prefix prefix-p)
                                  &allow-other-keys
                                  &aux (tag 'tag))
               `(list ,value (quote ,label) ,label-p
                       (quote ,prefix) ,prefix-p (quote ,tag)))
             (list (describe 7 :prefix 9 :ignored 1)
                   (describe 7 label :prefix 9))",
        )
        .must_exist();

    assert_eq!(
        values[1].to_string(),
        "((7 DEFAULT NIL 9 T TAG) (7 LABEL T 9 T TAG))"
    );
}

#[test]
fn compiled_macro_lambda_lists_bind_expansion_environment() {
    let values = Runtime::new()
        .eval_compiled_source(
            "(defmacro environment-present (&environment environment)
               (if (typep environment 'environment) '(quote t) '(quote nil)))
             (environment-present)",
        )
        .must_exist();
    assert_eq!(values[1].to_string(), "T");
}

#[test]
fn compiled_rejects_malformed_macro_lambda_list_sections() {
    for source in [
        "(defmacro bad (&whole) '(x))",
        "(defmacro bad (&environment) '(x))",
        "(defmacro bad (&optional x &optional y) '(x))",
        "(defmacro bad (&rest x y) '(x))",
        "(defmacro bad (&allow-other-keys) '(x))",
        "(defmacro bad (&key x &allow-other-keys y) '(x))",
        "(defmacro bad (&aux x &aux y) '(x))",
        "(defmacro bad (&unknown x) '(x))",
        "(defmacro bad (x x) '(x))",
        "(defmacro bad (&optional (x 1 2 3)) '(x))",
        "(defmacro bad (&key (:x)) '(x))",
    ] {
        assert!(
            Runtime::new().eval_compiled_source(source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn compiled_optional_parameters_report_missing_and_extra_arguments() {
    let missing = Runtime::new()
        .eval_compiled_source(
            "(defun bounded (required &optional optional) optional)
             (bounded)",
        )
        .must_fail();
    assert!(matches!(
        missing,
        RuntimeError::Arity {
            expected,
            actual: 0,
            ..
        } if expected == "at least 1"
    ));

    let extra = Runtime::new()
        .eval_compiled_source(
            "(defun bounded (required &optional optional) optional)
             (bounded 1 2 3)",
        )
        .must_fail();
    assert!(matches!(
        extra,
        RuntimeError::Arity {
            expected,
            actual: 3,
            ..
        } if expected == "at most 2"
    ));
}

#[test]
fn compiled_rejects_malformed_ordinary_lambda_parameters() {
    for source in [
        "(lambda (x x) x)",
        "(lambda (x X) x)",
        "(lambda (x &rest) x)",
        "(lambda (x &rest rest extra) x)",
        "(lambda (x &rest 1) x)",
        "(defun bad (x X) x)",
    ] {
        let error = Runtime::new().eval_compiled_source(source).must_fail();

        assert!(
            matches!(error, RuntimeError::Compile(_)),
            "{source}: {error:?}"
        );
    }
}

#[test]
fn compiled_expands_user_macros_across_forms() {
    let values = Runtime::new()
        .eval_compiled_source("(defmacro twice (x) `(+ ,x ,x)) (twice 4)")
        .must_exist();

    assert_eq!(values[0].to_string(), "TWICE");
    assert_eq!(values[1].to_string(), "8");
}

#[test]
fn compiled_macrolet_uses_local_shadowing_and_macroexpand() {
    assert_eq!(
        evaluate(
            "(progn
               (defmacro twice (value) `(+ ,value ,value))
               (list
                 (macrolet ((twice (value) `(* ,value ,value)))
                   (list (twice 3) (macroexpand-1 '(twice 4))))
                 (twice 3)))",
        )
        .to_string(),
        "((9 (* 4 4)) 6)"
    );
}

#[test]
fn compiled_symbol_macrolet_with_lexical_shadowing_and_places() {
    assert_eq!(
        evaluate(
            "(let ((cell (list 1)))
               (symbol-macrolet ((answer (+ 20 22))
                                 (item (car cell)))
                 (list answer
                       (let ((answer 7)) answer)
                       ((lambda (answer) answer) 9)
                       (progn (setq item 5) cell)
                       (progn (psetq item 6) cell))))",
        )
        .to_string(),
        "(42 7 9 (5) (6))"
    );
}

#[test]
fn compiled_symbol_macrolet_with_multiple_value_setq() {
    assert_eq!(
        evaluate(
            "(let ((cell (list 0)))
               (symbol-macrolet ((item (car cell)))
                 (progn
                   (multiple-value-setq (item) (values 7 8))
                   cell)))",
        )
        .to_string(),
        "(7)"
    );
}

#[test]
fn compiled_expands_macros_defined_in_same_progn() {
    assert_eq!(
        evaluate("(progn (defmacro twice (x) `(+ ,x ,x)) (twice 4))").to_string(),
        "8"
    );
}

#[test]
fn compiled_expands_macros_inside_functions_branches_and_bindings() {
    assert_eq!(
        evaluate(
            "(progn
               (defmacro twice (x) `(+ ,x ,x))
               (defun twice-value (x) (twice x))
               (let ((value (twice 2)))
                 (if t (twice-value value) 0)))",
        )
        .to_string(),
        "8"
    );
}

#[test]
fn compiled_expands_macros_inside_direct_lambda_callees() {
    assert_eq!(
        evaluate(
            "(progn
               (defmacro twice (x) `(+ ,x ,x))
               ((lambda (x) (twice x)) 3))",
        )
        .to_string(),
        "6"
    );
}

#[test]
fn compiled_macroexpand_1_returns_expanded_form() {
    assert_eq!(
        evaluate("(progn (defmacro twice (x) `(+ ,x ,x)) (macroexpand-1 '(twice 4)))").to_string(),
        "(+ 4 4)"
    );
}

#[test]
fn compiled_macroexpand_accepts_an_explicit_environment() {
    assert_eq!(
        evaluate(
            "(progn
               (defmacro expand-one-with-environment (form &environment environment)
                 (macroexpand-1 form environment))
               (defmacro expand-all-with-environment (form &environment environment)
                 (macroexpand form environment))
               (macrolet ((local () '(quote local)))
                 (list
                   (expand-one-with-environment '(local))
                   (expand-all-with-environment '(local)))))",
        )
        .to_string(),
        "((LOCAL) (LOCAL))"
    );
}

#[test]
fn compiled_macroexpand_expands_repeatedly() {
    assert_eq!(
        evaluate(
            "(progn
               (defmacro twice (x) \u{60}(+ ,x ,x))
               (defmacro wrapper (x) \u{60}(twice ,x))
               (macroexpand '(wrapper 3)))",
        )
        .to_string(),
        "(+ 3 3)"
    );
}

#[test]
fn compiled_macroexpand_reports_whether_a_form_was_expanded() {
    assert_eq!(
        evaluate(
            "(progn
               (defmacro twice (x) \u{60}(+ ,x ,x))
               (defmacro wrapper (x) \u{60}(twice ,x))
               (list
                 (multiple-value-bind (expanded expanded-p)
                     (macroexpand-1 '(twice 4))
                   (list expanded expanded-p))
                 (multiple-value-bind (expanded expanded-p)
                     (macroexpand-1 '(+ 1 2))
                   (list expanded expanded-p))
                 (multiple-value-bind (expanded expanded-p)
                     (macroexpand '(wrapper 3))
                   (list expanded expanded-p))))",
        )
        .to_string(),
        "(((+ 4 4) T) ((+ 1 2) NIL) ((+ 3 3) T))"
    );
}

#[test]
fn compiled_macro_expansion_limit_is_reported() {
    let error = Runtime::new()
        .eval_compiled_source("(defmacro loop (x) '(loop x)) (loop 1)")
        .must_fail();

    assert!(matches!(error, RuntimeError::InvalidForm { .. }));
}

#[test]
fn compiled_reports_compile_errors() {
    let error = Runtime::new()
        .eval_compiled_source("(if t 1 2 3)")
        .must_fail();

    assert!(matches!(error, RuntimeError::Compile(_)));
}

use super::*;
