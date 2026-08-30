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
fn evaluates_escaped_local_macro_symbol_macro_and_define_symbol_macro_names() {
    assert_eq!(evaluate("(macrolet ((|m| () 5)) (|m|))").to_string(), "5");
    assert_eq!(
        evaluate("(symbol-macrolet ((|s| 42)) |s|)").to_string(),
        "42"
    );
    assert_eq!(
        evaluate("(progn (define-symbol-macro |q| 7) |q|)").to_string(),
        "7"
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
fn arithmetic_promotes_overflow_to_a_bignum_and_comparisons_require_an_argument() {
    // FR-017: exact arithmetic that overflows i64 promotes to an
    // arbitrary-precision integer instead of erroring.
    assert_eq!(
        evaluate("(+ 9223372036854775807 1)").to_string(),
        "9223372036854775808"
    );
    assert_eq!(
        evaluate("(* (expt 2 64) (expt 2 64))").to_string(),
        "340282366920938463463374607431768211456"
    );

    // A bignum-denominator ratio is still out of scope: this codebase's
    // Rational only stores i64 numerator/denominator, so an uneven bignum
    // division still reports NumericOverflow rather than a wrong answer.
    let uneven_bignum_ratio = Runtime::new().eval_source("(/ (expt 2 100) 3)").must_fail();
    assert!(matches!(
        uneven_bignum_ratio,
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
fn bignum_arithmetic_demotes_back_to_a_fixnum_when_the_result_fits() {
    // FR-017: promotion to Value::BigInteger must not be one-directional --
    // an operation on two bignums whose exact result fits back in i64 has to
    // demote to Value::Integer again, not stay a bignum representation of a
    // small value.
    assert_eq!(
        evaluate("(- (* (expt 2 64) (expt 2 64)) (- (* (expt 2 64) (expt 2 64)) 5))").to_string(),
        "5"
    );
    assert_eq!(
        evaluate("(+ (expt 2 100) (- (expt 2 100)))").to_string(),
        "0"
    );

    // A demoted result must be typep FIXNUM, not BIGNUM: demotion has to
    // actually change representation, not just print the small value.
    assert_eq!(
        evaluate(
            "(list (typep (+ (expt 2 100) (- (expt 2 100))) 'fixnum) (typep (+ (expt 2 100) (- (expt 2 100))) 'bignum))"
        )
        .to_string(),
        "(T NIL)"
    );
}

#[test]
fn typep_distinguishes_fixnum_from_bignum() {
    // FR-017: FIXNUM and BIGNUM used to alias to the same i64-only check.
    // A machine-width integer must be a FIXNUM and not a BIGNUM, and an
    // overflow-promoted integer must be the reverse.
    assert_eq!(
        evaluate("(list (typep 5 'fixnum) (typep 5 'bignum))").to_string(),
        "(T NIL)"
    );
    assert_eq!(
        evaluate("(list (typep (expt 2 100) 'fixnum) (typep (expt 2 100) 'bignum))").to_string(),
        "(NIL T)"
    );
}

#[test]
fn bignum_supports_a_known_factorial_value_and_numeric_predicates() {
    // FR-017: a factorial pinned against its known correct value, not just
    // "some bignum came out" -- and the numeric functions the commit claims
    // were "manually verified": sqrt, abs, signum, numerator, denominator.
    let factorial_source = "\
(defun fact (n) (if (<= n 1) 1 (* n (fact (- n 1)))))
(fact 30)";
    assert_eq!(
        evaluate(factorial_source).to_string(),
        "265252859812191058636308480000000"
    );

    assert_eq!(
        evaluate("(list (abs (- (expt 2 100))) (signum (- (expt 2 100))) (signum (expt 2 100)))")
            .to_string(),
        "(1267650600228229401496703205376 -1 1)"
    );
    assert_eq!(
        evaluate("(list (numerator (expt 2 100)) (denominator (expt 2 100)))").to_string(),
        "(1267650600228229401496703205376 1)"
    );
    assert_eq!(
        evaluate("(integerp (expt 2 100))").to_string(),
        "T",
        "integerp on a bignum used to return NIL from a non-exhaustive matches! call"
    );

    // (expt 2 100) is a perfect square (2^50)^2 == 2^100, so an exact
    // bignum sqrt must both compute the correct root and demote it, since
    // 2^50 fits back in i64.
    assert_eq!(
        evaluate("(sqrt (expt 2 100))").to_string(),
        "1125899906842624"
    );
    assert_eq!(
        evaluate("(typep (sqrt (expt 2 100)) 'fixnum)").to_string(),
        "T"
    );
}

#[test]
fn bignum_vs_rational_comparisons_compute_a_real_answer_instead_of_reporting_equal() {
    // Regression: compare_number_values's bignum branch used to fall back to
    // Ordering::Equal for any operand it couldn't convert via as_big, which
    // silently included every non-integer-valued Rational (not just Float).
    // (expt 10 30) is astronomically larger than 1/2 in both directions.
    assert_eq!(evaluate("(= (expt 10 30) 1/2)").to_string(), "NIL");
    assert_eq!(evaluate("(< 1/2 (expt 10 30))").to_string(), "T");
    assert_eq!(evaluate("(> (expt 10 30) 1/2)").to_string(), "T");
    assert_eq!(evaluate("(equalp (expt 10 30) 1/2)").to_string(), "NIL");

    // min/max order-dependence was exactly how this bug hid: replacement
    // only happens on Less/Greater, so returning Equal for every comparison
    // meant the *first* argument always "won" regardless of its real value.
    assert_eq!(evaluate("(min (expt 10 30) 1/2)").to_string(), "1/2");
    assert_eq!(
        evaluate("(max 1/2 (expt 10 30))").to_string(),
        "1000000000000000000000000000000"
    );
}

#[test]
fn expt_with_an_astronomically_large_exponent_reports_overflow_instead_of_running_unbounded() {
    // Regression: ibig_power had no bound, so (expt 2 1000000000) ran with
    // ever-growing memory and never completed (verified separately: RSS
    // climbing indefinitely, no completion after 16s, had to be
    // force-killed). It must now return a typed error promptly instead.
    let overflow = Runtime::new()
        .eval_source("(expt 2 1000000000)")
        .must_fail();
    assert!(matches!(
        overflow,
        ncl_runtime::RuntimeError::NumericOverflow
    ));
}

#[test]
fn bignum_vs_rational_comparisons_are_order_independent_and_handle_negatives() {
    // The astronomically-large-exponent test above only exercises
    // min(bignum, rational) and max(rational, bignum). The pre-fix bug's
    // Equal fallback kept whichever argument came *first*, so it happened
    // to produce the right answer by luck when the smaller value was
    // already first -- min(rational, bignum) and max(bignum, rational) are
    // exactly that lucky ordering and would not have caught the original
    // bug. They still guard against a fix that is asymmetric in argument
    // order (e.g. a cross-multiplication that swaps a numerator/denominator
    // pair on only one side).
    assert_eq!(evaluate("(min 1/2 (expt 10 30))").to_string(), "1/2");
    assert_eq!(
        evaluate("(max (expt 10 30) 1/2)").to_string(),
        "1000000000000000000000000000000"
    );

    // Negative bignum vs. negative rational: the fix cross-multiplies
    // numerator/denominator pairs, which is a place a sign handling bug
    // could hide even though the positive-vs-positive cases above pass.
    assert_eq!(evaluate("(< (- (expt 10 30)) -1/2)").to_string(), "T");
    assert_eq!(evaluate("(> (- (expt 10 30)) -1/2)").to_string(), "NIL");
    assert_eq!(
        evaluate("(min (- (expt 10 30)) -1/2)").to_string(),
        "-1000000000000000000000000000000"
    );
    assert_eq!(evaluate("(max -1/2 (- (expt 10 30)))").to_string(), "-1/2");
}

#[test]
fn expt_cap_boundary_is_exact_not_merely_far_from_the_limit() {
    // The astronomically-large-exponent test above uses an exponent so
    // large the cap trips after a handful of squaring steps -- it would
    // not catch an off-by-one in the `> MAX_EXACT_BIGNUM_DIGITS` check.
    // 10^99999 is exactly 100_000 decimal digits (a 1 followed by 99999
    // zeros): right at the documented cap, so it must still succeed.
    assert_eq!(
        evaluate("(integerp (expt 10 99999))").to_string(),
        "T",
        "a result exactly at the 100,000-digit cap must not be rejected"
    );
    // 10^100000 is exactly 100_001 digits: one over the cap, so it must be
    // rejected. Together these two pin the boundary exactly rather than
    // merely confirming that *some* sufficiently large exponent overflows.
    let overflow = Runtime::new().eval_source("(expt 10 100000)").must_fail();
    assert!(matches!(
        overflow,
        ncl_runtime::RuntimeError::NumericOverflow
    ));
}

#[test]
fn repeated_bignum_multiplication_reports_overflow_instead_of_running_unbounded() {
    // Regression: expt's digit cap does not protect ordinary +/-/* on
    // bignums, since they go through a separate function (exact_binary_big)
    // that originally had no cap at all. Verified separately: 10 squarings
    // starting from a ~90,000-digit value took 51.55s and 143MB, still
    // growing -- reachable from a trivial loop over ordinary arithmetic,
    // not just from `expt` with an absurd exponent.
    let overflow = Runtime::new()
        .eval_source("(let ((x (expt 2 300000))) (dotimes (i 20) (setf x (* x x))) x)")
        .must_fail();
    assert!(matches!(
        overflow,
        ncl_runtime::RuntimeError::NumericOverflow
    ));
}

#[test]
fn multiplication_cap_boundary_is_exact_for_a_non_round_leading_digit() {
    // Regression: exceeds_exact_bignum_digit_cap's bit-length-based estimate
    // overestimates the true decimal digit count by up to 1 for any value
    // that isn't a "round" power of 10 -- 9 * 10^99999 has exactly the same
    // 100,000 digit count as 10^99999 (the existing expt boundary test's
    // value), but a larger bit length, so a naive `estimate >
    // MAX_EXACT_BIGNUM_DIGITS` check spuriously rejected it even though it's
    // legitimately at the cap, not over it. This exercises that boundary
    // through `*` specifically (not `expt`), with a leading digit that
    // defeats the "always starts with 1" blind spot the expt test alone has.
    assert_eq!(
        evaluate("(integerp (* 9 (expt 10 99999)))").to_string(),
        "T",
        "9 * 10^99999 has exactly 100,000 digits and must not be rejected"
    );
    let overflow = Runtime::new()
        .eval_source("(* (expt 10 50000) (expt 10 50000))")
        .must_fail();
    assert!(matches!(
        overflow,
        ncl_runtime::RuntimeError::NumericOverflow
    ));
}

#[test]
fn multiplication_cap_boundary_is_exact_for_a_negative_value() {
    // Regression: the exact fallback in exceeds_exact_bignum_digit_cap used
    // to check `value.to_string().len()` -- for a negative value this
    // string includes a leading '-', inflating the length by 1 and
    // spuriously rejecting a legitimate negative result whose true digit
    // count is exactly the cap, even though the equal-magnitude positive
    // value was correctly accepted.
    assert_eq!(
        evaluate("(integerp (* -9 (expt 10 99999)))").to_string(),
        "T",
        "-9 * 10^99999 has exactly 100,000 digits (the sign doesn't count as a digit) and must not be rejected"
    );
}

#[test]
fn multiplication_cap_boundary_is_exact_for_a_negative_round_power_of_ten() {
    // Regression: -9 * 10^99999 above has a non-round leading digit, whose
    // bit-length estimate lands at exactly MAX_EXACT_BIGNUM_DIGITS + 1 --
    // the sole value ambiguous under both the old and the tightened margin,
    // so it never exercised the tightened fast-accept arm
    // (`approx_digits <= MAX_EXACT_BIGNUM_DIGITS`) at all. -1 * 10^99999 is
    // a round power of ten whose estimate lands at exactly
    // MAX_EXACT_BIGNUM_DIGITS (not +1): before the margin was tightened,
    // this fell inside the wider ambiguous band (`approx_digits + 1 <
    // MAX_EXACT_BIGNUM_DIGITS` failed) and so still hit the buggy signed
    // `.to_string()` fallback, spuriously rejecting it; the tightened
    // margin now fast-accepts it directly, without ever reaching the exact
    // fallback. Verified this genuinely discriminates the fix: fails
    // against the pre-fix commit (72c87a3) and passes against the fixed
    // one (2fcab1d).
    assert_eq!(
        evaluate("(integerp (* -1 (expt 10 99999)))").to_string(),
        "T",
        "-1 * 10^99999 has exactly 100,000 digits and must not be rejected"
    );
}

#[test]
fn abs_and_negate_promote_i64_min_instead_of_overflowing() {
    // Regression: i64::MIN is the one integer whose absolute value/negation
    // doesn't fit back in i64 (i64::MAX == 9223372036854775807, but
    // |i64::MIN| == 9223372036854775808). abs/unary `-` used to report
    // NumericOverflow for exactly this value instead of promoting, breaking
    // the documented "exact arithmetic promotes on i64 overflow" guarantee.
    // Both engines share absolute/negate_number, so both are exercised here
    // rather than just the interpreter.
    for evaluator in [Runtime::eval_source, Runtime::eval_compiled_source] {
        assert_evaluates_to(
            evaluator,
            "(list (abs -9223372036854775808) (- -9223372036854775808) (typep (abs -9223372036854775808) 'bignum))",
            "(9223372036854775808 9223372036854775808 T)",
        );
    }
}

#[test]
fn negate_of_the_promoted_i64_min_bignum_demotes_back_to_a_fixnum() {
    // End-to-end sanity check for the same fix that
    // negate_number_demotes_a_bignum_that_fits_back_into_i64 (in
    // numbers/arithmetic.rs) actually regression-guards: this Lisp-level
    // version does NOT discriminate the bug on its own, since
    // number_to_value's Value::big_integer conversion independently
    // re-normalizes every Number before it's observable through evaluate(),
    // masking a denormalized intermediate Number::Big regardless of whether
    // negate_number's own demotion is correct.
    assert_eq!(
        evaluate("(- (abs -9223372036854775808))").to_string(),
        "-9223372036854775808"
    );
    assert_eq!(
        evaluate("(typep (- (abs -9223372036854775808)) 'fixnum)").to_string(),
        "T"
    );
}

#[test]
fn min_max_handle_ties_and_a_single_argument() {
    // extreme() was rewritten from clone-per-improvement to index-tracking.
    // A single argument exercises the loop running zero iterations
    // (values.iter().enumerate().skip(1) is empty), and a tie exercises
    // whether the index is left alone on Ordering::Equal rather than
    // drifting to the wrong element -- both are edge cases an off-by-one
    // in the index bookkeeping would plausibly get wrong while every
    // strictly-ordered multi-argument case still passes.
    assert_eq!(evaluate("(min 5)").to_string(), "5");
    assert_eq!(evaluate("(max 5)").to_string(), "5");
    assert_eq!(evaluate("(min 3 3 3)").to_string(), "3");
    assert_eq!(evaluate("(max 1 5 5 2)").to_string(), "5");
    assert_eq!(evaluate("(min 5 3 3 7)").to_string(), "3");
}

#[test]
fn reworded_exact_arithmetic_error_messages_name_bignums_not_just_floats() {
    // Regression: exact_binary's bignum branch and exact_quotient's
    // exact_parts() branch used to describe the rejected operand only as
    // "a non-exact number" / imply "a float", which was actively wrong
    // once bignums became a possible exact operand that still can't be
    // combined with a Rational or a Float here. Assert the new wording so
    // a future edit can't silently revert to the old misleading text --
    // matching only the error *variant* (as the pre-existing tests for
    // these functions do) would not catch that regression.
    let add_error = Runtime::new()
        .eval_source("(+ (expt 2 100) 1/2)")
        .must_fail();
    assert!(
        matches!(
            &add_error,
            ncl_runtime::RuntimeError::InvalidForm { message, .. }
                if message == "exact arithmetic between a bignum and a float or rational is not supported"
        ),
        "unexpected error: {add_error:?}"
    );

    let floor_error = Runtime::new()
        .eval_source("(floor (expt 2 100))")
        .must_fail();
    assert!(
        matches!(
            &floor_error,
            ncl_runtime::RuntimeError::InvalidForm { message, .. }
                if message == "exact quotient does not support a float or a bignum"
        ),
        "unexpected error: {floor_error:?}"
    );
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
fn propagates_errors_raised_while_evaluating_function_call_operands() {
    for source in [
        "(funcall (error \"boom\"))",
        "(funcall #'+ (error \"boom\"))",
        "(eval (error \"boom\"))",
        "(apply (error \"boom\") '())",
        "(apply #'+ (error \"boom\") '(1))",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn resolves_escaped_function_designators_and_reports_matching_errors() {
    let unbound = Runtime::new()
        .eval_source("(funcall '|TotallyMissingExactFn|)")
        .must_fail();
    assert!(matches!(
        unbound,
        ncl_runtime::RuntimeError::UnboundVariable { name, .. }
            if name == "TotallyMissingExactFn"
    ));

    let not_callable = Runtime::new()
        .eval_source("(progn (defvar |ExactVar| 9) (funcall '|ExactVar|))")
        .must_fail();
    assert!(matches!(
        not_callable,
        ncl_runtime::RuntimeError::NotCallable { .. }
    ));

    assert_evaluates_to(
        Runtime::eval_source,
        "(progn (defun |ExactFn| (x) (* x 2)) (funcall '|ExactFn| 5))",
        "10",
    );
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
        ("(eval :|foo|)", ":|foo|"),
        ("(let ((|foo| 42)) (eval '|foo|))", "42"),
    ];

    for (source, expected) in cases {
        assert_evaluates_to(Runtime::eval_source, source, expected);
    }

    assert!(Runtime::new().eval_source("(eval '(1 . 2))").is_err());
}

#[test]
fn eval_reconstructs_uninterned_symbols_and_rejects_unformable_values() {
    let unbound = Runtime::new()
        .eval_source(r#"(eval (make-symbol "foo"))"#)
        .must_fail();
    assert!(matches!(
        unbound,
        ncl_runtime::RuntimeError::UnboundVariable { .. }
    ));

    let unformable = Runtime::new()
        .eval_source("(eval (make-hash-table))")
        .must_fail();
    assert!(matches!(
        unformable,
        ncl_runtime::RuntimeError::Type { expected, actual, .. }
            if expected == "FORM" && actual == "HASH-TABLE"
    ));
}

#[test]
fn setq_defines_a_previously_unbound_global_variable() {
    assert_eq!(
        evaluate("(progn (setq newly-declared-global-variable 99) newly-declared-global-variable)")
            .to_string(),
        "99"
    );
}

#[test]
fn setq_updates_a_let_bound_special_variable_and_restores_it_afterwards() {
    assert_eq!(
        evaluate(
            "(progn
               (defvar *dynamic-setq-target* 1)
               (list (let ((*dynamic-setq-target* 2))
                       (setq *dynamic-setq-target* 3)
                       *dynamic-setq-target*)
                     *dynamic-setq-target*))"
        )
        .to_string(),
        "(3 1)"
    );
}

#[test]
fn setq_updates_a_let_bound_escaped_special_variable_and_restores_it_afterwards() {
    assert_eq!(
        evaluate(
            "(progn
               (defvar |EscapedDynamicSetq| 1)
               (list (let ((|EscapedDynamicSetq| 2))
                       (setq |EscapedDynamicSetq| 3)
                       |EscapedDynamicSetq|)
                     |EscapedDynamicSetq|))"
        )
        .to_string(),
        "(3 1)"
    );
}

#[test]
fn setq_declares_a_new_global_escaped_special_variable_outside_any_let() {
    assert_eq!(
        evaluate(
            "(progn
               (defvar |EscapedGlobalSetq| 1)
               (setq |EscapedGlobalSetq| 5)
               |EscapedGlobalSetq|)"
        )
        .to_string(),
        "5"
    );
}

#[test]
fn defvar_preserves_and_defparameter_replaces_values_through_the_compiled_engine() {
    let values = Runtime::new()
        .eval_compiled_source(
            "(defvar compiled-answer 1) (defvar compiled-answer 2) compiled-answer \
             (defparameter compiled-answer 3) compiled-answer",
        )
        .must_exist();

    assert_eq!(values[1].to_string(), "1");
    assert_eq!(values[2].to_string(), "1");
    assert_eq!(values[4].to_string(), "3");
}

#[test]
fn defvar_preserves_an_escaped_special_variable_through_the_compiled_engine() {
    let values = Runtime::new()
        .eval_compiled_source(
            "(defvar |CompiledEscaped| 1) (defvar |CompiledEscaped| 2) |CompiledEscaped|",
        )
        .must_exist();

    assert_eq!(values[2].to_string(), "1");
}

#[test]
fn evaluates_defconstant_and_constantp_for_escaped_symbols() {
    assert_eq!(
        evaluate(
            "(progn
               (defconstant |ExactAnswer| 42)
               (list |ExactAnswer| (constantp '|ExactAnswer|)))"
        )
        .to_string(),
        "(42 T)"
    );

    assert!(
        Runtime::new()
            .eval_source("(defconstant |ExactAnswer| 42) (setq |ExactAnswer| 7)")
            .is_err()
    );
}

#[test]
fn set_updates_a_let_shadowed_dynamic_variable_via_symbol_value() {
    assert_eq!(
        evaluate(
            "(progn
               (defvar *set-dynamic-target* 1)
               (let ((*set-dynamic-target* 2))
                 (set '*set-dynamic-target* 3)
                 *set-dynamic-target*))"
        )
        .to_string(),
        "3"
    );
}

#[test]
fn set_updates_a_let_shadowed_escaped_dynamic_variable_via_symbol_value() {
    assert_eq!(
        evaluate(
            "(progn
               (defvar |SetExactDynamicTarget| 1)
               (let ((|SetExactDynamicTarget| 2))
                 (set '|SetExactDynamicTarget| 3)
                 |SetExactDynamicTarget|))"
        )
        .to_string(),
        "3"
    );
}

#[test]
fn do_loop_supports_an_escaped_step_variable() {
    assert_eq!(
        evaluate("(do ((|i| 0 (1+ |i|))) ((>= |i| 3) |i|))").to_string(),
        "3"
    );
}

#[test]
fn makunbound_and_fmakunbound_support_escaped_symbol_names() {
    assert_eq!(
        evaluate(
            "(progn
               (defvar |MakunboundExactTarget| 1)
               (makunbound '|MakunboundExactTarget|)
               (boundp '|MakunboundExactTarget|))"
        )
        .to_string(),
        "NIL"
    );

    assert_eq!(
        evaluate(
            "(progn
               (defun |FmakunboundExactTarget| () 1)
               (fmakunbound '|FmakunboundExactTarget|)
               (fboundp '|FmakunboundExactTarget|))"
        )
        .to_string(),
        "NIL"
    );
}

#[test]
fn references_to_an_unknown_package_are_rejected() {
    let error = Runtime::new()
        .eval_source("no-such-package-in-this-test:foo")
        .must_fail();
    assert!(matches!(error, ncl_runtime::RuntimeError::Package { .. }));
}

#[test]
fn funcalling_map_into_as_a_primitive_reports_a_missing_argument() {
    let error = Runtime::new()
        .eval_source("(funcall #'map-into (list 1))")
        .must_fail();
    assert!(matches!(
        error,
        ncl_runtime::RuntimeError::Arity { function, actual: 1, .. }
            if function == "map-into"
    ));
}

#[test]
fn call_next_method_fails_when_no_further_method_is_applicable() {
    let error = Runtime::new()
        .eval_source(
            "(progn
               (defgeneric call-next-method-base-only (object))
               (defmethod call-next-method-base-only ((object t))
                 (call-next-method))
               (call-next-method-base-only 1))",
        )
        .must_fail();
    assert!(matches!(
        error,
        ncl_runtime::RuntimeError::InvalidForm { .. }
    ));
}

#[test]
fn call_next_method_accepts_explicit_override_arguments() {
    assert_eq!(
        evaluate(
            "(progn
               (defclass call-next-method-override-class () ())
               (defgeneric call-next-method-override (object))
               (defmethod call-next-method-override ((object t))
                 (list :base object))
               (defmethod call-next-method-override
                   ((object call-next-method-override-class))
                 (call-next-method 42))
               (call-next-method-override
                 (make-instance 'call-next-method-override-class)))"
        )
        .to_string(),
        "(:BASE 42)"
    );
}
