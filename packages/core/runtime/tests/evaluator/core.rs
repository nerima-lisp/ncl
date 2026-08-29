use super::*;

#[test]
fn supports_uninterned_symbols_and_gensym() {
    assert_evaluates_to(
        Runtime::eval_source,
        r#"(let ((symbol (make-symbol "foo")))
            (list (symbolp symbol)
                  (keywordp symbol)
                  (symbol-name symbol)
                  (symbol-package symbol)
                  (eq symbol symbol)
                  (eq (make-symbol "foo") (make-symbol "foo"))
                  (eq '#:foo '#:foo)))"#,
        r#"(T NIL "foo" NIL T NIL NIL)"#,
    );
    assert_evaluates_to(
        Runtime::eval_source,
        r#"(let ((symbol (gensym "TMP")))
            (list (symbolp symbol) (symbol-package symbol) (symbol-name symbol)))"#,
        r#"(T NIL "TMP0")"#,
    );
}

#[test]
fn evaluates_arithmetic_and_lexical_closures() {
    assert_eq!(evaluate("(+ 1 2 3)").to_string(), "6");
    assert_eq!(evaluate("((lambda (x) (+ x 1)) 2)").to_string(), "3");
}

#[test]
fn evaluates_flet_in_a_separate_function_namespace() {
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
fn evaluates_labels_with_mutual_recursion() {
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
fn evaluates_common_lisp_truth_and_lists() {
    assert_eq!(evaluate("(if nil 1 2)").to_string(), "2");
    assert_eq!(evaluate("(car (list 4 5))").to_string(), "4");
    assert_eq!(evaluate("(car nil)").to_string(), "NIL");
    assert_eq!(evaluate("(cdr nil)").to_string(), "NIL");
    assert_eq!(evaluate("'(a . b)").to_string(), "(A . B)");
}

#[test]
fn evaluates_quasiquote_unquote_and_splicing() {
    assert_eq!(
        evaluate("(let ((x 2) (xs '(3 4))) \u{60}(1 ,x ,@xs))").to_string(),
        "(1 2 3 4)"
    );
}

#[test]
fn expands_user_macros_before_evaluation() {
    let values = Runtime::new()
        .eval_source("(defmacro twice (x) \u{60}(+ ,x ,x)) (twice 4)")
        .must_exist();

    assert_eq!(values[1].to_string(), "8");
}

#[test]
fn evaluates_macrolet_with_local_shadowing_and_macroexpand() {
    assert_eq!(
        evaluate(
            "(progn
               (defmacro twice (value) \u{60}(+ ,value ,value))
               (list
                 (macrolet ((twice (value) \u{60}(* ,value ,value)))
                   (list (twice 3) (macroexpand-1 '(twice 4))))
                 (twice 3)))",
        )
        .to_string(),
        "((9 (* 4 4)) 6)"
    );
}

#[test]
fn evaluates_symbol_macrolet_with_lexical_shadowing_and_places() {
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
fn evaluates_symbol_macrolet_with_multiple_value_setq() {
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
fn evaluates_define_symbol_macro_and_generalized_places() {
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *symbol-macro-cell* (list 1))
               (define-symbol-macro *symbol-macro-item* (car *symbol-macro-cell*))
               (list *symbol-macro-item*
                     (progn (setq *symbol-macro-item* 7) *symbol-macro-item*)
                     *symbol-macro-cell*))",
        )
        .to_string(),
        "(1 7 (7))"
    );
}

#[test]
fn macro_rest_parameters_receive_unquoted_forms() {
    let values = Runtime::new()
        .eval_source(
            "(defmacro list* (first &rest rest) \u{60}(list ,first ,@rest)) \
             (list* 1 2 3)",
        )
        .must_exist();

    assert_eq!(values[1].to_string(), "(1 2 3)");
}

#[test]
fn macroexpand_1_returns_expanded_and_unexpanded_forms() {
    let values = Runtime::new()
        .eval_source(
            "(defmacro twice (x) \u{60}(+ ,x ,x)) \
             (macroexpand-1 '(twice 4)) (macroexpand-1 '(+ 1 2))",
        )
        .must_exist();

    assert_eq!(values[1].to_string(), "(+ 4 4)");
    assert_eq!(values[2].to_string(), "(+ 1 2)");
}

#[test]
fn macroexpand_accepts_an_explicit_environment() {
    let values = Runtime::new()
        .eval_source(
            "(defmacro expand-one-with-environment (form &environment environment)
                 (macroexpand-1 form environment))
             (defmacro expand-all-with-environment (form &environment environment)
                 (macroexpand form environment))
             (macrolet ((local () '(quote local)))
               (list
                 (expand-one-with-environment '(local))
                 (expand-all-with-environment '(local))))",
        )
        .must_exist();

    assert_eq!(values[2].to_string(), "((LOCAL) (LOCAL))");
}

#[test]
fn macroexpand_expands_repeatedly() {
    let values = Runtime::new()
        .eval_source(
            "(defmacro twice (x) \u{60}(+ ,x ,x))
             (defmacro wrapper (x) \u{60}(twice ,x))
             (macroexpand '(wrapper 3))",
        )
        .must_exist();

    assert_eq!(values[2].to_string(), "(+ 3 3)");
}

#[test]
fn macroexpand_reports_whether_a_form_was_expanded() {
    let values = Runtime::new()
        .eval_source(
            "(defmacro twice (x) \u{60}(+ ,x ,x))
             (multiple-value-bind (expanded expanded-p)
                 (macroexpand-1 '(twice 4))
               (list expanded expanded-p))
             (multiple-value-bind (expanded expanded-p)
                 (macroexpand-1 '(+ 1 2))
               (list expanded expanded-p))
             (defmacro wrapper (x) \u{60}(twice ,x))
             (multiple-value-bind (expanded expanded-p)
                 (macroexpand '(wrapper 3))
               (list expanded expanded-p))",
        )
        .must_exist();

    assert_eq!(values[1].to_string(), "((+ 4 4) T)");
    assert_eq!(values[2].to_string(), "((+ 1 2) NIL)");
    assert_eq!(values[4].to_string(), "((+ 3 3) T)");
}

#[test]
fn recursive_macro_expansion_has_a_typed_limit() {
    let error = Runtime::new()
        .eval_source("(defmacro loop (x) '(loop x)) (loop 1)")
        .must_fail();

    assert!(matches!(
        error,
        ncl_runtime::RuntimeError::InvalidForm { message, .. }
            if message.contains("macro expansion")
    ));
}

#[test]
fn macros_are_not_callable_through_normal_apply() {
    let error = Runtime::new()
        .eval_source(
            "(defmacro twice (x) \u{60}(+ ,x ,x)) \
             (funcall (function twice) 4)",
        )
        .must_fail();

    assert!(matches!(
        error,
        ncl_runtime::RuntimeError::NotCallable { value, .. }
            if value == "#<MACRO>"
    ));
}

#[test]
fn malformed_macro_parameters_are_rejected() {
    let error = Runtime::new()
        .eval_source("(defmacro bad (x &rest) '(x))")
        .must_fail();

    assert!(matches!(
        error,
        ncl_runtime::RuntimeError::InvalidForm { message, .. }
            if message.contains("&rest")
    ));
}

#[test]
fn rejects_malformed_macro_lambda_list_sections() {
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
        "(defmacro bad (&whole whole &whole other) '(x))",
        "(defmacro bad (&environment env &environment other) '(x))",
        "(defmacro bad (&rest rest &rest other) '(x))",
        "(defmacro bad (&key x &key y) '(x))",
        "(defmacro bad (&allow-other-keys) '(x))",
        "(defmacro bad (&key x &allow-other-keys y) '(x))",
        "(defmacro bad (&key x x) '(x))",
        "(defmacro bad (&key ((:x value)) '(x))",
        "(defmacro bad (&aux (x 1 2 3)) '(x))",
        "(defmacro bad ((1)) '(x))",
        "(defmacro bad 1 '(x))",
        "(defmacro bad (&key x &optional y) '(x))",
        "(defmacro bad (&key x &rest rest) '(x))",
        "(defmacro bad (&aux x &key y) '(x))",
        "(defmacro bad (&environment env &whole whole) '(x))",
        "(defmacro bad (&whole 1) '(x))",
        "(defmacro bad (&environment 1) '(x))",
        "(defmacro bad (&optional (1)) '(x))",
        "(defmacro bad (&key ((x y)) '(x))",
        "(defmacro bad (&key (:x value 1 2)) '(x))",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn macro_lambda_lists_bind_optional_keywords_and_auxiliary_parameters() {
    let values = Runtime::new()
        .eval_source(
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
fn macro_lambda_lists_cover_arity_whole_and_keyword_defaults() {
    for source in [
        "(defmacro required (value) value) (required)",
        "(defmacro bounded (value) value) (bounded 1 2)",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }

    let values = Runtime::new()
        .eval_source(
            "(defmacro capture (&whole whole &key (value 42))
               (list 'quote (list whole value)))
             (capture)
             (capture :value 7)",
        )
        .must_exist();

    assert_eq!(values[1].to_string(), "((CAPTURE) 42)");
    assert_eq!(values[2].to_string(), "((CAPTURE :VALUE 7) 7)");
}

#[test]
fn macro_keyword_arguments_validate_pairs_names_and_dynamic_allowance() {
    let values = Runtime::new()
        .eval_source(
            "(defmacro keyworded (&key value) value)
             (keyworded :allow-other-keys t :ignored 2 :value 7)",
        )
        .must_exist();
    assert_eq!(values[1].to_string(), "7");

    for source in [
        "(defmacro keyworded (&key value) value) (keyworded :ignored 2)",
        "(defmacro keyworded (&key value) value) (keyworded :value)",
        "(defmacro keyworded (&key value) value) (keyworded 'value 2)",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn macro_lambda_lists_support_body_and_destructuring() {
    let values = Runtime::new()
        .eval_source(
            "(defmacro first-of ((first . rest)) first)
             (defmacro list-body (first &body body) `(list ,first ,@body))
             (list (first-of (10 20)) (list-body 1 2 3))",
        )
        .must_exist();

    assert_eq!(values[2].to_string(), "(10 (1 2 3))");
}

#[test]
fn macro_lambda_lists_bind_expansion_environment() {
    let values = Runtime::new()
        .eval_source(
            "(defmacro environment-present (&environment environment)
               (if (typep environment 'environment) '(quote t) '(quote nil)))
             (environment-present)",
        )
        .must_exist();
    assert_eq!(values[1].to_string(), "T");
}

#[test]
fn ordinary_rest_parameters_bind_lists_and_capture_lexicals() {
    let values = Runtime::new()
        .eval_source(
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
fn ordinary_optional_parameters_use_defaults_and_supplied_p() {
    let values = Runtime::new()
        .eval_source(
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
fn ordinary_optional_parameters_evaluate_chained_defaults() {
    let values = Runtime::new()
        .eval_source(
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
fn ordinary_auxiliary_parameters_evaluate_sequentially_after_other_bindings() {
    let values = Runtime::new()
        .eval_source(
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
fn ordinary_keyword_parameters_use_defaults_supplied_p_and_allow_other_keys() {
    let values = Runtime::new()
        .eval_source(
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
fn ordinary_keyword_parameters_honor_dynamic_allow_other_keys() {
    let values = Runtime::new()
        .eval_source(
            "(defun read-value (&key value) value)
             (read-value :allow-other-keys t :ignored 2 :value 3)",
        )
        .must_exist();

    assert_eq!(values[1].to_string(), "3");
}

#[test]
fn ordinary_keyword_parameters_reject_unknown_and_malformed_arguments() {
    let unknown = Runtime::new()
        .eval_source("(defun read-value (&key value) value) (read-value :ignored 2)")
        .must_fail();
    assert!(matches!(
        unknown,
        ncl_runtime::RuntimeError::InvalidForm { message, .. }
            if message.contains("unknown keyword")
    ));

    let malformed = Runtime::new()
        .eval_source("(defun read-value (&key value) value) (read-value 'value 2)")
        .must_fail();
    assert!(matches!(
        malformed,
        ncl_runtime::RuntimeError::InvalidForm { message, .. }
            if message.contains("keyword")
    ));
}

#[test]
fn ordinary_optional_parameters_report_missing_and_extra_arguments() {
    let missing = Runtime::new()
        .eval_source(
            "(defun bounded (required &optional optional) optional)
             (bounded)",
        )
        .must_fail();
    assert!(matches!(
        missing,
        ncl_runtime::RuntimeError::Arity {
            expected,
            actual: 0,
            ..
        } if expected == "at least 1"
    ));

    let extra = Runtime::new()
        .eval_source(
            "(defun bounded (required &optional optional) optional)
             (bounded 1 2 3)",
        )
        .must_fail();
    assert!(matches!(
        extra,
        ncl_runtime::RuntimeError::Arity {
            expected,
            actual: 3,
            ..
        } if expected == "at most 2"
    ));
}

#[test]
fn malformed_ordinary_lambda_parameters_are_rejected() {
    for source in [
        "(lambda (x x) x)",
        "(lambda (x X) x)",
        "(lambda (x &rest) x)",
        "(lambda (x &rest rest extra) x)",
        "(lambda (x &rest 1) x)",
        "(defun bad (x X) x)",
    ] {
        let error = Runtime::new().eval_source(source).must_fail();

        assert!(
            matches!(
                error,
                ncl_runtime::RuntimeError::InvalidForm { .. }
                    | ncl_runtime::RuntimeError::Arity { .. }
            ),
            "{source}: {error:?}"
        );
    }
}

#[test]
fn malformed_definition_special_forms_are_rejected() {
    for source in [
        "(lambda)",
        "(function)",
        "(function foo bar)",
        "(defun)",
        "(defun (name) () nil)",
        "(defmacro)",
        "(defmacro (name) () nil)",
        "(defsetf)",
        "(defsetf 1 2)",
        "(define-setf-expander)",
        "(define-setf-expander (name) () nil)",
        "(define-modify-macro)",
        "(define-modify-macro (name) () +)",
        "(get-setf-expansion)",
        "(get-setf-expansion 1 2 3)",
        "(macroexpand-1)",
        "(macroexpand-1 '(quote value) 1 2)",
        "(macroexpand)",
        "(macroexpand '(quote value) 1 2)",
        "(macroexpand '(quote value) 1)",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn definitions_are_visible_to_later_forms() {
    let values = Runtime::new()
        .eval_source("(define answer 41) (+ answer 1)")
        .must_exist();

    assert_eq!(values[1].to_string(), "42");
}

#[test]
fn errors_are_typed() {
    let error = Runtime::new().eval_source("(+ 1 nil)").must_fail();

    assert!(matches!(error, ncl_runtime::RuntimeError::Type { .. }));
}

#[test]
fn predicates_and_equality_match_lisp_basics() {
    assert_eq!(evaluate("(listp nil)").to_string(), "T");
    assert_eq!(evaluate("(listp '(a b))").to_string(), "T");
    assert_eq!(evaluate("(listp '(a . b))").to_string(), "NIL");
    assert_eq!(evaluate("(consp nil)").to_string(), "NIL");
    assert_eq!(evaluate("(eq nil (null 1))").to_string(), "T");
    assert_eq!(evaluate("(eq 'foo 'foo)").to_string(), "T");
    assert_eq!(evaluate("(equal \"x\" \"x\")").to_string(), "T");
    assert_eq!(evaluate("(cond ((= 1 1)))").to_string(), "T");
}

#[test]
fn evaluates_short_circuit_conditionals_from_table_cases() {
    let cases = [
        ("(and)", "T"),
        ("(and 1 2)", "2"),
        ("(and nil (/ 1 0))", "NIL"),
        ("(or)", "NIL"),
        ("(or nil 2)", "2"),
        ("(or 1 (/ 1 0))", "1"),
        ("(when t 1 2)", "2"),
        ("(when nil (/ 1 0))", "NIL"),
        ("(unless nil 1 2)", "2"),
        ("(unless t (/ 1 0))", "NIL"),
        ("(cond (nil 1) (t 2))", "2"),
        ("(cond (t))", "T"),
        ("(cond (nil 1))", "NIL"),
    ];

    assert_value_cases(evaluate, &cases);
}

#[test]
fn rejects_malformed_conditional_forms_from_table_cases() {
    let cases = [
        "(cond 1)",
        "(cond ())",
        "(case 1 2)",
        "(case 1 ())",
        "(typecase 1 2)",
        "(typecase 1 ())",
    ];

    for source in cases {
        assert!(
            Runtime::new().eval_source(source).is_err(),
            "expected failure for {source}"
        );
    }
}

#[test]
fn evaluates_case_and_ecase_with_eql_keys() {
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
        .eval_source("(ecase 9 ((1) :one))")
        .must_fail();
    assert!(matches!(
        error,
        ncl_runtime::RuntimeError::InvalidForm { message, .. }
            if message == "ecase fell through"
    ));
}

#[test]
fn evaluates_typecase_and_etypecase_with_typep() {
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
        .eval_source("(etypecase \"text\" (integer :integer))")
        .must_fail();
    assert!(matches!(
        error,
        ncl_runtime::RuntimeError::InvalidForm { message, .. }
            if message == "etypecase fell through"
    ));
}

#[test]
fn defvar_preserves_and_defparameter_replaces_values() {
    let values = Runtime::new()
        .eval_source(
            "(defvar answer 1) (defvar answer 2) answer \
             (defparameter answer 3) answer",
        )
        .must_exist();

    assert_eq!(values[1].to_string(), "1");
    assert_eq!(values[2].to_string(), "1");
    assert_eq!(values[4].to_string(), "3");
}

#[test]
fn definition_forms_reject_malformed_names_and_arities() {
    let invalid_forms = [
        "(defvar)",
        "(defvar answer 1 2)",
        "(defvar 42 1)",
        "(defparameter)",
        "(defconstant)",
        "(defconstant answer)",
        "(defconstant 42 1)",
    ];

    for source in invalid_forms {
        assert!(
            Runtime::new().eval_source(source).is_err(),
            "expected malformed definition to fail: {source}"
        );
    }
}

#[test]
fn evaluates_defconstant_and_constantp() {
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
            .eval_source("(defconstant +answer+ 42) (setq +answer+ 7)")
            .is_err()
    );
    assert!(
        Runtime::new()
            .eval_source("(defconstant +answer+ 42) (setf (symbol-value '+answer+) 7)")
            .is_err()
    );
    assert!(
        Runtime::new()
            .eval_source("(defconstant +answer+ 42) (psetq +answer+ 7)")
            .is_err()
    );
}

#[test]
fn arithmetic_reports_overflow_and_comparisons_require_an_argument() {
    let overflow = Runtime::new()
        .eval_source("(+ 9223372036854775807 1)")
        .must_fail();
    assert!(matches!(
        overflow,
        ncl_runtime::RuntimeError::NumericOverflow
    ));

    let comparison_error = Runtime::new().eval_source("(=)").must_fail();
    assert!(matches!(
        comparison_error,
        ncl_runtime::RuntimeError::Arity {
            function,
            actual: 0,
            ..
        } if function == "="
    ));
}

#[test]
fn evaluates_common_sequence_and_conversion_builtins_in_both_engines() {
    for evaluator in [Runtime::eval_source, Runtime::eval_compiled_source] {
        assert_evaluates_to(evaluator, "(append '(1 2) '(3 4))", "(1 2 3 4)");
        assert_evaluates_to(evaluator, "(reverse '(1 2 3))", "(3 2 1)");
        assert_evaluates_to(evaluator, "(copy-tree '((1) (2 3)))", "((1) (2 3))");
        assert_evaluates_to(
            evaluator,
            "(list (coerce '(1 2) 'vector) (coerce #(1 2) 'list))",
            "(#(1 2) (1 2))",
        );
    }
}

#[test]
fn evaluates_list_construction_builtins_in_both_engines() {
    for evaluator in [Runtime::eval_source, Runtime::eval_compiled_source] {
        assert_evaluates_to(evaluator, "(list* 1 2 '(3 4))", "(1 2 3 4)");
        assert_evaluates_to(evaluator, "(make-list 3 :initial-element 'x)", "(X X X)");
        assert_evaluates_to(evaluator, "(nthcdr 2 '(a b c))", "(C)");
        assert_evaluates_to(evaluator, "(acons 'a 1 '((b . 2)))", "((A . 1) (B . 2))");
        assert_evaluates_to(evaluator, "(pairlis '(a b) '(1 2))", "((B . 2) (A . 1))");
        assert_evaluates_to(evaluator, "(list (car nil) (cdr nil))", "(NIL NIL)");
    }
}

#[test]
fn validates_function_designators_and_argument_lists_from_table_cases() {
    let cases = [
        ("(funcall)", "funcall arity"),
        ("(funcall 1)", "funcall designator"),
        ("(eval)", "eval arity"),
        ("(eval 1 2)", "eval arity"),
        ("(apply)", "apply arity"),
        ("(apply #'list 1 2)", "apply final list"),
    ];

    for (source, case_name) in cases {
        assert!(
            Runtime::new().eval_source(source).is_err(),
            "{case_name}: {source}"
        );
    }

    assert_evaluates_to(Runtime::eval_source, "(apply #'list 1 '(2 3))", "(1 2 3)");
    assert_evaluates_to(Runtime::eval_source, "(eval '(+ 2 3))", "5");
}

#[test]
fn eval_reconstructs_supported_literal_form_shapes() {
    let cases = [
        ("(eval 42)", "42"),
        ("(eval 3/6)", "1/2"),
        ("(eval 1.5)", "1.5"),
        ("(eval :name)", ":NAME"),
        ("(eval \"text\")", "\"text\""),
        ("(eval #\\A)", "#\\A"),
        ("(eval '#(1 2))", "#(1 2)"),
        ("(eval (find-package \"COMMON-LISP-USER\"))", "NIL"),
    ];

    for (source, expected) in cases {
        assert_evaluates_to(Runtime::eval_source, source, expected);
    }

    assert!(Runtime::new().eval_source("(eval '(1 . 2))").is_err());
}
