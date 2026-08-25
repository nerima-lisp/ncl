use ncl_runtime::{Runtime, RuntimeError, Value};

fn evaluate(source: &str) -> Value {
    Runtime::new().eval_source(source).unwrap().pop().unwrap()
}

#[test]
fn reader_features_are_empty_by_default_interpreted() {
    let values = Runtime::new()
        .eval_source("#+enabled 3 #-enabled 4")
        .unwrap();

    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "4");
}

#[test]
fn reader_features_control_interpreted_source() {
    let values = Runtime::new()
        .with_reader_features(["enabled"])
        .eval_source("#+enabled (+ 1 2) #-enabled (+ 4 5)")
        .unwrap();

    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "3");
}

#[test]
fn reader_features_control_interpreted_read_from_string() {
    let values = Runtime::new()
        .with_reader_features(["enabled"])
        .eval_source(
            r##"(car (multiple-value-list
                       (read-from-string "#+enabled 3 #-enabled 4")))"##,
        )
        .unwrap();

    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "3");
}

#[test]
fn reader_features_control_interpreted_stream_read() {
    let values = Runtime::new()
        .with_reader_features(["enabled"])
        .eval_source(r##"(read (make-string-input-stream "#+enabled 5 #-enabled 6"))"##)
        .unwrap();

    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "5");
}

#[test]
fn reader_features_control_interpreted_preserving_stream_read() {
    let values = Runtime::new()
        .with_reader_features(["enabled"])
        .eval_source(
            r##"(read-preserving-whitespace
                   (make-string-input-stream "#+enabled 7 #-enabled 8"))"##,
        )
        .unwrap();

    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "7");
}

#[test]
fn reader_features_are_visible_as_interpreted_special_variable() {
    let values = Runtime::new()
        .with_reader_features(["enabled"])
        .eval_source("(list *features* (member :enabled *features*))")
        .unwrap();

    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "((:ENABLED) (:ENABLED))");
}

#[test]
fn dynamically_bound_interpreted_features_control_reader() {
    let values = Runtime::new()
        .eval_source(
            r##"(let ((*features* '(:enabled)))
                   (list
                     (car (multiple-value-list
                            (read-from-string "#+enabled 3 #-enabled 4")))
                     (read (make-string-input-stream "#+enabled 5 #-enabled 6"))
                     (read-preserving-whitespace
                       (make-string-input-stream "#+enabled 7 #-enabled 8"))))"##,
        )
        .unwrap();

    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(3 5 7)");
}

#[test]
fn setq_interpreted_features_control_reader() {
    let values = Runtime::new()
        .eval_source(
            r##"(progn
                   (setq *features* '(:enabled))
                   (car (multiple-value-list
                          (read-from-string "#+enabled 3 #-enabled 4"))))"##,
        )
        .unwrap();

    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "3");
}

#[test]
fn supports_uninterned_symbols_and_gensym() {
    assert_eq!(
        evaluate(
            r#"(let ((symbol (make-symbol "foo")))
                (list (symbolp symbol)
                      (keywordp symbol)
                      (symbol-name symbol)
                      (symbol-package symbol)
                      (eq symbol symbol)
                      (eq (make-symbol "foo") (make-symbol "foo"))
                      (eq '#:foo '#:foo)))"#,
        )
        .to_string(),
        r#"(T NIL "foo" NIL T NIL NIL)"#,
    );
    assert_eq!(
        evaluate(
            r#"(let ((symbol (gensym "TMP")))
                (list (symbolp symbol) (symbol-package symbol) (symbol-name symbol)))"#,
        )
        .to_string(),
        r#"(T NIL "TMP0")"#,
    );
}

#[test]
fn preserves_escaped_symbol_identity_across_namespaces() {
    assert_eq!(
        evaluate(
            r#"(let ((foo 1) (|foo| 2))
                (setq |foo| 3)
                (list foo |foo|))"#,
        )
        .to_string(),
        "(1 3)",
    );
    assert_eq!(
        evaluate(
            r#"(flet ((foo () 1) (|foo| () 2))
                (list (foo)
                      (|foo|)
                      (funcall (function foo))
                      (funcall (function |foo|))))"#,
        )
        .to_string(),
        "(1 2 1 2)",
    );
    assert_eq!(
        evaluate(
            r#"(multiple-value-bind (foo |foo|) (values 1 2)
                (list foo |foo|))"#,
        )
        .to_string(),
        "(1 2)",
    );
    assert_eq!(
        evaluate(
            r#"(let ((foo 1) (|foo| 2))
                (setf |foo| 3)
                (list foo |foo|))"#,
        )
        .to_string(),
        "(1 3)",
    );
    assert_eq!(
        evaluate(r#"(list (symbol-name :|foo|) (symbol-name :FOO) (eq :|foo| :FOO))"#).to_string(),
        r#"("foo" "FOO" NIL)"#,
    );
}

#[test]
fn preserves_exact_symbol_values_for_dynamic_operations() {
    assert_eq!(
        evaluate(
            r#"(progn
                (defvar |EXACT-FOO| 10)
                (set '|EXACT-FOO| 11)
                (list
                  (eq 'EXACT-FOO '|EXACT-FOO|)
                  (symbol-name '|EXACT-FOO|)
                  (boundp '|EXACT-FOO|)
                  (symbol-value '|EXACT-FOO|)
                  (boundp 'EXACT-FOO)))"#,
        )
        .to_string(),
        r#"(NIL "EXACT-FOO" T 11 NIL)"#,
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
fn evaluates_nested_quasiquote_vector_and_dotted_tail_splicing() {
    assert_eq!(
        evaluate(
            "(let ((x 2) (xs '(3 4)))
               (list \u{60}(outer \u{60}(inner ,x))
                     \u{60}#(1 ,x ,@xs)
                     \u{60}(a . ,@xs)))",
        )
        .to_string(),
        "((OUTER (QUASIQUOTE (INNER (UNQUOTE X)))) #(1 2 3 4) (A 3 4))"
    );
}

#[test]
fn evaluates_quasiquote_dotted_tail_as_proper_list() {
    assert_eq!(
        evaluate(
            "(let ((tail '(1 2)))
               (list `(a . ,tail)
                     (listp `(a . ,tail))
                     (length `(a . ,tail))))",
        )
        .to_string(),
        "((A 1 2) T 3)"
    );
}

#[test]
fn expands_user_macros_before_evaluation() {
    let values = Runtime::new()
        .eval_source("(defmacro twice (x) \u{60}(+ ,x ,x)) (twice 4)")
        .unwrap();

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
fn evaluates_macrolet_inside_eval() {
    assert_eq!(
        evaluate("(macrolet ((m () 42)) (eval '(m)))").to_string(),
        "42"
    );
}

#[test]
fn evaluates_macrolet_through_callable_eval() {
    assert_eq!(
        evaluate(
            "(macrolet ((m () 42))
               (list
                 (funcall #'eval '(m))
                 (apply #'eval (list '(m)))))",
        )
        .to_string(),
        "(42 42)"
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
        .unwrap();

    assert_eq!(values[1].to_string(), "(1 2 3)");
}

#[test]
fn macroexpand_1_returns_expanded_and_unexpanded_forms() {
    let values = Runtime::new()
        .eval_source(
            "(defmacro twice (x) \u{60}(+ ,x ,x)) \
             (macroexpand-1 '(twice 4)) (macroexpand-1 '(+ 1 2))",
        )
        .unwrap();

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
        .unwrap();

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
        .unwrap();

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
        .unwrap();

    assert_eq!(values[1].to_string(), "((+ 4 4) T)");
    assert_eq!(values[2].to_string(), "((+ 1 2) NIL)");
    assert_eq!(values[4].to_string(), "((+ 3 3) T)");
}

#[test]
fn recursive_macro_expansion_has_a_typed_limit() {
    let error = Runtime::new()
        .eval_source("(defmacro loop (x) '(loop x)) (loop 1)")
        .unwrap_err();

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
        .unwrap_err();

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
        .unwrap_err();

    assert!(matches!(
        error,
        ncl_runtime::RuntimeError::InvalidForm { message, .. }
            if message.contains("&rest")
    ));
}

#[test]
fn macro_lambda_lists_accept_nil_as_an_empty_list() {
    let values = Runtime::new()
        .eval_source("(defmacro no-arguments nil '(quote ok)) (no-arguments)")
        .unwrap();

    assert_eq!(values[1].to_string(), "OK");
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
        .unwrap();

    assert_eq!(
        values[1].to_string(),
        "((7 DEFAULT NIL 9 T TAG) (7 LABEL T 9 T TAG))"
    );
}

#[test]
fn macro_lambda_lists_support_body_and_destructuring() {
    let values = Runtime::new()
        .eval_source(
            "(defmacro first-of ((first . rest)) first)
             (defmacro list-body (first &body body) `(list ,first ,@body))
             (list (first-of (10 20)) (list-body 1 2 3))",
        )
        .unwrap();

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
        .unwrap();
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
        .unwrap();

    assert_eq!(
        values[1].to_string(),
        "((1 NIL) (1 (2 3)) (7 (8 9)) (4 (5 6)) (11 (2 3)) NIL)"
    );
}

#[test]
fn declarations_are_accepted_in_function_bodies() {
    assert_eq!(
        evaluate(
            "(defun declared (value)
               (declare (type integer value) (ignore value))
               42)
             (let ((value 9))
               (declare (type integer value))
               (list (declared 7) value))",
        )
        .to_string(),
        "(42 9)"
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
        .unwrap();

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
        .unwrap();

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
        .unwrap();

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
        .unwrap();

    assert_eq!(
        values[1].to_string(),
        "((4 5 NIL 9 NIL 20 T (:SECOND 20) 29) (4 7 T 30 T 31 NIL (:FIRST 30 :OTHER 99) 61))"
    );
}

#[test]
fn duplicate_keyword_arguments_use_first_value() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defun duplicate-keyword (&key value) value)
                 (defmacro duplicate-keyword-macro (&key value) value)
                 (defstruct duplicate-keyword-record value)
                 (defstruct
                   (duplicate-keyword-boa
                    (:constructor make-duplicate-keyword-boa (&key value)))
                   value)
                 (list
                   (duplicate-keyword :value 1 :value 2)
                   (duplicate-keyword-macro :value 1 :value 2)
                   (destructuring-bind (&key value)
                       (list :value 1 :value 2)
                     value)
                   (duplicate-keyword-record-value
                     (make-duplicate-keyword-record :value 1 :value 2))
                   (duplicate-keyword-boa-value
                     (make-duplicate-keyword-boa :value 1 :value 2))))"#,
        )
        .unwrap();

    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(1 1 1 1 1)");
}

#[test]
fn ordinary_keyword_parameters_honor_dynamic_allow_other_keys() {
    let values = Runtime::new()
        .eval_source(
            "(defun read-value (&key value) value)
             (read-value :allow-other-keys t :ignored 2 :value 3)",
        )
        .unwrap();

    assert_eq!(values[1].to_string(), "3");
}

#[test]
fn ordinary_keyword_parameters_reject_unknown_and_malformed_arguments() {
    let unknown = Runtime::new()
        .eval_source("(defun read-value (&key value) value) (read-value :ignored 2)")
        .unwrap_err();
    assert!(matches!(
        unknown,
        ncl_runtime::RuntimeError::InvalidForm { message, .. }
            if message.contains("unknown keyword")
    ));

    let escaped = Runtime::new()
        .eval_source("(defun read-value (&key scale) scale) (read-value :|Scale| 2)")
        .unwrap_err();
    assert!(matches!(
        escaped,
        ncl_runtime::RuntimeError::InvalidForm { message, .. }
            if message == "unknown keyword :Scale"
    ));

    let malformed = Runtime::new()
        .eval_source("(defun read-value (&key value) value) (read-value 'value 2)")
        .unwrap_err();
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
        .unwrap_err();
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
        .unwrap_err();
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
        let error = Runtime::new().eval_source(source).unwrap_err();

        assert!(
            matches!(error, ncl_runtime::RuntimeError::InvalidForm { .. }),
            "{source}: {error:?}"
        );
    }
}

#[test]
fn definitions_are_visible_to_later_forms() {
    let values = Runtime::new()
        .eval_source("(define answer 41) (+ answer 1)")
        .unwrap();

    assert_eq!(values[1].to_string(), "42");
}

#[test]
fn errors_are_typed() {
    let error = Runtime::new().eval_source("(+ 1 nil)").unwrap_err();

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
        .unwrap_err();
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
        .unwrap_err();
    assert!(matches!(
        error,
        ncl_runtime::RuntimeError::InvalidForm { message, .. }
            if message == "etypecase fell through"
    ));
}

#[test]
fn evaluates_ccase_and_ctypecase_with_store_value_restart() {
    assert_eq!(
        evaluate(
            "(let ((box (vector :bad)) (reads 0))
               (list
                 (handler-bind ((condition
                                  (lambda (condition)
                                    (declare (ignore condition))
                                    (invoke-restart 'store-value :ok))))
                   (ccase (aref (progn (incf reads) box) 0)
                     (:ok :hit)))
                 reads
                 (aref box 0)
                 (let ((value :bad))
                   (list
                     (handler-bind ((condition
                                      (lambda (condition)
                                        (declare (ignore condition))
                                        (invoke-restart 'store-value 42))))
                       (ctypecase value
                         (integer :integer)))
                     value))))",
        )
        .to_string(),
        "(:HIT 1 :OK (:INTEGER 42))"
    );
    assert_eq!(
        evaluate(
            "(let ((box (vector :bad)) (reads 0))
               (list
                 (handler-bind ((condition
                                  (lambda (condition)
                                    (declare (ignore condition))
                                    (invoke-restart 'store-value :ok))))
                   (ccase (elt (progn (incf reads) box) 0)
                     (:ok :hit)))
                 reads
                 (aref box 0)))",
        )
        .to_string(),
        "(:HIT 1 :OK)"
    );
}

#[test]
fn defvar_preserves_and_defparameter_replaces_values() {
    let values = Runtime::new()
        .eval_source(
            "(defvar answer 1) (defvar answer 2) answer \
             (defparameter answer 3) answer",
        )
        .unwrap();

    assert_eq!(values[1].to_string(), "1");
    assert_eq!(values[2].to_string(), "1");
    assert_eq!(values[4].to_string(), "3");
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

    assert!(Runtime::new()
        .eval_source("(defconstant +answer+ 42) (setq +answer+ 7)")
        .is_err());
    assert!(Runtime::new()
        .eval_source("(defconstant +answer+ 42) (setf (symbol-value '+answer+) 7)")
        .is_err());
    assert!(Runtime::new()
        .eval_source("(defconstant +answer+ 42) (psetq +answer+ 7)")
        .is_err());
}

#[test]
fn special_variables_are_dynamically_bound_and_accessible_by_symbol_primitives() {
    assert_eq!(
        evaluate(
            "(progn
               (defvar *dynamic-test* 10)
               (list
                 (boundp '*dynamic-test*)
                 *dynamic-test*
                 (let ((*dynamic-test* 20))
                   (list *dynamic-test*
                         (funcall (lambda () *dynamic-test*))))
                 (set '*dynamic-test* 30)
                 (symbol-value '*dynamic-test*)
                 (makunbound '*dynamic-test*)
                 (boundp '*dynamic-test*)))",
        )
        .to_string(),
        "(T 10 (20 20) 30 30 *DYNAMIC-TEST* NIL)"
    );
    assert_eq!(
        evaluate(
            "(progn
               (defvar *dynamic-force* 1)
               (defparameter *dynamic-force* 2)
               *dynamic-force*)",
        )
        .to_string(),
        "2"
    );
}

#[test]
fn evaluates_special_declarations_in_lexical_and_global_scopes() {
    assert_eq!(
        evaluate(
            "(progn
               (defun read-declared-special () declared-special)
               (defun read-declaimed-special () declaimed-special)
               (defun read-proclaimed-special () proclaimed-special)
               (declaim (special declaimed-special))
               (proclaim '(special proclaimed-special))
               (list
                 (let ((declared-special 20))
                   (declare (special declared-special))
                   (read-declared-special))
                 (ignore-errors (read-declared-special))
                 (let ((declaimed-special 21)
                       (proclaimed-special 22))
                   (list (read-declaimed-special)
                         (read-proclaimed-special)))
                 (ignore-errors (read-declaimed-special))
                 (ignore-errors (read-proclaimed-special))))",
        )
        .to_string(),
        "(20 NIL (21 22) NIL NIL)"
    );
}

#[test]
fn boundp_and_symbol_value_ignore_lexical_bindings() {
    assert_eq!(
        evaluate(
            "(let ((lexical-bound 1))
               (list
                 (boundp 'lexical-bound)
                 (ignore-errors (symbol-value 'lexical-bound))))",
        )
        .to_string(),
        "(NIL NIL)"
    );
}

#[test]
fn function_body_special_declaration_dynamically_binds_parameters() {
    assert_eq!(
        evaluate(
            "(progn
               (defun function-special-value (value)
                 (declare (special value))
                 (list value (boundp 'value) (symbol-value 'value)))
               (list
                 (function-special-value 7)
                 (ignore-errors (symbol-value 'value))))",
        )
        .to_string(),
        "((7 T 7) NIL)"
    );
}

#[test]
fn progv_temporarily_binds_symbols_and_restores_them() {
    assert_eq!(
        evaluate(
            "(progn
               (defvar *progv-test* 1)
               (list
                 *progv-test*
                 (progv '(*progv-test* fresh-variable)
                        '(2 3)
                        (list *progv-test*
                              fresh-variable
                              (funcall (lambda () (list *progv-test* fresh-variable)))))
                 *progv-test*
                 (boundp 'fresh-variable)))",
        )
        .to_string(),
        "(1 (2 3 (2 3)) 1 NIL)"
    );
}

#[test]
fn evaluates_with_simple_restart_and_invoke_restart() {
    assert_eq!(
        evaluate(
            "(with-simple-restart
               (abort \"abort\")
               (invoke-restart 'abort 42))",
        )
        .to_string(),
        "42"
    );
    assert_eq!(
        evaluate(
            "(handler-case
               (with-simple-restart (abort \"abort\") (/ 1 0))
               (division-by-zero (condition) 9))",
        )
        .to_string(),
        "9"
    );
}

#[test]
fn evaluates_restart_case_and_passes_restart_arguments() {
    assert_eq!(
        evaluate(
            "(restart-case
               (invoke-restart 'use-values 20 22)
               (use-values (left right) (+ left right)))",
        )
        .to_string(),
        "42"
    );
    assert_eq!(
        evaluate(
            "(restart-case
               (invoke-restart 'abort)
               (abort () 7))",
        )
        .to_string(),
        "7"
    );
    assert_eq!(
        evaluate(
            "(restart-case
               (invoke-restart 'use-value 4)
               (use-value (value &optional (delta 2)) (+ value delta)))",
        )
        .to_string(),
        "6"
    );
}

#[test]
fn evaluates_restart_bind_invokes_function_and_propagates() {
    assert_eq!(
        evaluate(
            "(restart-bind
               ((use-values (lambda (left right) (+ left right))))
               (invoke-restart 'use-values 20 22))",
        )
        .to_string(),
        "42"
    );
    assert_eq!(
        evaluate(
            "(restart-case
               (restart-bind
                 ((use-value (lambda (value) value)))
                 (invoke-restart 'outer 7))
               (outer (value) value))",
        )
        .to_string(),
        "7"
    );
}

#[test]
fn evaluates_invoke_restart_interactively() {
    assert_eq!(
        evaluate(
            "(list
               (restart-bind
                 ((use-values (lambda () :handled)))
                 (invoke-restart-interactively 'use-values))
               (restart-case
                 (invoke-restart-interactively 'abort)
                 (abort () :aborted)))",
        )
        .to_string(),
        "(:HANDLED :ABORTED)"
    );
}

#[test]
fn evaluates_restart_introspection_and_object_invocation() {
    assert_eq!(
        evaluate(
            "(let ((seen nil))
               (restart-case
                 (with-simple-restart (outer \"outer\")
                   (let ((restart (find-restart 'use-value)))
                     (setq seen
                       (list
                         (typep restart 'restart)
                         (eq restart (find-restart 'use-value))
                         (restart-name restart)
                         (restart-name (car (compute-restarts)))
                         (restart-name (car (cdr (compute-restarts))))
                         (eq restart (car (cdr (compute-restarts))))))
                     (invoke-restart restart 42)))
                 (use-value (value) (list seen value))))",
        )
        .to_string(),
        "((T T USE-VALUE OUTER USE-VALUE T) 42)"
    );
}

#[test]
fn evaluates_restart_function_from_symbol_and_object_designators() {
    assert_eq!(
        evaluate(
            "(restart-bind
               ((use-values (lambda (left right) (+ left right))))
               (let ((restart (find-restart 'use-values)))
                 (list
                   (funcall (restart-function restart) 20 22)
                   (funcall (restart-function 'use-values) 7 5)
                   (eq (restart-function restart)
                       (restart-function 'use-values)))))",
        )
        .to_string(),
        "(42 12 T)"
    );
}

#[test]
fn evaluates_condition_restart_associations() {
    assert_eq!(
        evaluate(
            "(let ((condition (make-condition 'simple-condition
                              :format-control \"condition\"))
                  (other (make-condition 'simple-condition
                          :format-control \"other\"))
                  (seen nil))
               (restart-case
                 (with-simple-restart (outer \"outer\")
                   (with-condition-restarts
                       condition
                       (list (find-restart 'use-value))
                     (with-condition-restarts
                         other
                         (list (find-restart 'outer))
                       (setq seen
                         (list
                           (mapcar #'restart-name (compute-restarts condition))
                           (mapcar #'restart-name (compute-restarts other))
                           (mapcar #'restart-name (compute-restarts))
                           (restart-name (find-restart 'use-value condition))
                           (find-restart 'outer condition)))
                       (invoke-restart 'use-value 42))))
                 (use-value (value) (list seen value))))",
        )
        .to_string(),
        "(((USE-VALUE) (OUTER) (OUTER USE-VALUE) USE-VALUE NIL) 42)"
    );
}

#[test]
fn evaluates_parallel_assignments_and_multiple_value_setq() {
    assert_eq!(
        evaluate(
            "(let ((a 1) (b 2))
               (list
                 (psetq a b b a)
                 a
                 b
                 (multiple-value-setq (a b) (values 3 4))
                 a
                 b))",
        )
        .to_string(),
        "(NIL 2 1 3 3 4)"
    );
    assert_eq!(
        evaluate(
            "(let ((a 0) (b 0))
               (list (multiple-value-setq (a b) 7) a b))",
        )
        .to_string(),
        "(7 7 NIL)"
    );
}

#[test]
fn arithmetic_reports_overflow_and_comparisons_require_an_argument() {
    let overflow = Runtime::new()
        .eval_source("(+ 9223372036854775807 1)")
        .unwrap_err();
    assert!(matches!(
        overflow,
        ncl_runtime::RuntimeError::NumericOverflow
    ));

    let comparison_error = Runtime::new().eval_source("(=)").unwrap_err();
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
fn evaluates_forms_and_maps_functions_over_lists() {
    assert_eq!(evaluate("(eval '(+ 2 3))").to_string(), "5");
    assert_eq!(
        evaluate("(let ((form '(+ 2 3))) (eval form))").to_string(),
        "5"
    );
    assert_eq!(evaluate("(funcall #'eval '(+ 2 3))").to_string(), "5");
    assert_eq!(
        evaluate("(funcall #'funcall #'list 1 2)").to_string(),
        "(1 2)"
    );
    assert_eq!(
        evaluate("(funcall #'apply #'list 1 '(2 3))").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate("(mapcar (lambda (x) (* x 2)) '(1 2 3))").to_string(),
        "(2 4 6)"
    );
    assert_eq!(
        evaluate("(mapcar (lambda (x y) (+ x y)) '(1 2) '(10 20 30))").to_string(),
        "(11 22)"
    );
    assert_eq!(
        evaluate("(funcall #'mapcar (lambda (x) (+ x 1)) '(1 2 3))").to_string(),
        "(2 3 4)"
    );
    assert_eq!(evaluate("(funcall 'car '(9 8))").to_string(), "9");
    assert_eq!(evaluate("(apply 'list 1 '(2 3))").to_string(), "(1 2 3)");
    assert_eq!(
        evaluate("(mapcar 'car '((1 2) (3 4)))").to_string(),
        "(1 3)"
    );
    assert_eq!(
        evaluate("(let ((function-name 'car)) (funcall function-name '(7 6)))").to_string(),
        "7"
    );
    assert_eq!(
        evaluate("(mapc (lambda (x) (* x 2)) '(1 2 3))").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate("(mapl (lambda (tail) (car tail)) '(1 2 3))").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate("(maplist (lambda (tail) (car tail)) '(1 2 3))").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate(
            "(maplist (lambda (left right) (list (car left) (car right)))
                      '(1 2) '(10 20 30))",
        )
        .to_string(),
        "((1 10) (2 20))"
    );
    assert_eq!(
        evaluate("(mapcan (lambda (x) (list x (* x 10))) '(1 2 3))").to_string(),
        "(1 10 2 20 3 30)"
    );
    assert_eq!(
        evaluate("(mapcon (lambda (tail) (list (car tail))) '(1 2 3))").to_string(),
        "(1 2 3)"
    );
}

#[test]
fn evaluates_map_over_sequence_types() {
    assert_eq!(
        evaluate("(map 'list (lambda (x) (* x 2)) '(1 2 3))").to_string(),
        "(2 4 6)"
    );
    assert_eq!(
        evaluate("(map 'vector #'1+ #(1 2 3))").to_string(),
        "#(2 3 4)"
    );
    assert_eq!(
        evaluate("(map 'string #'identity \"abc\")").to_string(),
        "\"abc\""
    );
    assert_eq!(
        evaluate("(map 'list #'+ '(1 2) '(10 20 30))").to_string(),
        "(11 22)"
    );
    assert_eq!(
        evaluate(
            "(let ((total 0))
               (map nil (lambda (x) (incf total x)) '(1 2 3))
               total)",
        )
        .to_string(),
        "6"
    );
}

#[test]
fn evaluates_reduce_over_sequences() {
    assert_eq!(evaluate("(reduce #'+ '(1 2 3 4))").to_string(), "10");
    assert_eq!(
        evaluate("(reduce #'- '(1 2 3) :from-end t)").to_string(),
        "2"
    );
    assert_eq!(
        evaluate("(reduce #'+ '(1 2 3) :initial-value 10)").to_string(),
        "16"
    );
    assert_eq!(
        evaluate("(reduce #'+ '(1 2 3 4) :start 1 :end 3)").to_string(),
        "5"
    );
    assert_eq!(
        evaluate("(reduce #'+ '((1) (2) (3)) :key #'car)").to_string(),
        "6"
    );
    assert_eq!(
        evaluate("(reduce #'+ \"abc\" :key #'char-code)").to_string(),
        "294"
    );
    assert_eq!(
        evaluate("(reduce #'list '() :initial-value 42)").to_string(),
        "42"
    );
}

#[test]
fn evaluates_sequence_searches() {
    assert_eq!(evaluate("(find 2 '(1 2 3))").to_string(), "2");
    assert_eq!(evaluate("(position 2 #(1 2 3))").to_string(), "1");
    assert_eq!(evaluate("(count 2 '(1 2 2 3))").to_string(), "2");
    assert_eq!(
        evaluate("(position 2 '(1 2 3 2) :from-end t)").to_string(),
        "3"
    );
    assert_eq!(
        evaluate(
            "(find 2 '(1 2 3) :test-not (lambda (wanted candidate)\n               (= wanted (+ candidate 1))))",
        )
        .to_string(),
        "2"
    );
    assert_eq!(
        evaluate("(position 20 '(10 20 30) :start 1 :end 3)").to_string(),
        "1"
    );
    assert_eq!(
        evaluate("(find 2 '((1) (2) (3)) :key #'car)").to_string(),
        "(2)"
    );
    assert_eq!(evaluate("(count 2 '(1 2 3 2) :key #'1+)").to_string(), "1");
    assert_eq!(evaluate("(find 9 '(1 2 3))").to_string(), "NIL");
}

#[test]
fn evaluates_sequence_search_and_mismatch() {
    assert_eq!(evaluate("(search '(2 3) '(1 2 3 4))").to_string(), "1");
    assert_eq!(
        evaluate("(search '(2 3) '(1 2 3 2 3) :from-end t)").to_string(),
        "3"
    );
    assert_eq!(
        evaluate("(search '(0 1) '(2 4 6 1 3 5) :key #'oddp)").to_string(),
        "2"
    );
    assert_eq!(
        evaluate("(search \"ab\" \"xxABab\" :test #'char-equal :from-end t)").to_string(),
        "4"
    );
    assert_eq!(
        evaluate("(search '(2 3) '(0 1 2 3 4) :start2 2 :end2 5)").to_string(),
        "2"
    );
    assert_eq!(evaluate("(search '() '(1 2) :start2 1)").to_string(), "1");
    assert_eq!(
        evaluate("(search '() '(1 2) :start2 1 :from-end t)").to_string(),
        "2"
    );
    assert_eq!(evaluate("(mismatch '(1 2 9) '(1 2 3))").to_string(), "2");
    assert_eq!(
        evaluate("(mismatch '(3 2 1 1 2 3) '(1 2 3) :from-end t)").to_string(),
        "3"
    );
    assert_eq!(
        evaluate("(mismatch \"abcd\" \"ABCDE\" :test #'char-equal)").to_string(),
        "4"
    );
    assert_eq!(
        evaluate("(mismatch '(1 2 3) '(2 3 4) :test-not #'eq :key #'oddp)").to_string(),
        "NIL"
    );
    assert_eq!(
        evaluate("(mismatch \"def\" \"abcdef\" :from-end t)").to_string(),
        "0"
    );
    assert_eq!(evaluate("(funcall #'search '(2) '(0 2))").to_string(), "1");
}

#[test]
fn evaluates_sequence_sort_and_stable_sort() {
    assert_eq!(evaluate("(sort '(3 1 2) #'<)").to_string(), "(1 2 3)");
    assert_eq!(
        evaluate("(stable-sort '(2 -2 1 -1) #'< :key #'abs)").to_string(),
        "(1 -1 2 -2)"
    );
    assert_eq!(evaluate("(sort #(3 1 2) #'<)").to_string(), "#(1 2 3)");
    assert_eq!(evaluate("(sort \"cba\" #'char<)").to_string(), "\"abc\"");
    assert_eq!(
        evaluate("(funcall #'stable-sort '(3 1 2) #'<)").to_string(),
        "(1 2 3)"
    );
}

#[test]
fn evaluates_sequence_merge() {
    assert_eq!(
        evaluate("(merge 'list '(1 3 5) '(2 4 6) #'<)").to_string(),
        "(1 2 3 4 5 6)"
    );
    assert_eq!(
        evaluate("(merge 'vector #(1 3) #(2 4) #'<)").to_string(),
        "#(1 2 3 4)"
    );
    assert_eq!(
        evaluate("(merge 'string \"ace\" \"bdf\" #'char<)").to_string(),
        "\"abcdef\""
    );
    assert_eq!(
        evaluate("(merge 'list '(-1 -3) '(2 4) #'< :key #'abs)").to_string(),
        "(-1 2 -3 4)"
    );
    assert_eq!(
        evaluate("(merge 'list '((1 a) (2 b)) '((1 c) (2 d)) #'< :key #'car)").to_string(),
        "((1 A) (1 C) (2 B) (2 D))"
    );
    assert_eq!(
        evaluate("(funcall #'merge 'list '(1 3) '(2 4) #'<)").to_string(),
        "(1 2 3 4)"
    );
}

#[test]
fn evaluates_sequence_quantifiers() {
    assert_eq!(evaluate("(every #'numberp '(1 2))").to_string(), "T");
    assert_eq!(evaluate("(every #'= '(1 2) #(1 2))").to_string(), "T");
    assert_eq!(evaluate("(some #'identity '(nil 2 4))").to_string(), "2");
    assert_eq!(evaluate("(notany #'evenp '(1 3 5))").to_string(), "T");
    assert_eq!(evaluate("(notevery #'evenp '(2 4 5))").to_string(), "T");
    assert_eq!(evaluate("(every #'char= \"ab\" \"ab\")").to_string(), "T");
    assert_eq!(evaluate("(every #'identity '())").to_string(), "T");
    assert_eq!(evaluate("(some #'identity '())").to_string(), "NIL");
    assert_eq!(
        evaluate("(funcall #'some #'identity '(nil 3))").to_string(),
        "3"
    );
}

#[test]
fn evaluates_list_membership_and_association_searches() {
    assert_eq!(evaluate("(member 2 '(1 2 3))").to_string(), "(2 3)");
    assert_eq!(
        evaluate("(member 2 '(1 2 3) :test #'=)").to_string(),
        "(2 3)"
    );
    assert_eq!(
        evaluate("(member 2 '((1) (2) (3)) :key #'car)").to_string(),
        "((2) (3))"
    );
    assert_eq!(
        evaluate("(member-if #'evenp '(1 3 4 6))").to_string(),
        "(4 6)"
    );
    assert_eq!(
        evaluate("(member-if-not #'evenp '(2 4 5 6))").to_string(),
        "(5 6)"
    );
    assert_eq!(evaluate("(adjoin 2 '(1 2 3))").to_string(), "(1 2 3)");
    assert_eq!(evaluate("(adjoin 4 '(1 2 3))").to_string(), "(4 1 2 3)");
    assert_eq!(
        evaluate("(assoc 'b '((a . 1) (b . 2)))").to_string(),
        "(B . 2)"
    );
    assert_eq!(
        evaluate("(assoc-if (lambda (key) (eq key 'b)) '((a . 1) (b . 2)))").to_string(),
        "(B . 2)"
    );
    assert_eq!(
        evaluate("(rassoc 2 '((a . 1) (b . 2)))").to_string(),
        "(B . 2)"
    );
    assert_eq!(
        evaluate("(rassoc-if #'evenp '((a . 1) (b . 2)))").to_string(),
        "(B . 2)"
    );
    assert_eq!(
        evaluate(
            "(member 2 '(1 2 3) :test-not (lambda (wanted candidate)\n               (= wanted (+ candidate 1))))",
        )
        .to_string(),
        "(2 3)"
    );
    assert_eq!(
        evaluate("(funcall #'member 2 '(1 2 3))").to_string(),
        "(2 3)"
    );
}

#[test]
fn evaluates_sequence_removals() {
    assert_eq!(evaluate("(remove 2 '(1 2 2 3))").to_string(), "(1 3)");
    assert_eq!(
        evaluate("(remove 2 '(1 2 3 2) :from-end t :count 1)").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate("(remove 2 '(1 2 3 2) :start 1 :end 3)").to_string(),
        "(1 3 2)"
    );
    assert_eq!(
        evaluate("(remove-if #'evenp '(1 2 4 3))").to_string(),
        "(1 3)"
    );
    assert_eq!(
        evaluate("(remove-if-not #'evenp '(1 2 4 3))").to_string(),
        "(2 4)"
    );
    assert_eq!(evaluate("(remove 2 #(1 2 3))").to_string(), "#(1 3)");
    assert_eq!(
        evaluate("(remove #\\a \"banana\" :count 2)").to_string(),
        "\"bnna\""
    );
    assert_eq!(
        evaluate("(remove 2 '((1) (2) (2)) :key #'car :count 1)").to_string(),
        "((1) (2))"
    );
    assert_eq!(
        evaluate("(remove-duplicates '(1 2 1 3 2))").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate("(remove-duplicates '(1 2 1 3 2) :from-end t)").to_string(),
        "(1 3 2)"
    );
    assert_eq!(
        evaluate("(delete-if #'evenp '(1 2 4 3))").to_string(),
        "(1 3)"
    );
    assert_eq!(
        evaluate("(delete-duplicates '(1 2 1))").to_string(),
        "(1 2)"
    );
    assert_eq!(
        evaluate("(funcall #'remove 2 '(1 2 3))").to_string(),
        "(1 3)"
    );
}

#[test]
fn evaluates_sequence_substitutions() {
    assert_eq!(
        evaluate("(substitute 9 2 '(1 2 2 3))").to_string(),
        "(1 9 9 3)"
    );
    assert_eq!(
        evaluate("(substitute 9 2 '(1 2 2 3) :from-end t :count 1)").to_string(),
        "(1 2 9 3)"
    );
    assert_eq!(
        evaluate("(substitute-if 0 #'evenp '(1 2 4 3))").to_string(),
        "(1 0 0 3)"
    );
    assert_eq!(
        evaluate("(substitute-if-not 0 #'evenp '(1 2 4 3))").to_string(),
        "(0 2 4 0)"
    );
    assert_eq!(
        evaluate("(substitute 0 2 #(1 2 3))").to_string(),
        "#(1 0 3)"
    );
    assert_eq!(
        evaluate("(substitute #\\x #\\a \"banana\" :count 2)").to_string(),
        "\"bxnxna\""
    );
    assert_eq!(
        evaluate("(substitute 9 2 '((1) (2) (2)) :key #'car :count 1)").to_string(),
        "((1) 9 (2))"
    );
    assert_eq!(
        evaluate("(substitute 9 2 '(1 2 3) :test #'= :start 1 :end 3)").to_string(),
        "(1 9 3)"
    );
    assert_eq!(
        evaluate("(nsubstitute-if 0 #'evenp '(1 2 3))").to_string(),
        "(1 0 3)"
    );
    assert_eq!(
        evaluate("(funcall #'substitute 9 2 '(1 2 3))").to_string(),
        "(1 9 3)"
    );
}

#[test]
fn evaluates_list_set_operations() {
    assert_eq!(evaluate("(union '(1 2 2) '(2 3 3))").to_string(), "(1 2 3)");
    assert_eq!(
        evaluate("(nunion '(1 2 2) '(2 3 3))").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate("(intersection '(1 2 2 3) '(2 3 4))").to_string(),
        "(2 3)"
    );
    assert_eq!(
        evaluate("(nintersection '(1 2 2 3) '(2 3 4))").to_string(),
        "(2 3)"
    );
    assert_eq!(
        evaluate("(set-difference '(1 2 2 3) '(2))").to_string(),
        "(1 3)"
    );
    assert_eq!(
        evaluate("(nset-difference '(1 2 2 3) '(2))").to_string(),
        "(1 3)"
    );
    assert_eq!(
        evaluate("(set-exclusive-or '(1 2 2 3) '(2 4))").to_string(),
        "(1 3 4)"
    );
    assert_eq!(
        evaluate("(nset-exclusive-or '(1 2 2 3) '(2 4))").to_string(),
        "(1 3 4)"
    );
    assert_eq!(evaluate("(subsetp '(1 2) '(3 2 1 4))").to_string(), "T");
    assert_eq!(evaluate("(subsetp '(1 5) '(3 2 1 4))").to_string(), "NIL");
    assert_eq!(
        evaluate("(union '(1 2) '(2 3) :test #'=)").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate("(union '((1 a) (2 b)) '((1 c) (3 d)) :key #'car)").to_string(),
        "((1 A) (2 B) (3 D))"
    );
    assert_eq!(
        evaluate("(set-difference '(1 2 3) '(2) :test-not #'eql)").to_string(),
        "(2)"
    );
    assert_eq!(evaluate("(funcall #'union '(1) '(2))").to_string(), "(1 2)");
}

#[test]
fn evaluates_list_construction_and_partitioning() {
    assert_eq!(evaluate("(list* 1 2 '(3 4))").to_string(), "(1 2 3 4)");
    assert_eq!(evaluate("(list* 1 2 3)").to_string(), "(1 2 . 3)");
    assert_eq!(evaluate("(list* 7)").to_string(), "7");
    assert_eq!(
        evaluate("(make-list 3 :initial-element 'x)").to_string(),
        "(X X X)"
    );
    assert_eq!(evaluate("(make-list 2)").to_string(), "(NIL NIL)");
    assert_eq!(evaluate("(copy-list '(1 2 3))").to_string(), "(1 2 3)");
    assert_eq!(
        evaluate("(copy-tree '((1) (2 3)))").to_string(),
        "((1) (2 3))"
    );
    assert_eq!(evaluate("(list-length '(1 2 3))").to_string(), "3");
    assert_eq!(evaluate("(nthcdr 2 '(1 2 3))").to_string(), "(3)");
    assert_eq!(evaluate("(nthcdr 3 '(1 2 3))").to_string(), "NIL");
    assert_eq!(evaluate("(nthcdr 1 '(1 . 2))").to_string(), "2");
    assert_eq!(
        evaluate("(acons 'a 1 '((b . 2)))").to_string(),
        "((A . 1) (B . 2))"
    );
    assert_eq!(
        evaluate("(pairlis '(a b) '(1 2) '((c . 3)))").to_string(),
        "((B . 2) (A . 1) (C . 3))"
    );
    assert_eq!(
        evaluate("(copy-alist '((a . 1) (b 2)))").to_string(),
        "((A . 1) (B 2))"
    );
    assert_eq!(
        evaluate("(multiple-value-list (get-properties '(:a 1 :b 2) '(:b :a)))").to_string(),
        "(:A 1 (:A 1 :B 2))"
    );
    assert_eq!(
        evaluate("(multiple-value-list (get-properties '(:a 1) '(:z)))").to_string(),
        "(NIL NIL NIL)"
    );
    assert_eq!(evaluate("(last '(1 2 3))").to_string(), "(3)");
    assert_eq!(evaluate("(last '(1 2 3) 2)").to_string(), "(2 3)");
    assert_eq!(evaluate("(last '(1 2 3) 0)").to_string(), "NIL");
    assert_eq!(evaluate("(butlast '(1 2 3))").to_string(), "(1 2)");
    assert_eq!(evaluate("(nbutlast '(1 2 3) 2)").to_string(), "(1)");
    assert_eq!(evaluate("(reverse #(1 2 3))").to_string(), "#(3 2 1)");
    assert_eq!(evaluate("(nreverse \"abc\")").to_string(), "\"cba\"");
    assert_eq!(evaluate("(nreverse '(1 2 3))").to_string(), "(3 2 1)");
    assert_eq!(evaluate("(nconc '(1 2) '(3 4))").to_string(), "(1 2 3 4)");
    assert_eq!(evaluate("(nconc '(1 2) 3)").to_string(), "(1 2 . 3)");
    assert_eq!(
        evaluate("(revappend '(1 2) '(3 4))").to_string(),
        "(2 1 3 4)"
    );
    assert_eq!(evaluate("(nreconc '(1 2) '(3 4))").to_string(), "(2 1 3 4)");
    assert_eq!(
        evaluate("(funcall #'list* 1 '(2 3))").to_string(),
        "(1 2 3)"
    );
    assert_eq!(evaluate("(funcall #'nthcdr 1 '(4 5))").to_string(), "(5)");
}

#[test]
fn evaluates_sequence_fill_replace_and_concatenate() {
    assert_eq!(
        evaluate("(fill 0 '(1 2 3 4) :start 1 :end 3)").to_string(),
        "(1 0 0 4)"
    );
    assert_eq!(
        evaluate("(fill #\\x \"abcd\" :start 1)").to_string(),
        "\"axxx\""
    );
    assert_eq!(evaluate("(fill 9 #(1 2 3) :end 2)").to_string(), "#(9 9 3)");
    assert_eq!(
        evaluate("(replace '(9 9 9) '(1 2 3 4) :start1 1 :end1 3 :start2 0 :end2 2)").to_string(),
        "(9 1 2)"
    );
    assert_eq!(
        evaluate("(replace \"xxxx\" \"abcd\" :start1 1 :end1 3 :start2 0 :end2 2)").to_string(),
        "\"xabx\""
    );
    assert_eq!(evaluate("(copy-seq #(1 2))").to_string(), "#(1 2)");
    assert_eq!(
        evaluate("(concatenate 'list '(1 2) #(3) \"4\")").to_string(),
        "(1 2 3 #\\4)"
    );
    assert_eq!(
        evaluate("(concatenate 'string \"ab\" '(#\\c #\\d))").to_string(),
        "\"abcd\""
    );
    assert_eq!(
        evaluate("(funcall #'fill 0 '(1 2) :start 1)").to_string(),
        "(1 0)"
    );
}

#[test]
fn evaluates_map_into_over_sequences() {
    assert_eq!(
        evaluate(
            "(let ((result (vector 0 0 0)))
               (map-into result #'+ '(1 2)))",
        )
        .to_string(),
        "#(1 2 0)"
    );
    assert_eq!(
        evaluate(
            "(let ((result (list 9 9 9)))
               (map-into result #'1+ '(1 2))
               result)",
        )
        .to_string(),
        "(2 3 9)"
    );
    assert_eq!(
        evaluate(
            "(let ((result \"xxx\"))
               (map-into result #'identity \"ab\")
               result)",
        )
        .to_string(),
        "\"abx\""
    );
    assert_eq!(
        evaluate(
            "(let ((result (vector 0 0)))
               (map-into result (lambda () 7))
               result)",
        )
        .to_string(),
        "#(7 7)"
    );
    assert_eq!(
        evaluate("(map-into (vector 0 0) #'1+ '(1 2))").to_string(),
        "#(2 3)"
    );
    assert_eq!(
        evaluate("(map-into \"xx\" #'identity \"ab\")").to_string(),
        "\"ab\""
    );
    assert_eq!(evaluate("(map-into nil #'1+ '(1 2))").to_string(), "NIL");
    assert_eq!(
        evaluate(
            "(let ((container (vector (vector 0 0))))
               (map-into (aref container 0) #'1+ '(1 2))
               container)",
        )
        .to_string(),
        "#(#(2 3))"
    );
}

#[test]
fn rejects_non_sequence_map_into_destination() {
    let error = Runtime::new()
        .eval_source("(map-into 42 #'identity '(1))")
        .unwrap_err();

    assert!(matches!(
        error,
        RuntimeError::Type {
            expected,
            actual,
            ..
        } if expected == "SEQUENCE" && actual == "INTEGER"
    ));
}

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
        .unwrap_err();
    assert!(matches!(
        error,
        ncl_runtime::RuntimeError::UnboundVariable { name, .. }
            if name == "MISSING-FUNCTION"
    ));
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
}

#[test]
fn evaluates_load_file() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/load.lisp")
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    assert_eq!(
        evaluate(&format!(
            r#"(list (load "{}") *NCL-LOAD-VALUE* (NCL-LOAD-TARGET 1))"#,
            path
        ))
        .to_string(),
        "(T 41 42)"
    );
}

#[test]
fn evaluates_load_time_value() {
    assert_eq!(
        evaluate(
            "(let ((function (lambda () (load-time-value (+ 8 9)))))
               (list (funcall function) (funcall function)
                     (load-time-value (+ 1 2) nil)))",
        )
        .to_string(),
        "(17 17 3)"
    );
}

#[test]
fn evaluates_load_time_value_in_lexical_environment() {
    assert_eq!(
        evaluate("(let ((x 10)) (load-time-value x))").to_string(),
        "10"
    );
}

#[test]
fn evaluates_nth_value() {
    assert_eq!(
        evaluate(
            "(list
               (nth-value 0 (values 10 20))
               (nth-value 1 (values 10 20))
               (nth-value 4 (values 10 20))
               (nth-value 0 99)
               (nth-value 0 (values)))",
        )
        .to_string(),
        "(10 20 NIL 99 NIL)"
    );
}

#[test]
fn rejects_invalid_nth_value_indices() {
    let type_error = Runtime::new()
        .eval_source("(nth-value 'index (values 1))")
        .unwrap_err();
    assert!(matches!(
        type_error,
        RuntimeError::Type {
            expected,
            actual,
            ..
        } if expected == "INTEGER" && actual == "SYMBOL"
    ));

    let negative_error = Runtime::new()
        .eval_source("(nth-value -1 (values 1))")
        .unwrap_err();
    assert!(matches!(
        negative_error,
        RuntimeError::InvalidForm { message, .. }
            if message == "nth-value index must be non-negative"
    ));

    let arity_error = Runtime::new().eval_source("(nth-value 0)").unwrap_err();
    assert!(matches!(arity_error, RuntimeError::Arity { actual: 1, .. }));

    assert_eq!(
        Runtime::new()
            .eval_source("(nth-value 1000000 (values 1))")
            .unwrap()
            .pop()
            .unwrap()
            .to_string(),
        "NIL"
    );
}

#[test]
fn evaluates_function_and_macro_introspection() {
    assert_eq!(
        evaluate(
            "(progn
               (defmacro introspection-macro (value) (list '+ value 1))
               (defmacro local-macro-visible (&environment environment)
                 (if (functionp (macro-function 'local-macro environment))
                     '(quote t)
                     '(quote nil)))
               (list (functionp (macro-function 'introspection-macro))
                     (eq (macro-function 'missing-macro) nil)
                     (special-operator-p 'if)
                     (special-operator-p 'and)
                     (special-operator-p 'return-from)
                     (special-operator-p 'load-time-value)
                     (compiled-function-p (function +))
                     (macrolet ((local-macro (value) (list '+ value 2)))
                       (list (functionp (macro-function 'local-macro))
                             (local-macro-visible)))))",
        )
        .to_string(),
        "(T T T NIL NIL T NIL (NIL T))"
    );
}

#[test]
fn rejects_invalid_function_designators() {
    for source in [
        "(function (+ 1 2))",
        "(function (quote foo))",
        "(function (setf))",
        "(function (setf foo bar))",
        "(function 1)",
    ] {
        let error = Runtime::new()
            .eval_source(source)
            .expect_err("invalid function designator");
        assert!(
            matches!(error, RuntimeError::InvalidForm { .. }),
            "unexpected error for {source}: {error:?}"
        );
    }

    assert!(Runtime::new()
        .eval_source("(function (lambda (value) value))")
        .is_ok());
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
fn evaluates_fdefinition_setf() {
    assert_eq!(
        evaluate(
            "(progn
               (defun fdefinition-target (value) (+ value 2))
               (let ((name 'fdefinition-target))
                 (list (funcall (fdefinition name) 5)
                       (progn
                         (setf (fdefinition name)
                               (lambda (value) (+ value 3)))
                         (funcall (fdefinition name) 5))
                       (fboundp name))))",
        )
        .to_string(),
        "(7 8 T)"
    );
}

#[test]
fn evaluates_function_namespace_mutation() {
    assert_eq!(
        evaluate(
            "(progn
               (defun fmakunbound-target () 42)
               (list (fboundp 'fmakunbound-target)
                     (symbolp (fmakunbound 'fmakunbound-target))
                     (fboundp 'fmakunbound-target)))",
        )
        .to_string(),
        "(T T NIL)"
    );
}

#[test]
fn evaluates_numeric_predicates_and_extrema() {
    assert_eq!(
        evaluate("(list (zerop 0) (plusp 1) (minusp -1) (evenp 4) (oddp 3) (min 3 1 2) (max 3 1 2) (abs -5))").to_string(),
        "(T T T T T 1 3 5)"
    );
    assert_eq!(
        evaluate("(list (realp 1) (realp 1/2) (realp 1.0) (realp #\\a) (realp nil))").to_string(),
        "(T T T NIL NIL)"
    );
}

#[test]
fn evaluates_common_lisp_integer_arithmetic_and_bit_operations() {
    assert_eq!(
        evaluate(
            "(list (mod -7 3) (mod 7 -3) (rem -7 3) (rem 7 -3)
                    (ash 3 2) (ash -8 -2)
                    (logand 7 3) (logior 4 1) (logxor 7 3) (lognot 0)
                    (logeqv 7 3) (lognand 7 3) (lognor 7 3)
                    (logandc1 7 3) (logandc2 7 3) (logorc1 7 3) (logorc2 7 3)
                    (boole boole-clr 7 3) (boole boole-set 7 3)
                    (boole boole-and 7 3) (boole boole-orc2 7 3)
                    (logtest 6 2) (logtest 4 2)
                    (logbitp 0 1) (logbitp 1 1) (logbitp 5 32)
                    (logbitp 63 -1) (logbitp 64 -1) (logbitp 64 0)
                    (logcount 13) (logcount -8)
                    (integer-length 8) (integer-length -8)
                    (logand) (logior) (logxor))",
        )
        .to_string(),
        "(2 -2 -1 1 12 -2 3 5 4 -1 -5 -4 -8 0 4 -5 -1 0 -1 3 -1 T NIL T NIL T T T NIL 3 3 4 3 -1 0 0)"
    );
    assert!(Runtime::new().eval_source("(logbitp -1 1)").is_err());
}

#[test]
fn evaluates_common_lisp_boole_constants_as_immutable_constants() {
    assert_eq!(
        evaluate("(list boole-clr boole-set boole-and boole-orc2 (constantp 'boole-and))")
            .to_string(),
        "(0 1 6 15 T)"
    );
    assert!(Runtime::new().eval_source("(setq boole-and 0)").is_err());
}

#[test]
fn evaluates_common_lisp_quotients_gcd_and_rational_parts() {
    assert_eq!(
        evaluate(
            "(list
                    (multiple-value-bind (q r) (floor 7 3) (list q r))
                    (multiple-value-bind (q r) (floor -7 3) (list q r))
                    (multiple-value-bind (q r) (ceiling -7 3) (list q r))
                    (multiple-value-bind (q r) (truncate -7 3) (list q r))
                    (multiple-value-bind (q r) (round 5 2) (list q r))
                    (multiple-value-bind (q r) (round 7 2) (list q r))
                    (multiple-value-bind (q r) (floor -7/3) (list q r))
                    (multiple-value-bind (q r) (ceiling 7/3) (list q r))
                    (multiple-value-bind (q r) (floor 3.5 2.0) (list q r))
                    (multiple-value-bind (q r) (round 2.5) (list q r))
                    (gcd 18 -24 30) (gcd) (lcm 6 -8 15) (lcm)
                    (numerator -6/8) (denominator -6/8)
                    (numerator 7) (denominator 7))",
        )
        .to_string(),
        "((2 1) (-3 2) (-2 -1) (-2 -1) (2 1) (4 -1) (-3 2/3) (3 -2/3) (1 1.5) (2 0.5) 6 0 120 1 -3 4 7 1)"
    );
}

#[test]
fn evaluates_common_lisp_expt_across_numeric_types() {
    assert_eq!(
        evaluate(
            "(list (expt 2 10) (expt 2 -3) (expt 3/2 2)
                    (= (expt 2.0 3) 8.0) (floatp (expt 2.0 3))
                    (floatp (expt 2 1/2)) (expt 0 0))",
        )
        .to_string(),
        "(1024 1/8 9/4 T T T 1)"
    );
}

#[test]
fn evaluates_common_lisp_exp_across_numeric_types() {
    assert_eq!(
        evaluate(
            "(list (= (exp 0) 1.0) (> (exp 1) 2.7)
                    (< (exp -1) 0.4) (> (exp 1/2) 1.6)
                    (floatp (exp 0)) (= (exp #C(0 1)) (cis 1))
                    (complexp (exp #C(1 1)))
                    (> (realpart (exp #C(1 1))) 0.0)
                    (> (imagpart (exp #C(1 1))) 0.0))",
        )
        .to_string(),
        "(T T T T T T T T T)",
    );

    let arity_error = Runtime::new().eval_source("(exp)").unwrap_err();
    assert!(matches!(
        arity_error,
        RuntimeError::Arity {
            function,
            actual: 0,
            ..
        } if function == "exp"
    ));
    let type_error = Runtime::new().eval_source("(exp 'x)").unwrap_err();
    assert!(matches!(type_error, RuntimeError::Type { .. }));
}

#[test]
fn evaluates_common_lisp_log_across_numeric_types() {
    assert_eq!(
        evaluate(
            "(list (= (log 1) 0.0) (> (log 8 2) 2.99)
                    (< (log 8 2) 3.01) (> (log 100 10) 1.99)
                    (< (log 100 10) 2.01) (floatp (log 2))
                    (= (log 8 0) 0) (complexp (log -1))
                    (= (realpart (log -1)) 0.0)
                    (> (imagpart (log -1)) 3.0)
                    (< (imagpart (log -1)) 3.2)
                    (= (log #C(0 1) #C(0 -1)) -1.0)
                    (complexp (log #C(1 1))))",
        )
        .to_string(),
        "(T T T T T T T T T T T T T)",
    );

    let arity_error = Runtime::new().eval_source("(log)").unwrap_err();
    assert!(matches!(
        arity_error,
        RuntimeError::Arity {
            function,
            actual: 0,
            ..
        } if function == "log"
    ));
    let too_many_arguments = Runtime::new()
        .eval_source("(log 1 2 3)")
        .unwrap_err();
    assert!(matches!(
        too_many_arguments,
        RuntimeError::Arity {
            function,
            actual: 3,
            ..
        } if function == "log"
    ));
    let zero_error = Runtime::new().eval_source("(log 0)").unwrap_err();
    assert!(matches!(zero_error, RuntimeError::DivisionByZero));
    let type_error = Runtime::new().eval_source("(log 'x)").unwrap_err();
    assert!(matches!(type_error, RuntimeError::Type { .. }));
}

#[test]
fn evaluates_common_lisp_sin_across_numeric_types() {
    assert_eq!(
        evaluate(
            "(list (= (sin 0) 0.0) (> (sin 1) 0.84)
                    (< (sin 1) 0.85) (> (sin 1/2) 0.47)
                    (< (sin 1/2) 0.49) (floatp (sin 0))
                    (= (realpart (sin #C(0 1))) 0.0)
                    (> (imagpart (sin #C(0 1))) 1.17)
                    (< (imagpart (sin #C(0 1))) 1.18)
                    (complexp (sin #C(1 1))))",
        )
        .to_string(),
        "(T T T T T T T T T T)",
    );

    let arity_error = Runtime::new().eval_source("(sin)").unwrap_err();
    assert!(matches!(
        arity_error,
        RuntimeError::Arity {
            function,
            actual: 0,
            ..
        } if function == "sin"
    ));
    let too_many_arguments = Runtime::new()
        .eval_source("(sin 1 2)")
        .unwrap_err();
    assert!(matches!(
        too_many_arguments,
        RuntimeError::Arity {
            function,
            actual: 2,
            ..
        } if function == "sin"
    ));
    let type_error = Runtime::new().eval_source("(sin 'x)").unwrap_err();
    assert!(matches!(type_error, RuntimeError::Type { .. }));
}

#[test]
fn evaluates_common_lisp_cos_across_numeric_types() {
    assert_eq!(
        evaluate(
            "(list (= (cos 0) 1.0) (> (cos 1) 0.54)
                    (< (cos 1) 0.55) (> (cos 1/2) 0.87)
                    (< (cos 1/2) 0.88) (floatp (cos 0))
                    (> (realpart (cos #C(0 1))) 1.54)
                    (< (realpart (cos #C(0 1))) 1.55)
                    (= (imagpart (cos #C(0 1))) 0.0)
                    (complexp (cos #C(1 1))))",
        )
        .to_string(),
        "(T T T T T T T T T T)",
    );

    let arity_error = Runtime::new().eval_source("(cos)").unwrap_err();
    assert!(matches!(
        arity_error,
        RuntimeError::Arity {
            function,
            actual: 0,
            ..
        } if function == "cos"
    ));
    let too_many_arguments = Runtime::new()
        .eval_source("(cos 1 2)")
        .unwrap_err();
    assert!(matches!(
        too_many_arguments,
        RuntimeError::Arity {
            function,
            actual: 2,
            ..
        } if function == "cos"
    ));
    let type_error = Runtime::new().eval_source("(cos 'x)").unwrap_err();
    assert!(matches!(type_error, RuntimeError::Type { .. }));
}

#[test]
fn evaluates_common_lisp_tan_across_numeric_types() {
    assert_eq!(
        evaluate(
            "(list (= (tan 0) 0.0) (> (tan 1) 1.55)
                    (< (tan 1) 1.56) (> (tan 1/2) 0.54)
                    (< (tan 1/2) 0.55) (floatp (tan 0))
                    (= (realpart (tan #C(0 1))) 0.0)
                    (> (imagpart (tan #C(0 1))) 0.76)
                    (< (imagpart (tan #C(0 1))) 0.77)
                    (complexp (tan #C(1 1))))",
        )
        .to_string(),
        "(T T T T T T T T T T)",
    );

    let arity_error = Runtime::new().eval_source("(tan)").unwrap_err();
    assert!(matches!(
        arity_error,
        RuntimeError::Arity {
            function,
            actual: 0,
            ..
        } if function == "tan"
    ));
    let too_many_arguments = Runtime::new()
        .eval_source("(tan 1 2)")
        .unwrap_err();
    assert!(matches!(
        too_many_arguments,
        RuntimeError::Arity {
            function,
            actual: 2,
            ..
        } if function == "tan"
    ));
    let type_error = Runtime::new().eval_source("(tan 'x)").unwrap_err();
    assert!(matches!(type_error, RuntimeError::Type { .. }));
}

#[test]
fn evaluates_common_lisp_atan_across_numeric_types() {
    assert_eq!(
        evaluate(
            "(list (= (atan 0) 0.0) (> (atan 1) 0.78)
                    (< (atan 1) 0.79) (> (atan 1/2) 0.46)
                    (< (atan 1/2) 0.47) (floatp (atan 0))
                    (> (atan 1 1) 0.78) (< (atan 1 1) 0.79)
                    (> (atan 1 -1) 2.35) (< (atan 1 -1) 2.36)
                    (< (realpart (atan #C(0 2))) -1.57)
                    (> (realpart (atan #C(0 2))) -1.58)
                    (> (imagpart (atan #C(0 2))) 0.54)
                    (< (imagpart (atan #C(0 2))) 0.56)
                    (complexp (atan #C(1 1))))",
        )
        .to_string(),
        "(T T T T T T T T T T T T T T T)",
    );

    let arity_error = Runtime::new().eval_source("(atan)").unwrap_err();
    assert!(matches!(
        arity_error,
        RuntimeError::Arity {
            function,
            actual: 0,
            ..
        } if function == "atan"
    ));
    let too_many_arguments = Runtime::new()
        .eval_source("(atan 1 2 3)")
        .unwrap_err();
    assert!(matches!(
        too_many_arguments,
        RuntimeError::Arity {
            function,
            actual: 3,
            ..
        } if function == "atan"
    ));
    let type_error = Runtime::new().eval_source("(atan 'x)").unwrap_err();
    assert!(matches!(type_error, RuntimeError::Type { .. }));
    let complex_dividend = Runtime::new()
        .eval_source("(atan #C(1 1) 2)")
        .unwrap_err();
    assert!(matches!(complex_dividend, RuntimeError::Type { .. }));
    let singularity = Runtime::new()
        .eval_source("(atan #C(0 1))")
        .unwrap_err();
    assert!(matches!(singularity, RuntimeError::DivisionByZero));
}

#[test]
fn evaluates_common_lisp_asin_and_acos_across_numeric_types() {
    assert_eq!(
        evaluate(
            "(list (= (asin 0) 0.0) (> (asin 1/2) 0.52)
                    (< (asin 1/2) 0.53) (> (acos 0) 1.57)
                    (< (acos 0) 1.58) (> (acos 1/2) 1.04)
                    (< (acos 1/2) 1.05) (floatp (asin 0))
                    (floatp (acos 0)) (> (realpart (asin 2)) 1.57)
                    (< (realpart (asin 2)) 1.58)
                    (< (imagpart (asin 2)) -1.31)
                    (> (imagpart (asin 2)) -1.32)
                    (> (realpart (acos 2)) -0.01)
                    (< (realpart (acos 2)) 0.01)
                    (> (imagpart (acos 2)) 1.31)
                    (< (imagpart (acos 2)) 1.32)
                    (> (realpart (acos #C(0 1))) 1.57)
                    (< (realpart (acos #C(0 1))) 1.58)
                    (< (imagpart (acos #C(0 1))) -0.88)
                    (> (imagpart (acos #C(0 1))) -0.89)
                    (complexp (asin #C(1 1)))
                    (complexp (acos #C(1 1))))",
        )
        .to_string(),
        "(T T T T T T T T T T T T T T T T T T T T T T T)",
    );

    let arity_error = Runtime::new().eval_source("(asin)").unwrap_err();
    assert!(matches!(
        arity_error,
        RuntimeError::Arity {
            function,
            actual: 0,
            ..
        } if function == "asin"
    ));
    let too_many_arguments = Runtime::new().eval_source("(acos 0 1)").unwrap_err();
    assert!(matches!(
        too_many_arguments,
        RuntimeError::Arity {
            function,
            actual: 2,
            ..
        } if function == "acos"
    ));
    let asin_type_error = Runtime::new().eval_source("(asin 'x)").unwrap_err();
    assert!(matches!(asin_type_error, RuntimeError::Type { .. }));
    let acos_type_error = Runtime::new().eval_source("(acos 'x)").unwrap_err();
    assert!(matches!(acos_type_error, RuntimeError::Type { .. }));
}

#[test]
fn evaluates_common_lisp_hyperbolic_functions_across_numeric_types() {
    assert_eq!(
        evaluate(
            "(list (= (sinh 0) 0.0) (> (sinh 1/2) 0.52)
                    (< (sinh 1/2) 0.53) (> (cosh 0) 0.99)
                    (< (cosh 0) 1.01) (> (cosh 1/2) 1.12)
                    (< (cosh 1/2) 1.13) (> (tanh 1/2) 0.46)
                    (< (tanh 1/2) 0.47) (floatp (sinh 0))
                    (floatp (cosh 0)) (floatp (tanh 0))
                    (= (realpart (sinh #C(0 1))) 0.0)
                    (> (imagpart (sinh #C(0 1))) 0.84)
                    (< (imagpart (sinh #C(0 1))) 0.85)
                    (> (realpart (cosh #C(0 1))) 0.54)
                    (< (realpart (cosh #C(0 1))) 0.55)
                    (= (imagpart (cosh #C(0 1))) 0.0)
                    (= (realpart (tanh #C(0 1))) 0.0)
                    (> (imagpart (tanh #C(0 1))) 1.55)
                    (< (imagpart (tanh #C(0 1))) 1.56))",
        )
        .to_string(),
        "(T T T T T T T T T T T T T T T T T T T T T)",
    );

    let arity_error = Runtime::new().eval_source("(sinh)").unwrap_err();
    assert!(matches!(
        arity_error,
        RuntimeError::Arity {
            function,
            actual: 0,
            ..
        } if function == "sinh"
    ));
    let too_many_arguments = Runtime::new()
        .eval_source("(cosh 0 1)")
        .unwrap_err();
    assert!(matches!(
        too_many_arguments,
        RuntimeError::Arity {
            function,
            actual: 2,
            ..
        } if function == "cosh"
    ));
    let type_error = Runtime::new().eval_source("(tanh 'x)").unwrap_err();
    assert!(matches!(type_error, RuntimeError::Type { .. }));
}

#[test]
fn evaluates_common_lisp_inverse_hyperbolic_functions_across_numeric_types() {
    assert_eq!(
        evaluate(
            "(list (= (asinh 0) 0.0) (> (asinh 1/2) 0.48)
                    (< (asinh 1/2) 0.49) (floatp (asinh 0))
                    (= (acosh 1) 0.0) (> (acosh 2) 1.31)
                    (< (acosh 2) 1.32) (floatp (acosh 2))
                    (> (realpart (acosh 0)) -0.01)
                    (< (realpart (acosh 0)) 0.01)
                    (> (imagpart (acosh 0)) 1.57)
                    (< (imagpart (acosh 0)) 1.58)
                    (> (atanh 1/2) 0.54) (< (atanh 1/2) 0.55)
                    (floatp (atanh 0)) (> (realpart (atanh 2)) 0.54)
                    (< (realpart (atanh 2)) 0.56)
                    (> (imagpart (atanh 2)) 1.57)
                    (< (imagpart (atanh 2)) 1.58)
                    (= (realpart (asinh #C(0 1))) 0.0)
                    (> (imagpart (asinh #C(0 1))) 1.57)
                    (< (imagpart (asinh #C(0 1))) 1.58)
                    (complexp (asinh #C(1 1)))
                    (complexp (acosh #C(1 1)))
                    (= (realpart (atanh #C(0 1))) 0.0)
                    (> (imagpart (atanh #C(0 1))) 0.78)
                    (< (imagpart (atanh #C(0 1))) 0.79))",
        )
        .to_string(),
        "(T T T T T T T T T T T T T T T T T T T T T T T T T T T)",
    );

    let arity_error = Runtime::new().eval_source("(asinh)").unwrap_err();
    assert!(matches!(
        arity_error,
        RuntimeError::Arity {
            function,
            actual: 0,
            ..
        } if function == "asinh"
    ));
    let too_many_arguments = Runtime::new()
        .eval_source("(acosh 0 1)")
        .unwrap_err();
    assert!(matches!(
        too_many_arguments,
        RuntimeError::Arity {
            function,
            actual: 2,
            ..
        } if function == "acosh"
    ));
    let type_error = Runtime::new().eval_source("(atanh 'x)").unwrap_err();
    assert!(matches!(type_error, RuntimeError::Type { .. }));
    let singularity = Runtime::new().eval_source("(atanh 1)").unwrap_err();
    assert!(matches!(singularity, RuntimeError::DivisionByZero));
}

#[test]
fn evaluates_common_lisp_sqrt_across_exact_and_float_numbers() {
    assert_eq!(
        evaluate(
            "(list (sqrt 0) (sqrt 4) (sqrt 1/4)
                    (rationalp (sqrt 2)) (floatp (sqrt 2))
                    (= (sqrt 4.0) 2.0))",
        )
        .to_string(),
        "(0 2 1/2 NIL T T)"
    );
}

#[test]
fn evaluates_common_lisp_isqrt_integer_square_roots() {
    assert_eq!(
        evaluate(
            "(list (isqrt 0) (isqrt 1) (isqrt 15) (isqrt 16)
                    (isqrt 1000000000000000000))",
        )
        .to_string(),
        "(0 1 3 4 1000000000)"
    );

    let error = Runtime::new().eval_source("(isqrt -1)").unwrap_err();
    assert!(matches!(
        error,
        RuntimeError::InvalidForm { message, .. }
            if message == "isqrt requires a non-negative integer"
    ));
}

#[test]
fn evaluates_common_lisp_complex_numbers() {
    assert_eq!(
        evaluate(
            "(list (numberp #C(1 2)) (complexp #C(1 2)) (realp #C(1 2))
                    (typep #C(1 2) 'number) (typep #C(1 2) 'complex)
                    (= (realpart #C(1 2)) 1) (= (imagpart #C(1 2)) 2)
                    (equal (conjugate #C(1 2)) #C(1 -2))
                    (= #C(1 2) #C(1.0 2.0)) (eql #C(1 2) #C(1 2))
                    (= (realpart (+ #C(1 2) 3)) 4)
                    (= (imagpart (* #C(1 2) 2)) 4)
                    (= (realpart (sqrt -1)) 0)
                    (= (imagpart (sqrt -1)) 1)
                    (= (imagpart (expt -1 1/2)) 1))",
        )
        .to_string(),
        "(T T NIL T T T T T T T T T T T T)",
    );
}

#[test]
fn evaluates_common_lisp_numeric_not_equal() {
    assert_eq!(
        evaluate(
            "(list (/= 1) (/= 1 2) (/= 1 2 1)
                    (/= #C(1 2) #C(1 2))
                    (/= #C(1 2) #C(1 3))
                    (/= 1 1.0) (/= 1/2 2/3)
                    (/= (- (- 0 9223372036854775807) 1)
                        9223372036854775807))",
        )
        .to_string(),
        "(T T NIL NIL T NIL T T)"
    );

    let arity_error = Runtime::new().eval_source("(/=)").unwrap_err();
    assert!(matches!(
        arity_error,
        RuntimeError::Arity {
            function,
            actual: 0,
            ..
        } if function == "/="
    ));
    let type_error = Runtime::new().eval_source("(/= 1 'x)").unwrap_err();
    assert!(matches!(type_error, RuntimeError::Type { .. }));
}

#[test]
fn evaluates_common_lisp_phase() {
    assert_eq!(
        evaluate(
            "(list (phase 1) (phase -1) (phase 0) (phase -0.0)
                    (phase 1/2) (phase #C(1 1)) (phase #C(-1 1))
                    (phase #C(0 -1)) (phase #C(0 0))
                    (floatp (phase 1)))",
        )
        .to_string(),
        "(0.0 3.141592653589793 0.0 0.0 0.0 0.7853981633974483 2.356194490192345 -1.5707963267948966 0.0 T)"
    );

    let arity_error = Runtime::new().eval_source("(phase)").unwrap_err();
    assert!(matches!(
        arity_error,
        RuntimeError::Arity {
            function,
            actual: 0,
            ..
        } if function == "phase"
    ));
    let type_error = Runtime::new().eval_source("(phase 'x)").unwrap_err();
    assert!(matches!(type_error, RuntimeError::Type { .. }));
}

#[test]
fn evaluates_common_lisp_cis() {
    assert_eq!(
        evaluate(
            "(list (cis 0) (cis 1) (cis -1) (cis 1/2)
                    (complexp (cis 0)) (realpart (cis 0)) (imagpart (cis 0)))",
        )
        .to_string(),
        "(#C(1.0 0.0) #C(0.5403023058681398 0.8414709848078965) #C(0.5403023058681398 -0.8414709848078965) #C(0.8775825618903728 0.479425538604203) T 1.0 0.0)",
    );

    let arity_error = Runtime::new().eval_source("(cis)").unwrap_err();
    assert!(matches!(
        arity_error,
        RuntimeError::Arity {
            function,
            actual: 0,
            ..
        } if function == "cis"
    ));
    let type_error = Runtime::new().eval_source("(cis #C(1 1))").unwrap_err();
    assert!(matches!(type_error, RuntimeError::Type { .. }));
}

#[test]
fn evaluates_common_lisp_signum_and_rationalize() {
    assert_eq!(
        evaluate(
            "(list (signum -7) (signum 0) (signum -5/2)
                    (signum -0.0) (signum 3.5)
                    (rationalize 2) (rationalize 3/6)
                    (rationalize 0.1) (rationalize (/ 1.0 3.0))
                    (rationalp (rationalize 0.1))
                    (floatp (signum 0.0)))",
        )
        .to_string(),
        "(-1 0 -1 -0.0 1.0 2 1/2 1/10 1/3 T T)"
    );
}

#[test]
fn evaluates_common_lisp_random_state() {
    assert_eq!(
        evaluate(
            "(list (random-state-p *random-state*)
                    (random-state-p 1)
                    (typep *random-state* 'random-state)
                    (let* ((state (make-random-state nil))
                           (copy (make-random-state state)))
                      (let ((*random-state* state))
                        (equal (list (random 100) (random 100))
                               (list (random 100 copy) (random 100 copy)))))
                    (let ((value (random 1)))
                      (and (integerp value) (= value 0)))
                    (let ((value (random 1.0)))
                      (and (floatp value) (>= value 0.0) (< value 1.0))))",
        )
        .to_string(),
        "(T NIL T T T T)"
    );
}

#[test]
fn evaluates_common_lisp_float_sign() {
    assert_eq!(
        evaluate(
            "(list (float-sign 5.0) (float-sign -5.0)
                    (float-sign 0.0) (float-sign -0.0)
                    (float-sign -1.0 10.0) (float-sign 1.0 -10.0)
                    (float-sign -0.0 0.0) (float-sign 1.0 -0.0))",
        )
        .to_string(),
        "(1.0 -1.0 1.0 -1.0 -10.0 10.0 -0.0 0.0)"
    );
    assert!(Runtime::new().eval_source("(float-sign 1)").is_err());
    assert!(Runtime::new().eval_source("(float-sign 1.0 1)").is_err());
    assert!(Runtime::new().eval_source("(float-sign)").is_err());
    assert!(Runtime::new()
        .eval_source("(float-sign 1.0 2.0 3.0)")
        .is_err());
}

#[test]
fn evaluates_common_lisp_float_radix() {
    assert_eq!(
        evaluate("(list (float-radix 1.0) (float-radix -0.0))").to_string(),
        "(2 2)"
    );
    assert!(Runtime::new().eval_source("(float-radix 1)").is_err());
    assert!(Runtime::new()
        .eval_source("(float-radix 1.0 2.0)")
        .is_err());
}

#[test]
fn evaluates_common_lisp_float_digits_and_precision() {
    assert_eq!(
        evaluate(
            "(list (float-digits 1.0) (float-digits 5e-324)
                    (float-precision 1.0) (float-precision 5e-324)
                    (float-precision 0.0) (float-precision -0.0))",
        )
        .to_string(),
        "(53 53 53 1 0 0)"
    );
    assert!(Runtime::new().eval_source("(float-digits 1)").is_err());
    assert!(Runtime::new().eval_source("(float-precision 1)").is_err());
    assert!(Runtime::new().eval_source("(float-digits)").is_err());
    assert!(Runtime::new()
        .eval_source("(float-precision 1.0 2.0)")
        .is_err());
}

#[test]
fn evaluates_common_lisp_float_decode_scale_and_integer_decode() {
    assert_eq!(
        evaluate(
            "(list (multiple-value-list (decode-float 1.0))
                    (multiple-value-list (decode-float -0.0))
                    (multiple-value-list (decode-float 5e-324))
                    (multiple-value-list (integer-decode-float 1.0))
                    (multiple-value-list (integer-decode-float -0.0))
                    (multiple-value-list (integer-decode-float 5e-324))
                    (scale-float 1.5 2) (scale-float 1.5 -1)
                    (scale-float -0.0 10)
                    (scale-float 1.0 -9223372036854775807))",
        )
        .to_string(),
        "((0.5 1 1.0) (0.0 0 -1.0) (0.5 -1073 1.0) (4503599627370496 -52 1) (0 0 -1) (1 -1074 1) 6.0 0.75 -0.0 0.0)"
    );
    assert!(Runtime::new().eval_source("(decode-float 1)").is_err());
    assert!(Runtime::new()
        .eval_source("(integer-decode-float 1)")
        .is_err());
    assert!(Runtime::new().eval_source("(scale-float 1 2)").is_err());
    assert!(Runtime::new()
        .eval_source("(scale-float 1.0 2.0)")
        .is_err());
    assert!(Runtime::new().eval_source("(decode-float)").is_err());
    assert!(Runtime::new()
        .eval_source("(integer-decode-float 1.0 2.0)")
        .is_err());
}

#[test]
fn evaluates_common_lisp_byte_operations() {
    assert_eq!(
        evaluate(
            "(let ((spec (byte 4 4)))
               (list (byte-size spec) (byte-position spec)
                     (byte-size (byte 0 100))
                     (byte-position (byte 0 100))
                     (ldb (byte 4 0) 45)
                     (ldb (byte 4 4) -42)
                     (ldb (byte 4 100) -1)
                     (ldb (byte 4 100) 1)
                     (ldb-test (byte 4 0) 8)
                     (ldb-test (byte 4 0) 0)
                     (ldb-test (byte 0 100) -1)
                     (ldb-test (byte 4 100) -1)
                     (mask-field (byte 4 4) 45)
                     (mask-field (byte 4 0) -2)
                     (dpb 3 (byte 4 4) 0)
                     (dpb 3 (byte 4 0) 16)
                     (deposit-field -1 (byte 4 0) 0)
                     (deposit-field 0 (byte 2 1) -3)
                     (ldb (byte 0 100) -1)
                     (dpb 123 (byte 0 100) -7)))",
        )
        .to_string(),
        "(4 4 0 100 13 13 15 0 T NIL NIL T 32 14 48 19 15 -7 0 -7)"
    );
    assert!(Runtime::new().eval_source("(byte -1 0)").is_err());
    assert!(Runtime::new().eval_source("(byte 4 -1)").is_err());
    assert!(Runtime::new()
        .eval_source("(ldb (byte 4 0) 1 2)")
        .is_err());
    assert!(Runtime::new()
        .eval_source("(ldb (byte 64 0) -1)")
        .is_err());
    assert!(Runtime::new()
        .eval_source("(dpb 1 (byte 1 63) 0)")
        .is_err());
    assert!(Runtime::new()
        .eval_source("(mask-field (byte 1 63) -1)")
        .is_err());
    assert!(Runtime::new().eval_source("(byte-size 1)").is_err());
    assert!(Runtime::new().eval_source("(byte 1)").is_err());
    assert!(Runtime::new().eval_source("(byte 1 2 3)").is_err());
    assert!(Runtime::new()
        .eval_source("(byte-size (list 1 2 3))")
        .is_err());
    assert!(Runtime::new()
        .eval_source("(byte-position (byte 1))")
        .is_err());
    assert!(Runtime::new().eval_source("(ldb 1 0)").is_err());
    assert!(Runtime::new()
        .eval_source("(ldb-test (byte 1 0) 1 2)")
        .is_err());
    assert!(Runtime::new()
        .eval_source("(mask-field (byte 1 0) 1.0)")
        .is_err());
    assert!(Runtime::new().eval_source("(dpb 1 0 0)").is_err());
    assert!(Runtime::new()
        .eval_source("(dpb 1.0 (byte 1 0) 0)")
        .is_err());
    assert!(Runtime::new()
        .eval_source("(deposit-field 1 (byte 1 0))")
        .is_err());
    assert!(Runtime::new()
        .eval_source("(deposit-field 1 (byte 1 0) 0 1)")
        .is_err());
}

#[test]
fn evaluates_common_lisp_float_exponent_markers() {
    assert_eq!(
        evaluate(
            "(list (floatp 1.0s0) (floatp 1.0f0) (floatp 1.0d0) (floatp 1.0l0)
                    (= 1.0s0 1.0e0) (= 1f2 100.0) (= -1.5d-1 -0.15))",
        )
        .to_string(),
        "(T T T T T T T)",
    );
}

#[test]
fn evaluates_common_lisp_float_and_rational_conversion() {
    assert_eq!(
        evaluate(
            "(list (float 3) (float 1/2) (float -0.0) (float 1.25 0.0)
                    (rational 3) (rational 3/6) (rational 1.5)
                    (rational 0.1) (rationalp (rational 0.1)))",
        )
        .to_string(),
        "(3.0 0.5 -0.0 1.25 3 1/2 3/2 3602879701896397/36028797018963968 T)"
    );
}

#[test]
fn evaluates_basic_format_directives() {
    assert_eq!(
        evaluate(r#"(format nil "~A/~S" "text" "text")"#).to_string(),
        r#""text/\"text\"""#,
    );
    assert_eq!(
        evaluate(r#"(list (format nil "~:S" nil) (format nil "~:S" 'foo))"#).to_string(),
        r#"("()" "FOO")"#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~D/~B/~O/~X" -12 10 8 255)"#).to_string(),
        r#""-12/1010/10/FF""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~C/~~/~%end" #\!)"#).to_string(),
        r#""!/~/\nend""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "line~&next")"#).to_string(),
        r#""line\nnext""#,
    );
    assert_eq!(
        evaluate(
            r#"(let ((output (make-string-output-stream)))
               (format output "head")
               (format output "~&next")
               (get-output-stream-string output))"#,
        )
        .to_string(),
        r#""head\nnext""#,
    );
    assert_eq!(evaluate(r#"(format t "")"#).to_string(), "NIL");
    assert_eq!(
        evaluate(r#"(format nil "~?/~*" "~A ~D" '(foo 7) 99 100)"#).to_string(),
        r#""FOO 7/""#,
    );
}

#[test]
fn evaluates_format_percent_always_emits_newlines() {
    assert_eq!(
        evaluate(
            r#"(list (length (format nil "~%"))
                           (length (format nil "a~%~%b")))"#
        )
        .to_string(),
        "(1 4)",
    );
}

#[test]
fn evaluates_format_newline_suppression_directive() {
    assert_eq!(
        evaluate(
            "(list (format nil \"a~\n   b\")
                         (format nil \"a~\n   b~\n c\"))",
        )
        .to_string(),
        "(\"ab\" \"abc\")",
    );
}

#[test]
fn evaluates_format_argument_repositioning_directive() {
    assert_eq!(
        evaluate(
            r#"(list (format nil "~A|~:*~A" "x")
                         (format nil "~A|~@*~A" "x" "y")
                         (format nil "~A|~1@*~A|~A" "x" "y" "z"))"#,
        )
        .to_string(),
        r#"("x|x" "x|x" "x|y|z")"#,
    );
}

#[test]
fn evaluates_plural_format_directive() {
    assert_eq!(
        evaluate(r#"(format nil "~P|~P|~@P|~@P" 1 2 1 2)"#).to_string(),
        r#""|s|y|ies""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~D~:P|~D~:@P" 1 2)"#).to_string(),
        r#""1|2ies""#,
    );
}

#[test]
fn evaluates_dollar_float_format_directive() {
    assert_eq!(
        evaluate(r#"(format nil "~$|~,3$|~,,8$|~2,4,10,'*$|~@$|~,,10:@$" 12.3456 12.3456 12.3 12.3 12.3 12.3)"#)
            .to_string(),
        r#""12.35|012.35|   12.30|***0012.30|+12.30|+    12.30""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~0$|~0@$|~0:$" 12.3 12.3 -12.3)"#).to_string(),
        r#""12.|+12.|-12.""#,
    );
}

#[test]
fn evaluates_general_float_format_directive() {
    assert_eq!(
        evaluate(r#"(format nil "~G|~,3G|~10,3G|~10,3G|~10,3,0G|~10,3,1G|~10,3,2G|~@G" 12.3456 1.25 12.3456 0.0123456 12.3456 12.3456 12.3456 1.25)"#)
            .to_string(),
        r#""12.3456    |1.25    |  12.3    |  1.235e-2|    12.3  |   12.3   |  12.3    |+1.25    ""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~10,3,2G" 0.0123456)"#).to_string(),
        r#"" 1.235e-02""#,
    );
    assert_eq!(
        evaluate(
            r#"(list (format nil "~10,3,-3G" 12.3456)
                       (format nil "~10,3,-3G" 0.0123456))"#,
        )
        .to_string(),
        r#"("       12.3" "  1.235e-2")"#,
    );
}

#[test]
fn evaluates_format_tabulation_modifiers() {
    assert_eq!(
        evaluate(
            r#"(list (format nil "x~T|")
                       (format nil "x~:T|")
                       (format nil "x~@T|")
                       (format nil "x~:@T|")
                       (format nil "x~3,4T|")
                       (format nil "x~3,4:T|")
                       (format nil "x~3,4@T|")
                       (format nil "x~3,4:@T|"))"#,
        )
        .to_string(),
        r#"("x |" "x|" "x |" "x|" "x  |" "x|" "x   |" "x|")"#,
    );
}

#[test]
fn evaluates_format_write_directive() {
    assert_eq!(
        evaluate(
            r#"(list (format nil "~W" '("abc"))
                       (format nil "~:W" "abc")
                       (format nil "~@W" "abc")
                       (format nil "~:@W" "abc"))"#,
        )
        .to_string(),
        r#"("(\"abc\")" "\"abc\"" "\"abc\"" "\"abc\"")"#,
    );
}

#[test]
fn evaluates_fixed_float_format_directive() {
    assert_eq!(
        evaluate(r#"(format nil "~F|~,2F|~10,2F|~@F|~4,2,,'*F" 1.25 1.25 1.25 1.25 123.4)"#)
            .to_string(),
        r#""1.25|1.25|      1.25|+1.25|****""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~,0F" 1.25)"#).to_string(),
        r#""1.""#
    );
    assert_eq!(evaluate(r#"(format nil "~F" 3)"#).to_string(), r#""3.0""#);
    assert_eq!(
        evaluate(
            r#"(list (format nil "~3F" 1.25)
                       (format nil "~1F" 0.0)
                       (format nil "~1F" 1.0)
                       (format nil "~2F" 123.0)
                       (format nil "~,2F" 1.125)
                       (format nil "~,2F" 1.375))"#,
        )
        .to_string(),
        r#"("1.3" ".0" "1." "123." "1.13" "1.38")"#,
    );
}

#[test]
fn evaluates_exponential_float_format_directive() {
    assert_eq!(
        evaluate(r#"(format nil "~E|~,2E|~10,2E|~@E" 1.25 1.25 1.25 1.25)"#).to_string(),
        r#""1.25e+0|1.25e+0|   1.25e+0|+1.25e+0""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~,2,3E|~,2,,0E|~,2,,-1E" 0.0125 637.5 637.5)"#).to_string(),
        r#""1.25e-002|0.64e+3|0.06e+4""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~6,2,,,'*E" 123.4)"#).to_string(),
        r#""******""#,
    );
}

#[test]
fn evaluates_parameterized_format_directives() {
    assert_eq!(
        evaluate(r#"(format nil "~10A|~10@A|~10,'0D|~:D|~@D" "x" "y" 42 1234567 8)"#).to_string(),
        r#""x         |         y|0000000042|1,234,567|+8""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~2{~A~}" '(a b c))"#).to_string(),
        r#""AB""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~vA/~8T" 5 "x")"#).to_string(),
        r#""x    /  ""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~R/~:R/~@R/~W" 42 42 4 '(a 1))"#).to_string(),
        r#""forty-two/forty-second/IV/(A 1)""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~@R/~:@R" 44 44)"#).to_string(),
        r#""XLIV/XXXXIIII""#,
    );
    assert!(Runtime::new()
        .eval_source(r#"(format nil "~@R" -42)"#)
        .is_err());
    assert!(Runtime::new()
        .eval_source(r#"(format nil "~@R" 0)"#)
        .is_err());
    assert!(Runtime::new()
        .eval_source(r#"(format nil "~@R" 4000)"#)
        .is_err());
    assert_eq!(
        evaluate(r#"(format nil "~:C/~@C" #\Newline #\Space)"#).to_string(),
        r#""Newline/#\\Space""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "a~2%b")"#).to_string(),
        r#""a\n\nb""#,
    );
}

#[test]
fn evaluates_format_iteration_directives() {
    assert_eq!(
        evaluate(r#"(format nil "~{~A/~A~}" '(one 1 two 2))"#).to_string(),
        r#""ONE/1TWO/2""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~:{~A=~D;~}" '((x 1) (y 2)))"#).to_string(),
        r#""X=1;Y=2;""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~@{~A~}" 'one 'two 'three)"#).to_string(),
        r#""ONETWOTHREE""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~{~{~A~}~}" '((one two) (three four)))"#).to_string(),
        r#""ONETWOTHREEFOUR""#,
    );
}

#[test]
fn evaluates_format_recursive_processing_directive() {
    assert_eq!(
        evaluate(
            r#"(list (format nil "~? ~D" "<~A ~D>" '("Foo" 5) 7)
                       (format nil "~@? ~D" "<~A ~D>" "Foo" 5 7)
                       (format nil "~@? ~D" "<~A ~D>" "Foo" 5 14 7))"#,
        )
        .to_string(),
        r#"("<Foo 5> 7" "<Foo 5> 7" "<Foo 5> 14")"#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~:{ ~@?~:^ ...~} " '(("a") ("b")))"#,).to_string(),
        r#"" a ... b ""#,
    );
}

#[test]
fn evaluates_format_justification_directive() {
    assert_eq!(
        evaluate(
            r#"(list (format nil "~15<~S~;~^~S~;~^~S~>" 'foo)
                       (format nil "~15<~S~;~^~S~;~^~S~>" 'foo 'bar)
                       (format nil "~15<~S~;~^~S~;~^~S~>" 'foo 'bar 'baz)
                       (format nil "~10<~A~;~A~>" "a" "b")
                       (format nil "~10:<~A~;~A~>" "a" "b")
                       (format nil "~10@<~A~;~A~>" "a" "b")
                       (format nil "~10:@<~A~;~A~>" "a" "b")
                       (format nil "~10,2,1<~A~;~A~>" "a" "b")
                       (format nil "~10<~A~;~A~1,1^~>~A" "a" "b" "c"))"#,
        )
        .to_string(),
        r#"("            FOO" "FOO         BAR" "FOO   BAR   BAZ" "a        b" "    a    b" "a    b    " "  a   b   " "a        b" "         ac")"#,
    );
}

#[test]
fn evaluates_format_conditional_newline_directive() {
    assert_eq!(
        evaluate(
            r#"(list (format nil "a~_b")
                       (format nil "a~:_b")
                       (format nil "a~@_b")
                       (format nil "a~:@_b")
                       (format nil "a~_~A" 'b))"#,
        )
        .to_string(),
        r#"("ab" "ab" "ab" "ab" "aB")"#,
    );
}

#[test]
fn evaluates_format_indentation_directive() {
    assert_eq!(
        evaluate(
            r#"(list (format nil "a~I b")
                       (format nil "a~1I b")
                       (format nil "a~:I b")
                       (format nil "a~1:I b")
                       (format nil "a~-1I b")
                       (format nil "a~-1:I b")
                       (format nil "a~I~A" 'b))"#,
        )
        .to_string(),
        r#"("a b" "a b" "a b" "a b" "a b" "a b" "aB")"#,
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
fn evaluates_format_case_conversion_directive() {
    assert_eq!(
        evaluate(
            r#"(list (format nil "~(~A~)" "MiXeD Words")
                       (format nil "~:(~A~)" "MiXeD Words")
                       (format nil "~@(~A~)" "MiXeD Words")
                       (format nil "~:@(~A~)" "MiXeD Words")
                       (format nil "~(~A ~A~)" "MiXeD" "WORDS")
                       (format nil "~:(~A ~A~)" "MiXeD" "WORDS")
                       (format nil "~:@(~A ~A~)" "MiXeD" "WORDS"))"#,
        )
        .to_string(),
        r#"("mixed words" "Mixed Words" "Mixed words" "MIXED WORDS" "mixed words" "Mixed Words" "MIXED WORDS")"#,
    );
}

#[test]
fn evaluates_format_escape_upward_directive() {
    assert_eq!(
        evaluate(r#"(format nil "~{~A~^, ~}" '(one two three))"#).to_string(),
        r#""ONE, TWO, THREE""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "done~^ignored")"#).to_string(),
        r#""done""#,
    );
    assert_eq!(evaluate(r#"(format nil "a~1,1^b")"#).to_string(), r#""a""#,);
    assert_eq!(evaluate(r#"(format nil "a~1,2^b")"#).to_string(), r#""ab""#,);
    assert_eq!(
        evaluate(r#"(format nil "~:{~A~:^, ~}" '((a) (b) (c)))"#).to_string(),
        r#""A, B, C""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~[a~^b~;c~]" 0)"#).to_string(),
        r#""a""#,
    );
}

#[test]
fn evaluates_format_choice_directives() {
    assert_eq!(
        evaluate(r#"(format nil "~[zero~;one~;two~]" 1)"#).to_string(),
        r#""one""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~[zero~;one~:;other~]" 9)"#).to_string(),
        r#""other""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~[zero~;~{~A~}~]" 1 '(a b))"#).to_string(),
        r#""AB""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~:[false~;true~]" nil)"#).to_string(),
        r#""false""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~:[false~;true~]" t)"#).to_string(),
        r#""true""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~@[yes~]" t)"#).to_string(),
        r#""yes""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~@[yes~]" nil)"#).to_string(),
        r#""""#,
    );
    assert_eq!(
        evaluate(
            r#"(list (format nil "~@[~A~]~A" t 'x)
                       (format nil "~@[~A~]~A" nil 'x)
                       (format nil "~@[yes~]~A" t 'x)
                       (format nil "~@[yes~]~A" nil 'x))"#,
        )
        .to_string(),
        r#"("TX" "X" "yesT" "X")"#,
    );
}

#[test]
fn evaluates_format_choice_parameters() {
    assert_eq!(
        evaluate(
            r#"(list (format nil "~2[zero~;one~;two~]~A" 'x)
                       (format nil "~v[zero~;one~;two~]~A" 2 'x)
                       (format nil "~#[zero~;one~;two~;many~]~A" 'x 'y))"#,
        )
        .to_string(),
        r#"("twoX" "twoX" "twoX")"#,
    );
}

#[test]
fn evaluates_write_to_string() {
    assert_eq!(
        evaluate("(write-to-string '(a 1))").to_string(),
        r#""(A 1)""#,
    );
    assert_eq!(
        evaluate("(write-to-string \"abc\")").to_string(),
        r#""\"abc\"""#,
    );
    assert_eq!(
        evaluate("(write-to-string #(1 2))").to_string(),
        r##""#(1 2)""##,
    );
}

#[test]
fn evaluates_write_escape_options() {
    assert_eq!(
        evaluate(
            r#"(list (write-to-string "abc")
                       (write-to-string "abc" :escape nil)
                       (write-to-string '("abc") :escape nil))"#,
        )
        .to_string(),
        r#"("\"abc\"" "abc" "(abc)")"#,
    );
}

#[test]
fn evaluates_print_variants_to_string_stream() {
    assert_eq!(
        evaluate(
            r#"(let ((output (make-string-output-stream)))
               (list (princ "a" output)
                     (prin1 "a" output)
                     (print 1 output)
                     (get-output-stream-string output)))"#,
        )
        .to_string(),
        r#"("a" "a" 1 "a\"a\"\n1\n")"#,
    );
}

#[test]
fn evaluates_write_to_stream() {
    assert_eq!(
        evaluate(
            r#"(let ((output (make-string-output-stream)))
               (list (princ "abc" output)
                     (prin1 "abc" output)
                     (write "abc" :stream output :escape nil)
                     (write "abc" :stream output :escape t)
                     (get-output-stream-string output)))"#,
        )
        .to_string(),
        r#"("abc" "abc" "abc" "abc" "abc\"abc\"abc\"abc\"")"#,
    );
}

#[test]
fn evaluates_read_from_string() {
    assert_eq!(
        evaluate(
            r#"(multiple-value-bind (value position)
                   (read-from-string "  (a 1) trailing")
                 (list value position))"#,
        )
        .to_string(),
        "((A 1) 8)",
    );
    assert_eq!(
        evaluate(
            r#"(multiple-value-bind (value position)
                   (read-from-string "42 rest")
                 (list value position))"#,
        )
        .to_string(),
        "(42 3)",
    );
    assert_eq!(
        evaluate(
            r#"(multiple-value-bind (value position)
                   (read-from-string "" nil :eof)
                 (list value position))"#,
        )
        .to_string(),
        "(:EOF 0)",
    );
}

#[test]
fn evaluates_read_from_string_options() {
    assert_eq!(
        evaluate(
            r#"(list
                   (multiple-value-bind (value position)
                       (read-from-string "  (a)  b" nil :eof :start 1 :end 8)
                     (list value position))
                   (multiple-value-bind (value position)
                       (read-from-string "  (a)  b" nil :eof :start 1 :end 8
                                         :preserve-whitespace t)
                     (list value position))
                   (multiple-value-bind (value position)
                       (read-from-string "  (a)  b" nil :eof :start 2 :end 5)
                     (list value position)))"#,
        )
        .to_string(),
        "(((A) 6) ((A) 5) ((A) 5))",
    );
}

#[test]
fn evaluates_read_from_string_stream() {
    assert_eq!(
        evaluate(
            r#"(let ((input (make-string-input-stream "  (a 1) 42  ")))
               (list (read input)
                     (read input)
                     (read-preserving-whitespace input nil :eof)
                     (read input nil :eof)))"#,
        )
        .to_string(),
        "((A 1) 42 :EOF :EOF)",
    );
}

#[test]
fn evaluates_default_input_stream_operations() {
    assert_eq!(
        evaluate(
            r#"(list
                 (let ((*standard-input* (make-string-input-stream "AB")))
                   (list (read-char)
                         (read-char nil nil :eof)
                         (read-char t nil :eof)))
                 (let ((*standard-input* (make-string-input-stream "x")))
                   (list (peek-char) (read-char)))
                 (let ((*standard-input* (make-string-input-stream "a")))
                   (list (read-char)
                         (unread-char #\a)
                         (read-char)))
                 (let ((*standard-input* (make-string-input-stream "a")))
                   (list (listen) (clear-input) (listen)))
                 (let ((*standard-input* (make-string-input-stream "foo")))
                   (multiple-value-list (read-line)))
                 (let ((*standard-input* (make-string-input-stream "abc"))
                       (buffer (vector #\_ #\_ #\_ #\_)))
                   (list (read-sequence buffer :start 1 :end 3)
                         (elt buffer 0)
                         (elt buffer 1)
                         (elt buffer 2)
                         (elt buffer 3)))
                 (let ((*standard-input* (make-string-input-stream "(+ 1 2)")))
                   (eval (read))))"#,
        )
        .to_string(),
        r#"((#\A #\B :EOF) (#\x #\x) (#\a NIL #\a) (T NIL T) ("foo" T) (3 #\_ #\a #\b #\_) 3)"#,
    );
}

#[test]
fn evaluates_read_whitespace_consumption() {
    assert_eq!(
        evaluate(
            r#"(let ((read-input (make-string-input-stream "(a)  b"))
                     (preserve-input (make-string-input-stream "(a)  b")))
                 (list (read read-input)
                       (read-char read-input)
                       (read read-input)
                       (read-preserving-whitespace preserve-input)
                       (read-char preserve-input)
                       (read preserve-input)))"#,
        )
        .to_string(),
        r#"((A) #\SPACE B (A) #\SPACE B)"#,
    );
}

#[test]
fn evaluates_character_stream_options_and_eof() {
    assert_eq!(
        evaluate(
            r#"(list
                 (let ((input (make-string-input-stream "a")))
                   (list (read-char input nil :eof)
                         (read-char input nil :eof)))
                 (let ((input (make-string-input-stream "  a ")))
                   (list (peek-char t input nil :eof)
                         (read-char input nil :eof)
                         (peek-char nil input nil :eof)
                         (read-char input nil :eof)
                         (read-char input nil :eof)))
                 (let ((input (make-string-input-stream "acb")))
                   (list (peek-char #\b input nil :eof)
                         (read-char input nil :eof)))
                 (let ((input (make-string-input-stream "a")))
                   (list (multiple-value-list (read-line input nil :eof))
                         (multiple-value-list (read-line input nil :eof))))
                 (let ((input (make-string-input-stream (format nil "abc~%def"))))
                   (list (multiple-value-list (read-line input nil :eof))
                         (multiple-value-list (read-line input nil :eof))
                         (multiple-value-list (read-line input nil :eof)))))"#,
        )
        .to_string(),
        r#"((#\a :EOF) (#\a #\a #\SPACE #\SPACE :EOF) (#\b #\b) (("a" T) (:EOF T)) (("abc" NIL) ("def" T) (:EOF T)))"#,
    );
}

#[test]
fn evaluates_read_sequence_into_vector() {
    assert_eq!(
        evaluate(
            r#"(list
                 (let ((input (make-string-input-stream "abc"))
                       (buffer (vector #\_ #\_ #\_)))
                   (list (read-sequence buffer input)
                         (elt buffer 0)
                         (elt buffer 1)
                         (elt buffer 2)))
                 (let ((input (make-string-input-stream "abc"))
                       (buffer (vector #\_ #\_ #\_ #\_)))
                   (list (read-sequence buffer input :start 1 :end 3)
                         (elt buffer 0)
                         (elt buffer 1)
                         (elt buffer 2)
                         (elt buffer 3)))
                 (let ((input (make-string-input-stream "a"))
                       (buffer (vector #\_ #\_ #\_)))
                   (list (read-sequence buffer input)
                         (elt buffer 0)
                         (elt buffer 1)
                         (elt buffer 2)))
                 (let ((*standard-input* (make-string-input-stream "ab"))
                       (buffer (vector #\_ #\_)))
                   (list (read-sequence buffer)
                         (elt buffer 0)
                         (elt buffer 1)))
                 (let ((input (make-string-input-stream "abcd"))
                       (buffer (vector #\_ #\_ #\_ #\_)))
                   (list (read-sequence buffer input :start 2)
                         (elt buffer 0)
                         (elt buffer 1)
                         (elt buffer 2)
                         (elt buffer 3)))
                 (let ((input (make-string-input-stream "abcd"))
                       (buffer (vector #\_ #\_ #\_ #\_)))
                   (list (read-sequence buffer input :end 2)
                         (elt buffer 0)
                         (elt buffer 1)
                         (elt buffer 2)
                         (elt buffer 3))))"#,
        )
        .to_string(),
        "((3 #\\a #\\b #\\c) (3 #\\_ #\\a #\\b #\\_) (1 #\\a #\\_ #\\_) (2 #\\a #\\b) (4 #\\_ #\\_ #\\a #\\b) (2 #\\a #\\b #\\_ #\\_))"
    );
}

#[test]
fn evaluates_sequence_operations_and_type_predicates() {
    assert_eq!(
        evaluate("(list (first '(a b c)) (rest '(a b c)) (nth 1 '(a b c)) (elt \"abc\" 1) (subseq '(a b c d) 1 3) (subseq \"abcd\" 1 3) (member 'b '(a b c)) (assoc 'b '((a 1) (b 2))) (getf '(:a 1 :b 2) :b) (length \"abc\"))").to_string(),
        "(A (B C) B #\\b (B C) \"bc\" (B C) (B 2) 2 3)"
    );
    assert_eq!(
        evaluate(
            "(let ((array (make-array 4 :initial-contents '(1 2 3 4))))
               (let ((slice (subseq array 1 3)))
                 (list slice (array-dimensions slice) (array-element-type slice))))",
        )
        .to_string(),
        "(#<ARRAY [2]> (2) T)"
    );
    assert_eq!(
        evaluate("(list (typep 1 'integer) (typep \"abc\" 'sequence) (characterp #\\a) (keywordp :x) (vectorp #(1 2)) (endp nil) (endp '(1)))").to_string(),
        "(T T T T T T NIL)"
    );
}

#[test]
fn evaluates_compound_type_designators() {
    assert_eq!(
        evaluate(
            "(list
                (typep 3 '(or string (integer 0 5)))
                (typep 7 '(and integer (not (member 4 5))))
                (typep 4 '(member 3 4 5))
                (typep 4 '(eql 4))
                (typep 3 '(mod 4))
                (typep 3 '(unsigned-byte 4))
                (typep -8 '(signed-byte 4))
                (typep '(1 2) '(cons integer list))
                (typep #(1 2) '(vector integer 2))
                (typep #(1 2) '(simple-vector 2))
                (typep #(0 1) '(bit-vector 2))
                (typep #(1 2) '(array integer 1))
                (typep #(1 2) '(array integer (2)))
                (typep #(0 2) 'bit-vector)
                (the (or integer string) 7)
                (the (vector integer 2) #(1 2)))",
        )
        .to_string(),
        "(T T T T T T T T T T NIL T T NIL 7 #(1 2))"
    );
}

#[test]
fn evaluates_subtypep() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
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
                   (multiple-value-list (subtypep 'type-error 'condition))
                   (multiple-value-list (subtypep 'type-error 'error))
                   (multiple-value-list (subtypep 'type-error 'serious-condition))
                   (multiple-value-list (subtypep 'simple-error 'condition))
                   (multiple-value-list (subtypep 'simple-error 'error))
                   (multiple-value-list (subtypep 'simple-error 'serious-condition))
                   (multiple-value-list (subtypep 'simple-error 'simple-condition))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "((T T) (T T) (NIL T) (T T) (T T) (T T) (T T) (T T) (T T) (T T) (T T) (T T) (T T))"
    );
}

#[test]
fn evaluates_standard_condition_subtypep_hierarchy() {
    let cases = [
        ("serious-condition", "condition"),
        ("error", "condition"),
        ("error", "serious-condition"),
        ("warning", "condition"),
        ("simple-condition", "condition"),
        ("simple-warning", "condition"),
        ("simple-warning", "warning"),
        ("simple-warning", "simple-condition"),
        ("arithmetic-error", "condition"),
        ("arithmetic-error", "error"),
        ("arithmetic-error", "serious-condition"),
        ("division-by-zero", "condition"),
        ("division-by-zero", "error"),
        ("division-by-zero", "serious-condition"),
        ("division-by-zero", "arithmetic-error"),
        ("simple-type-error", "condition"),
        ("simple-type-error", "error"),
        ("simple-type-error", "serious-condition"),
        ("simple-type-error", "simple-condition"),
        ("simple-type-error", "type-error"),
        ("program-error", "condition"),
        ("program-error", "error"),
        ("program-error", "serious-condition"),
        ("package-error", "condition"),
        ("package-error", "error"),
        ("package-error", "serious-condition"),
        ("reader-error", "condition"),
        ("reader-error", "error"),
        ("reader-error", "serious-condition"),
        ("file-error", "condition"),
        ("file-error", "error"),
        ("file-error", "serious-condition"),
        ("unbound-variable", "condition"),
        ("unbound-variable", "error"),
        ("unbound-variable", "serious-condition"),
        ("control-error", "condition"),
        ("control-error", "error"),
        ("control-error", "serious-condition"),
    ];

    for (subtype, supertype) in cases {
        let values = Runtime::new()
            .eval_source(&format!(
                "(multiple-value-list (subtypep '{subtype} '{supertype}))"
            ))
            .unwrap();
        assert_eq!(values.len(), 1, "{subtype} vs {supertype}");
        assert_eq!(
            values[0].to_string(),
            "(T T)",
            "{subtype} should be a subtype of {supertype}"
        );
    }
}

#[test]
fn evaluates_sequence_construction_and_coercion() {
    assert_eq!(
        evaluate(
            "(list (make-sequence 'list 3)
                    (make-sequence 'vector 2 :initial-element 7)
                    (make-sequence 'string 3 :initial-element #\\x)
                    (coerce '(1 2) 'vector)
                    (coerce #(1 2) 'list)
                    (coerce '(#\\a #\\b) 'string)
                    (coerce 'foo 'string)
                    (simple-string-p \"abc\"))",
        )
        .to_string(),
        "((NIL NIL NIL) #(7 7) \"xxx\" #(1 2) (1 2) \"ab\" \"FOO\" T)"
    );
    assert_eq!(
        evaluate(
            "(let ((array (make-array 3 :initial-contents '(4 5 6))))
               (list (length array)
                     (elt array 1)
                     (typep array 'sequence)
                     (typep (coerce array 'sequence) 'sequence)))",
        )
        .to_string(),
        "(3 5 T T)"
    );
}

#[test]
fn evaluates_parse_integer() {
    assert_eq!(
        evaluate(
            "(list
                (multiple-value-bind (value position)
                    (parse-integer \"  -1x\" :junk-allowed t)
                  (list value position))
                (multiple-value-bind (value position)
                    (parse-integer \"xx42yy\" :start 2 :end 4)
                  (list value position))
                (parse-integer \"ff\" :radix 16)
                (multiple-value-bind (value position)
                    (parse-integer \"no-integer\" :junk-allowed t)
                  (list value position)))",
        )
        .to_string(),
        "((-1 4) (42 4) 255 (NIL 0))"
    );
}

#[test]
fn evaluates_basic_clos_instances_and_accessors() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defclass point ()
                   ((x :initarg :x :accessor point-x)
                    (y :initarg :y :accessor point-y)))
                 (let ((point (make-instance 'point :x 2 :y 3)))
                   (list (slot-value point 'x)
                         (point-x point)
                         (point-y point)
                         (slot-exists-p point 'x)
                         (slot-boundp point 'y)
                         (typep point 'point)
                         (class-name (class-of point))
                         (class-name (find-class 'point)))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(2 2 3 T T T POINT POINT)");
}

#[test]
fn evaluates_clos_with_slots_and_accessors() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(progn
                 (defclass ws-point ()
                   ((x :initarg :x :accessor ws-point-x)
                    (y :initarg :y :accessor ws-point-y)))
                 (let ((point (make-instance 'ws-point :x 2 :y 3)))
                   (with-slots ((x xx) y) point
                     (setf xx 5 y 7)
                     (with-accessors ((ax ws-point-x) (ay ws-point-y)) point
                       (list xx y ax ay
                             (progn (setf ax 11) ax)
                             (ws-point-x point)
                             (ws-point-y point))))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(5 7 5 7 11 11 7)");

    assert!(runtime
        .eval_source("(with-accessors (x) object x)")
        .is_err());
}

#[test]
fn evaluates_clos_accessor_function_designator() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defclass accessor-designator ()
                   ((value :initarg :value :accessor accessor-designator-value)))
                 (let ((object (make-instance 'accessor-designator :value 3)))
                   (list
                     (funcall #'(setf accessor-designator-value) 9 object)
                     (accessor-designator-value object)
                     (setf (accessor-designator-value object) 11)
                     (accessor-designator-value object))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(9 9 11 11)");
}

#[test]
fn evaluates_clos_slot_type_options() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(progn
                 (defclass typed-slot-eval ()
                   ((value :initarg :value
                           :type (or integer null)
                           :accessor typed-slot-eval-value)))
                 (let ((object (make-instance 'typed-slot-eval :value 3)))
                   (list
                     (typed-slot-eval-value object)
                     (setf (typed-slot-eval-value object) 4)
                     (setf (slot-value object 'value) nil)
                     (slot-value object 'value))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(3 4 NIL NIL)");

    assert!(runtime
        .eval_source(r#"(make-instance 'typed-slot-eval :value "bad")"#,)
        .is_err());
    assert!(runtime
        .eval_source(
            r#"(let ((object (make-instance 'typed-slot-eval :value 1)))
                 (setf (typed-slot-eval-value object) "bad"))"#,
        )
        .is_err());
    assert!(runtime
        .eval_source(
            r#"(let ((object (make-instance 'typed-slot-eval :value 1)))
                 (setf (slot-value object 'value) "bad"))"#,
        )
        .is_err());
}

#[test]
fn evaluates_clos_slot_initialization_options() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defclass defaults ()
                   ((x :initform 7 :reader defaults-x)
                    (y :initarg :y :writer set-defaults-y)
                    (z :initarg nil)))
                 (let ((object (make-instance 'defaults :y 3)))
                   (set-defaults-y 9 object)
                   (list (defaults-x object)
                         (slot-value object 'y)
                         (slot-boundp object 'z)
                         (not (ignore-errors (make-instance 'defaults :x 1))))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(7 9 NIL T)");
}

#[test]
fn evaluates_clos_class_allocated_slots() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defclass counter ()
                   ((value :allocation :class :initarg :value
                           :initform 0 :accessor counter-value)))
                 (defclass child-counter (counter) ())
                 (let ((counter (make-instance 'counter))
                       (child (make-instance 'child-counter :value 4)))
                   (setf (counter-value counter) 7)
                   (let ((before (list (counter-value counter)
                                       (counter-value child)
                                       (slot-boundp counter 'value)
                                       (slot-boundp child 'value))))
                     (slot-makunbound child 'value)
                     (list before
                           (slot-boundp counter 'value)
                           (slot-boundp child 'value)))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "((7 7 T T) NIL NIL)");
}

#[test]
fn evaluates_clos_invalid_slot_allocation_is_rejected() {
    assert!(Runtime::new()
        .eval_source(
            r#"(defclass invalid-allocation ()
                 ((value :allocation :bogus)))"#,
        )
        .is_err());
}

#[test]
fn evaluates_clos_multiple_method_qualifiers_are_rejected() {
    assert!(Runtime::new()
        .eval_source(
            r#"(progn
                 (defgeneric qualifier-multi (object))
                 (defmethod qualifier-multi :before :after ((object t)) :value))"#,
        )
        .is_err());
}

#[test]
fn evaluates_clos_make_instance_type_error_does_not_mutate_class_slots() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defclass atomic-class-slots ()
                   ((first :allocation :class :initarg :first
                           :initform 1 :type integer)
                    (second :allocation :class :initarg :second
                            :initform 2 :type integer)))
                 (let ((object (make-instance 'atomic-class-slots)))
                   (ignore-errors
                     (make-instance 'atomic-class-slots :first 9 :second "bad"))
                   (list (slot-value object 'first)
                         (slot-value object 'second)
                         (slot-value (make-instance 'atomic-class-slots) 'first)
                         (slot-value (make-instance 'atomic-class-slots) 'second))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(1 2 1 2)");
}

#[test]
fn evaluates_clos_default_initargs() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defclass defaults ()
                   ((value :initarg :value :initform 1))
                   (:default-initargs :value (+ 2 5)))
                 (defclass child-defaults (defaults) ())
                 (let ((explicit (make-instance 'child-defaults :value 9))
                       (implicit (make-instance 'child-defaults)))
                   (list (slot-value explicit 'value)
                         (slot-value implicit 'value))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(9 7)");
}

#[test]
fn evaluates_clos_setf_and_generic_methods() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defclass point ()
                   ((x :initarg :x :accessor point-x)
                    (y :initarg :y :accessor point-y)))
                 (defgeneric point-total (object))
                 (defmethod point-total ((object point))
                   (+ (point-x object) (point-y object)))
                 (let ((point (make-instance 'point :x 2 :y 3)))
                   (setf (point-x point) 8)
                   (list (point-x point)
                         (slot-value point 'x)
                         (point-total point))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(8 8 11)");
}

#[test]
fn evaluates_clos_methods_with_ordinary_lambda_lists() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defclass point () ())
                 (defgeneric describe-point (object &optional prefix &key suffix))
                 (defmethod describe-point
                   ((object point)
                    &optional (prefix "default" prefix-p)
                    &key (suffix "suffix" suffix-p))
                   (list prefix suffix prefix-p suffix-p))
                 (defgeneric collect-point (object))
                 (defmethod collect-point ((object point) &rest values)
                   values)
                 (let ((point (make-instance 'point)))
                   (list (describe-point point)
                         (describe-point point "given" :suffix "tail")
                         (collect-point point 1 2 3))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "((\"default\" \"suffix\" NIL NIL) (\"given\" \"tail\" T T) (1 2 3))"
    );
}

#[test]
fn evaluates_clos_inheritance_and_specialization() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defclass point ()
                   ((x :initarg :x :accessor point-x)))
                 (defclass colored-point (point)
                   ((color :initarg :color :accessor point-color)))
                 (defgeneric point-coordinate (object))
                 (defmethod point-coordinate ((object point))
                   (list (point-x object)))
                 (let ((point (make-instance 'colored-point :x 4 :color :red)))
                   (list (point-x point)
                         (point-color point)
                         (typep point 'point)
                         (typep point 'colored-point)
                         (point-coordinate point))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(4 :RED T T (4))");
}

#[test]
fn evaluates_clos_c3_class_precedence() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defclass c3-root () ())
                 (defclass c3-left (c3-root) ())
                 (defclass c3-right (c3-root) ())
                 (defclass c3-diamond (c3-left c3-right) ())
                 (defgeneric c3-walk (object))
                 (defmethod c3-walk ((object c3-root)) (list :root))
                 (defmethod c3-walk ((object c3-right))
                   (cons :right (call-next-method)))
                 (defmethod c3-walk ((object c3-left))
                   (cons :left (call-next-method)))
                 (let ((diamond (make-instance 'c3-diamond)))
                   (list
                     (c3-walk diamond)
                     (not (ignore-errors
                            (defclass c3-inconsistent-left (c3-left c3-right) ())
                            (defclass c3-inconsistent-right (c3-right c3-left) ())
                            (defclass c3-inconsistent
                              (c3-inconsistent-left c3-inconsistent-right) ()))))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "((:LEFT :RIGHT :ROOT) T)");
}

#[test]
fn evaluates_clos_class_precedence_list_returns_class_objects_in_order() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defclass cpl-root () ())
                 (defclass cpl-left (cpl-root) ())
                 (defclass cpl-right (cpl-root) ())
                 (defclass cpl-diamond (cpl-left cpl-right) ())
                 (let ((precedence
                         (class-precedence-list (find-class 'cpl-diamond))))
                   (list
                     (typep (car precedence) 'class)
                     (mapcar #'class-name precedence))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "(T (CPL-DIAMOND CPL-LEFT CPL-RIGHT CPL-ROOT STANDARD-OBJECT))"
    );
}

#[test]
fn evaluates_clos_builtin_type_specializers() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defgeneric numeric-kind (object))
                 (defmethod numeric-kind ((object number)) :number)
                 (defmethod numeric-kind ((object integer)) :integer)
                 (list (numeric-kind 3)
                       (numeric-kind 3.5)))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(:INTEGER :NUMBER)");
}

#[test]
fn evaluates_clos_eql_specializers() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defgeneric numeric-kind-eql (object))
                 (defmethod numeric-kind-eql ((object number)) :number)
                 (defmethod numeric-kind-eql ((object (eql 1))) :one)
                 (list (numeric-kind-eql 1)
                       (numeric-kind-eql 2)))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(:ONE :NUMBER)");
}

#[test]
fn evaluates_clos_class_documentation() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defclass documented-class () ()
                   (:documentation "class-doc"))
                 (list (documentation (find-class 'documented-class) t)
                       (documentation (find-class 'documented-class) 'class)))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(\"class-doc\" NIL)");
}

#[test]
fn evaluates_read_time_evaluation() {
    let values = Runtime::new()
        .eval_source(r#"(list #.(+ 1 2) #.(list 'list :ok 4))"#)
        .unwrap();

    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(3 (:OK 4))");
}

#[test]
fn evaluates_clos_unbound_slots() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defclass point ()
                   ((x :initarg :x)))
                 (let ((point (make-instance 'point)))
                   (list
                     (slot-exists-p point 'x)
                     (slot-boundp point 'x)
                     (not (ignore-errors (slot-value point 'x)))
                     (progn
                       (setf (slot-value point 'x) 9)
                       (list (slot-boundp point 'x) (slot-value point 'x)))
                     (progn
                       (slot-makunbound point 'x)
                       (slot-boundp point 'x)))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(T NIL T (T 9) NIL)");
}

#[test]
fn evaluates_clos_method_combination() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defclass point () ((x :initarg :x)))
                 (let ((events nil))
                   (defgeneric point-value (object))
                   (defmethod point-value ((object t))
                     (setf events (cons :base events))
                     (list :base (next-method-p)))
                   (defmethod point-value :before ((object point))
                     (setf events (cons :before events)))
                   (defmethod point-value :after ((object point))
                     (setf events (cons :after events)))
                   (defmethod point-value ((object point))
                     (setf events (cons :primary events))
                     (list :primary (next-method-p) (call-next-method)))
                   (defmethod point-value :around ((object point))
                     (setf events (cons :around-before events))
                     (let ((value (call-next-method)))
                       (setf events (cons :around-after events))
                       (list :around value)))
                   (let ((point (make-instance 'point :x 7)))
                     (list (point-value point) (reverse events)))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "((:AROUND (:PRIMARY T (:BASE NIL))) (:AROUND-BEFORE :BEFORE :PRIMARY :BASE :AFTER :AROUND-AFTER))"
    );
}

#[test]
fn evaluates_clos_multi_argument_precedence_and_method_redefinition() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defclass precedence-first () ())
                 (defclass precedence-second () ())
                 (defgeneric precedence-pick (first second))
                 (defmethod precedence-pick ((first t) (second precedence-second))
                   :second-specific)
                 (defmethod precedence-pick ((first precedence-first) (second t))
                   :first-specific)
                 (defgeneric redefine-method (object))
                 (defmethod redefine-method ((object t)) :old)
                 (defmethod redefine-method ((object t)) :new)
                 (list
                   (precedence-pick (make-instance 'precedence-first)
                                    (make-instance 'precedence-second))
                   (redefine-method nil)))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(:FIRST-SPECIFIC :NEW)");
}

#[test]
fn evaluates_the_with_type_designators() {
    assert_eq!(
        evaluate(
            "(list (the integer (+ 3 4))
                    (the rational 1/2)
                    (the float 0.5)
                    (ignore-errors (the integer 1/2)))",
        )
        .to_string(),
        "(7 1/2 0.5 NIL)"
    );
}

#[test]
fn evaluates_locally_and_eval_when() {
    assert_eq!(
        evaluate(
            "(let ((seen 0))
               (list
                 (locally
                   (declare (type integer seen))
                   (setq seen 4)
                   seen)
                 (eval-when (:execute) (+ seen 1))
                 (eval-when (:compile-toplevel) (setq seen 99))
                 (progn
                   (declaim (optimize speed))
                   (proclaim '(inline seen))
                   seen)))",
        )
        .to_string(),
        "(4 5 NIL 4)"
    );
}

#[test]
fn evaluates_defstruct_constructors_accessors_and_copies() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defstruct person name (age 21))
                 (let ((person (make-person :name "Ada")))
                   (setf (person-name person) "Grace")
                   (list (person-p person)
                         (person-name person)
                         (person-age person)
                         (typep person 'person)
                         (type-of person)
                         (eq person (copy-person person))
                         (equal person (copy-person person))
                         (write-to-string person))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        r##"(T "Grace" 21 T PERSON NIL T "#S(PERSON :NAME \"Grace\" :AGE 21)")"##,
    );
}

#[test]
fn evaluates_defstruct_documentation() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defstruct
                   (audit-doc (:constructor make-audit-doc))
                   "bounded defstruct doc"
                   value)
                 (let ((record (make-audit-doc :value 7)))
                   (list (documentation 'audit-doc 'structure)
                         (documentation 'audit-doc 'function)
                         (audit-doc-value record))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), r#"("bounded defstruct doc" NIL 7)"#);
}

#[test]
fn evaluates_function_documentation() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defun documented (value) "function doc" (+ value 1))
                 (let ((anonymous (lambda (value) "lambda doc" value)))
                   (list (documentation 'documented 'function)
                         (documentation #'documented 'function)
                         (documentation anonymous 'function)
                         (documentation 'documented 'variable)
                         (documented 7))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        r#"("function doc" "function doc" "lambda doc" NIL 8)"#
    );
}

#[test]
fn evaluates_defstruct_name_and_options() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defstruct
                   (account
                    (:conc-name acct-)
                    (:predicate account-p)
                    (:copier clone-account)
                    (:constructor make-account-record))
                   id (balance 0))
                 (defstruct
                   (plain
                    (:conc-name nil)
                    (:predicate plain-p)
                    (:copier clone-plain)
                    (:constructor make-plain))
                   amount)
                 (defstruct
                   (disabled
                    (:predicate nil)
                    (:copier nil)
                    (:constructor nil))
                   value)
                 (let ((account (make-account-record :id 7))
                       (plain (make-plain :amount 9)))
                   (list (account-p account)
                         (acct-id account)
                         (acct-balance account)
                         (equal account (clone-account account))
                         (typep account 'account)
                         (type-of account)
                         (amount plain)
                         (plain-p plain)
                         (equal plain (clone-plain plain)))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(T 7 0 T T ACCOUNT 9 T T)");
}

#[test]
fn evaluates_defstruct_named_option() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defstruct (named-record :named) value)
                 (let ((record (make-named-record :value 7)))
                   (list (named-record-p record)
                         (named-record-value record)
                         (typep record 'named-record))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(T 7 T)");
}

#[test]
fn evaluates_defstruct_typed_list_and_vector() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defstruct (typed-list (:type list)) first second)
                 (defstruct (typed-named-list (:type list) :named) first second)
                 (defstruct (typed-vector (:type vector)) first second)
                 (defstruct (typed-named-vector (:type vector) :named) first second)
                 (let ((list-record (make-typed-list :first 1 :second 2))
                       (named-list-record (make-typed-named-list :first 3 :second 4))
                       (vector-record (make-typed-vector :first 5 :second 6))
                       (named-vector-record (make-typed-named-vector :first 7 :second 8)))
                   (setf (elt list-record 0) 11)
                   (setf (nth 1 list-record) 22)
                   (setf (aref vector-record 1) 66)
                   (setf (svref vector-record 0) 55)
                   (setf (row-major-aref vector-record 0) 44)
                   (list (listp list-record)
                         (vectorp list-record)
                         (typed-list-first list-record)
                         (typed-list-second list-record)
                         (car list-record)
                         (cdr list-record)
                         (length list-record)
                         (elt list-record 1)
                         (endp list-record)
                         (listp named-list-record)
                         (car named-list-record)
                         (typed-named-list-first named-list-record)
                         (typed-named-list-p named-list-record)
                         (vectorp vector-record)
                         (simple-vector-p vector-record)
                         (typed-vector-first vector-record)
                         (typed-vector-second vector-record)
                         (svref vector-record 0)
                         (aref vector-record 1)
                         (row-major-aref vector-record 0)
                         (vectorp named-vector-record)
                         (simple-vector-p named-vector-record)
                         (typed-named-vector-p named-vector-record)
                         (typep list-record 'list)
                         (typep vector-record 'vector)
                         (typep list-record 'sequence)
                         (typep vector-record 'sequence)
                         (type-of list-record)
                         (type-of vector-record)
                         named-list-record
                         named-vector-record)))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "(T NIL 11 22 11 (22) 2 22 NIL T TYPED-NAMED-LIST 3 T T T 44 66 44 66 44 T T T T T T T LIST VECTOR (TYPED-NAMED-LIST 3 4) #(TYPED-NAMED-VECTOR 7 8))"
    );
}

#[test]
fn evaluates_defstruct_typed_list_endp_boundaries() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defstruct (typed-empty-list (:type list)))
                 (defstruct (typed-empty-named-list (:type list) :named))
                 (let ((empty (make-typed-empty-list))
                       (named (make-typed-empty-named-list)))
                   (list (endp empty)
                         (endp named)
                         (listp empty)
                         (listp named)
                         (typed-empty-named-list-p named)
                         empty
                         named)))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "(T NIL T T T NIL (TYPED-EMPTY-NAMED-LIST))"
    );
}

#[test]
fn evaluates_typed_sequence_copy_and_coerce() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defstruct (typed-list (:type list) :named) first second)
                 (defstruct (typed-vector (:type vector) :named) first second)
                 (let ((list-record (make-typed-list :first 1 :second (list 2 3)))
                       (vector-record (make-typed-vector :first 4 :second 5)))
                   (list (copy-tree list-record)
                         (eq (coerce list-record 'sequence) list-record)
                         (coerce list-record 'list)
                         (eq (coerce vector-record 'sequence) vector-record)
                         (coerce vector-record 'vector))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "((TYPED-LIST 1 (2 3)) T (TYPED-LIST 1 (2 3)) T #(TYPED-VECTOR 4 5))"
    );
}

#[test]
fn evaluates_typed_sequence_setf_subseq() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defstruct (typed-list (:type list) :named) first second)
                 (defstruct (typed-vector (:type vector) :named) first second)
                 (let ((list-record (make-typed-list :first 1 :second 2))
                       (vector-record (make-typed-vector :first 3 :second 4)))
                   (setf (subseq list-record 1 3) '(11 22))
                   (setf (subseq vector-record 1 3) #(33 44))
                   (list list-record
                         vector-record
                         (typed-list-p list-record)
                         (typed-vector-p vector-record)
                         (typed-list-first list-record)
                         (typed-list-second list-record)
                         (typed-vector-first vector-record)
                         (typed-vector-second vector-record))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "((TYPED-LIST 11 22) #(TYPED-VECTOR 33 44) T T 11 22 33 44)"
    );
}

#[test]
fn evaluates_typed_list_setf_car_and_cdr() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defstruct (named-list (:type list) :named) first second)
                 (defstruct (plain-list (:type list)) first second)
                 (let ((named (make-named-list :first 1 :second 2))
                       (plain (make-plain-list :first 3 :second 4)))
                   (setf (cdr named) '(11 22))
                   (setf (car plain) 33)
                   (setf (cdr plain) '(44 55))
                   (list named
                         (named-list-p named)
                         (named-list-first named)
                         (named-list-second named)
                         plain
                         (plain-list-first plain)
                         (plain-list-second plain))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "((NAMED-LIST 11 22) T 11 22 (33 44 55) 33 44)"
    );
}

#[test]
fn evaluates_typed_named_discriminator_mutation() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defstruct (named-list (:type list) :named) first second)
                 (defstruct (named-vector (:type vector) :named) first second)
                 (let* ((list-record (make-named-list :first 1 :second 2))
                        (vector-record (make-named-vector :first 3 :second 4))
                        (list-copy (copy-named-list list-record))
                        (vector-copy (copy-named-vector vector-record)))
                   (setf (car list-record) 'broken-list)
                   (setf (svref vector-record 0) 'broken-vector)
                   (list list-record
                         (named-list-p list-record)
                         (typep list-record 'named-list)
                         (type-of list-record)
                         (named-list-first list-record)
                         list-copy
                         (named-list-p list-copy)
                         vector-record
                         (named-vector-p vector-record)
                         (typep vector-record 'named-vector)
                         (type-of vector-record)
                         (named-vector-first vector-record)
                         vector-copy
                         (named-vector-p vector-copy))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "((BROKEN-LIST 1 2) NIL NIL CONS 1 (NAMED-LIST 1 2) T #(BROKEN-VECTOR 3 4) NIL NIL SIMPLE-VECTOR 3 #(NAMED-VECTOR 3 4) T)"
    );
}

#[test]
fn evaluates_defstruct_read_only_slots() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defstruct record
                   (id 0 t)
                   (label "initial" nil))
                 (let ((record (make-record :id 7 :label "before")))
                   (setf (record-label record) "after")
                   (list (record-id record)
                         (record-label record))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), r#"(7 "after")"#);

    let error = Runtime::new()
        .eval_source(
            r#"(progn
                 (defstruct immutable (id 0 t))
                 (let ((record (make-immutable :id 1)))
                   (setf (immutable-id record) 2)))"#,
        )
        .unwrap_err();
    assert!(matches!(
        error,
        ncl_runtime::RuntimeError::InvalidForm { message, .. }
            if message == "cannot SETF a read-only structure slot"
    ));
}

#[test]
fn evaluates_defstruct_included_slots_and_type_hierarchy() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defstruct
                   (person (:constructor nil))
                   name (age 0))
                 (defstruct
                   (employee (:include person (age 21)))
                   id)
                 (let ((employee (make-employee :name "Ada" :id 7)))
                   (setf (person-name employee) "Grace")
                   (setf (employee-age employee) 42)
                   (list (employee-name employee)
                         (person-name employee)
                         (employee-age employee)
                         (person-age employee)
                         (employee-id employee)
                         (person-p employee)
                         (employee-p employee)
                         (typep employee 'person)
                         (typep employee 'employee)
                         (type-of employee)
                         (equal employee (copy-person employee))
                         (write-to-string employee))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        r##"("Grace" "Grace" 42 42 7 T T T T EMPLOYEE T "#S(EMPLOYEE :NAME \"Grace\" :AGE 42 :ID 7)")"##,
    );
}

#[test]
fn evaluates_defstruct_boa_constructors() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defstruct
                   (boa
                    (:constructor make-boa
                      (first
                       &optional
                       (second)
                       (third (+ first second))
                       &rest rest
                       &key
                       ((:flag flag) t)
                       &aux
                       (sum (+ first second)))))
                   first (second 20) (third 30) rest flag sum)
                 (let ((default (make-boa 1))
                       (explicit (make-boa 1 2 3 :flag nil)))
                   (list
                     (list (boa-first default)
                           (boa-second default)
                           (boa-third default)
                           (boa-rest default)
                           (boa-flag default)
                           (boa-sum default))
                     (list (boa-first explicit)
                           (boa-second explicit)
                           (boa-third explicit)
                           (boa-rest explicit)
                           (boa-flag explicit)
                           (boa-sum explicit)))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "((1 20 21 NIL T 21) (1 2 3 (:FLAG NIL) NIL 3))",
    );
}

#[test]
fn evaluates_arrays_and_multidimensional_setf() {
    assert_eq!(
        evaluate(
            "(let ((array (make-array '(2 2) :element-type t :initial-element 0))
                   (vector (make-array 3 :element-type 't :initial-element 5)))
               (setf (aref array 1 0) 7
                     (aref vector 2) 9)
               (list (arrayp array) (array-rank array) (array-dimensions array)
                     (array-dimension array 1) (array-total-size array)
                     (aref array 1 0) (row-major-aref array 2)
                     (aref vector 2) (typep array 'array)))",
        )
        .to_string(),
        "(T 2 (2 2) 2 4 7 7 9 T)"
    );
    assert_eq!(
        evaluate(
            "(let ((array (make-array '(2 2)
                                      :initial-contents '((1 2) (3 4)))))
               (list (aref array 0 1) (aref array 1 0)
                     (row-major-aref array 3)))",
        )
        .to_string(),
        "(2 3 4)"
    );
    assert_eq!(
        evaluate(
            "(let ((array (make-array '(2 3)
                                      :initial-contents '((0 1 2) (3 4 5)))))
               (list (array-row-major-index array 1 2)
                     (array-in-bounds-p array 1 2)
                     (array-in-bounds-p array 2 0)
                     (aref array 1 2)
                     (row-major-aref array (array-row-major-index array 1 2))
                     (array-element-type array)
                     (simple-array-p array)
                     (simple-vector-p (vector 1 2))
                     (simple-vector-p array)))",
        )
        .to_string(),
        "(5 T NIL 5 5 T T T NIL)"
    );
    assert_eq!(
        evaluate(
            "(let ((array (make-array '(2 3))))
               (list (array-in-bounds-p array -1 0)
                     (array-in-bounds-p array 0 -1)))",
        )
        .to_string(),
        "(NIL NIL)"
    );
    assert_eq!(
        evaluate(
            "(list (simple-array-p (make-array 2))
                   (simple-array-p (make-array 2 :fill-pointer 1))
                   (simple-array-p (make-array 2 :adjustable t)))",
        )
        .to_string(),
        "(T NIL NIL)"
    );
    assert!(matches!(
        Runtime::new().eval_source("(make-array 1 :element-type 'integer)"),
        Err(RuntimeError::InvalidForm { message, .. })
            if message.contains("only supports :element-type T, CHARACTER, or BIT")
    ));
    assert!(matches!(
        Runtime::new().eval_source("(svref (make-array 1 :element-type 'bit) 0)"),
        Err(RuntimeError::Type { expected, .. }) if expected == "svref requires simple-vector"
    ));
    assert_eq!(
        evaluate(
            "(let ((text \"abc\"))
               (setf (aref text 1) #\\X
                     (row-major-aref text 2) #\\Z)
               (list text (arrayp text) (array-rank text)
                     (array-dimensions text) (array-total-size text)
                     (array-element-type text) (aref text 1)
                     (row-major-aref text 2) (array-in-bounds-p text 2)))",
        )
        .to_string(),
        "(\"aXZ\" T 1 (3) 3 CHARACTER #\\X #\\Z T)"
    );
}

#[test]
fn evaluates_adjust_array_preserves_contents() {
    assert_eq!(
        evaluate(
            r#"(let ((array (make-array '(2 2)
                                      :initial-contents '((1 2) (3 4)))))
               (let ((adjusted (adjust-array array '(3 2) :initial-element 9)))
                 (list (array-dimensions adjusted)
                       (row-major-aref adjusted 0)
                       (row-major-aref adjusted 1)
                       (row-major-aref adjusted 2)
                       (row-major-aref adjusted 3)
                       (row-major-aref adjusted 4)
                       (row-major-aref adjusted 5)
                       (array-dimensions array))))"#,
        )
        .to_string(),
        "((3 2) 1 2 3 4 9 9 (2 2))"
    );
    assert_eq!(
        evaluate(
            r#"(let ((array (vector 1 2)))
               (let ((adjusted (adjust-array array 3 :initial-contents '(7 8 9))))
                 (list (simple-vector-p adjusted)
                       (array-dimensions adjusted)
                       (aref adjusted 0)
                       (aref adjusted 1)
                       (aref adjusted 2)
                       (aref array 0))))"#,
        )
        .to_string(),
        "(T (3) 7 8 9 1)"
    );
    assert_eq!(
        evaluate(
            r#"(adjust-array
                 (make-array 2 :element-type 'character :initial-contents "ab")
                 3 :initial-element #\c)"#,
        )
        .to_string(),
        "\"abc\""
    );
}

#[test]
fn evaluates_typed_arrays_and_character_updates() {
    assert_eq!(
        evaluate(
            r#"(let ((chars (make-array 4 :element-type 'character
                                      :initial-contents "abcd"))
                   (matrix (make-array '(1 2) :element-type 'character
                                       :initial-contents '((#\a #\b))))
                   (general (make-array 3 :element-type t
                                        :initial-contents "abc")))
               (setf (aref chars 1) #\X
                     (row-major-aref matrix 1) #\Y)
               (list (array-element-type chars) chars
                     (array-element-type matrix) (aref matrix 0 1)
                     (array-element-type general) (aref general 1)
                     (typep chars '(array character (4)))
                     (typep chars '(array t (4)))
                     (typep matrix '(array character (1 2)))
                     (typep matrix '(array t (1 2)))
                     (typep general '(array t (3)))
                     (typep general '(array character (3)))
                     (typep "abc" '(vector character 3))))"#,
        )
        .to_string(),
        "(CHARACTER \"aXcd\" CHARACTER #\\Y T #\\b T NIL T NIL T NIL T)"
    );
    assert!(matches!(
        Runtime::new().eval_source(
            "(let ((array (make-array 1 :element-type 'character)))\
             (setf (aref array 0) 1))",
        ),
        Err(RuntimeError::Type { expected, .. })
            if expected == "CHARACTER"
    ));
    assert!(matches!(
        Runtime::new().eval_source(
            "(let ((array (make-array 1 :element-type 'character)))\
             (setf (svref array 0) #\\a))",
        ),
        Err(RuntimeError::Type { expected, .. })
            if expected == "SIMPLE-VECTOR"
    ));
    assert!(matches!(
        Runtime::new().eval_source(
            "(make-array 2 :element-type 'character :initial-contents '(#\\a 1))",
        ),
        Err(RuntimeError::Type { expected, .. })
            if expected == "make-array requires CHARACTER"
    ));
    assert!(matches!(
        Runtime::new().eval_source(
            "(let ((array (make-array 1 :element-type 'character)))\
             (setf (row-major-aref array 0) 1))",
        ),
        Err(RuntimeError::Type { expected, .. }) if expected == "CHARACTER"
    ));
}

#[test]
fn evaluates_vector_type_metadata() {
    assert_eq!(
        evaluate(
            r#"(let ((general (make-array 3 :element-type t
                                           :initial-contents "abc"))
                   (chars (make-array 3 :element-type 'character
                                       :initial-contents "abc")))
               (list (vectorp "abc")
                     (simple-vector-p "abc")
                     (typep "abc" 'vector)
                     (typep "abc" 'simple-vector)
                     (typep general '(vector character 3))
                     (typep general '(vector t 3))
                     (typep chars '(vector character 3))
                     (typep chars '(vector t 3))
                     (simple-vector-p general)
                     (simple-vector-p chars)
                     (typep general 'bit-vector)
                     (typep chars 'bit-vector)
                     (typep #(0 1) 'bit-vector)
                     (typep #(0 1) 'simple-bit-vector)))"#,
        )
        .to_string(),
        "(T NIL T NIL NIL T T NIL T NIL NIL NIL NIL NIL)"
    );
}

#[test]
fn evaluates_fill_pointer_and_vector_push_operations() {
    assert_eq!(
        evaluate(
            r#"(let ((vector (make-array 3 :element-type t
                                           :initial-element :empty
                                           :fill-pointer 1))
                   (adjustable (make-array 1 :element-type 'character
                                             :fill-pointer 0
                                             :adjustable t)))
               (setf (fill-pointer vector) 2)
               (list (fill-pointer vector)
                     (array-has-fill-pointer-p vector)
                     (adjustable-array-p vector)
                     (length vector)
                     (vector-push :a vector)
                     (vector-push :b vector)
                     (length vector)
                     (vector-pop vector)
                     (fill-pointer vector)
                     (vector-push-extend #\z adjustable)
                     (vector-push-extend #\y adjustable)
                     (vector-push-extend #\x adjustable 2)
                     (array-dimensions adjustable)
                     (length adjustable)
                     (aref adjustable 0)
                     (aref adjustable 1)
                     (aref adjustable 2)))"#,
        )
        .to_string(),
        "(2 T NIL 2 2 NIL 3 :A 2 0 1 2 (4) 3 #\\z #\\y #\\x)"
    );
    assert!(matches!(
        Runtime::new().eval_source("(fill-pointer #(1 2))"),
        Err(RuntimeError::Type { expected, .. })
            if expected == "fill-pointer requires an array with a fill pointer"
    ));
    assert!(matches!(
        Runtime::new().eval_source(
            "(vector-push-extend 1 (make-array 1 :fill-pointer 0))",
        ),
        Err(RuntimeError::Type { expected, .. })
            if expected == "vector-push-extend requires an adjustable vector with a fill pointer"
    ));
}

#[test]
fn evaluates_bit_vectors() {
    assert_eq!(
        evaluate(
            r#"(let ((bits (make-array 4 :element-type 'bit
                                         :initial-contents '(0 1 1 0))))
               (setf (aref bits 1) 0)
               (list (array-element-type bits)
                     (vectorp bits)
                     (typep bits '(vector bit 4))
                     (typep bits 'bit-vector)
                     (typep bits 'simple-bit-vector)
                     (aref bits 1)))"#,
        )
        .to_string(),
        "(BIT T T T T 0)"
    );
    assert!(matches!(
        Runtime::new().eval_source(
            "(make-array 2 :element-type 'bit :initial-contents '(0 2))",
        ),
        Err(RuntimeError::Type { expected, .. })
            if expected == "make-array requires BIT"
    ));
    assert_eq!(
        evaluate(
            "(list #*101 (sbit #*101 1) (bit-vector-p #*101)
                    (let ((bits #*0110))
                      (setf (sbit bits 1) 0
                            (bit bits 2) 0)
                      (list bits (sbit bits 0) (bit bits 3))))",
        )
        .to_string(),
        "(#*101 0 T (#*0000 0 0))"
    );
}

#[test]
fn evaluates_common_lisp_bit_not() {
    assert_eq!(
        evaluate(
            "(let ((source #*101)
                   (target #*000))
               (list (bit-not source)
                     (bit-not source nil)
                     (bit-not source target)
                     target
                     source))",
        )
        .to_string(),
        "(#*010 #*010 #*010 #*010 #*101)"
    );
    assert_eq!(
        evaluate(
            "(let ((bits #*101))
               (list (bit-not bits bits) bits))",
        )
        .to_string(),
        "(#*010 #*010)"
    );
    assert_eq!(
        evaluate(
            "(let ((bits #*101))
               (list (eq (bit-not bits t) bits) bits))",
        )
        .to_string(),
        "(T #*010)"
    );
    let error = Runtime::new().eval_source("(bit-not 1)").unwrap_err();
    assert!(matches!(
        error,
        RuntimeError::Type { expected, .. }
            if expected == "bit-not requires bit-array"
    ));
    let mismatch = Runtime::new()
        .eval_source("(bit-not #*10 #*0)")
        .unwrap_err();
    assert!(matches!(
        mismatch,
        RuntimeError::InvalidForm { message, .. }
            if message == "bit-not requires arrays with matching dimensions"
    ));
}

#[test]
fn evaluates_hash_tables_and_gethash_setf() {
    assert_eq!(
        evaluate(
            "(let ((eq-table (make-hash-table :test #'eq))
                   (eql-table (make-hash-table))
                   (equal-table (make-hash-table :test #'equal))
                   (equalp-table (make-hash-table :test #'equalp)))
               (setf (gethash 'key eq-table) 1
                     (gethash 42 eql-table) 2
                     (gethash '(a b) equal-table) 3
                     (gethash \"Key\" equalp-table) 4)
               (list (hash-table-p eq-table) (typep eq-table 'hash-table)
                     (hash-table-count eq-table) (hash-table-test eq-table)
                     (gethash 'key eq-table) (gethash 42 eql-table)
                     (gethash '(a b) equal-table) (gethash \"key\" equalp-table)))",
        )
        .to_string(),
        "(T T 1 EQ 1 2 3 4)"
    );
    assert_eq!(
        evaluate(
            "(let ((table (make-hash-table :test #'equal :size 4)))
               (setf (gethash \"key\" table) 42)
               (multiple-value-bind (value present) (gethash \"key\" table)
                 (list value present (gethash \"missing\" table 99)
                       (remhash \"key\" table) (hash-table-count table)
                       (progn (setf (gethash 'other table) 7)
                              (clrhash table)
                              (hash-table-count table)))))",
        )
        .to_string(),
        "(42 T 99 T 0 0)"
    );
}

#[test]
fn evaluates_hash_table_options() {
    assert_eq!(
        evaluate(
            "(let ((table (make-hash-table :test #'equal :size 4
                                            :rehash-size 1.5
                                            :rehash-threshold 0.5
                                            :synchronized t)))
               (list (hash-table-size table)
                     (= (hash-table-rehash-size table) 1.5)
                     (= (hash-table-rehash-threshold table) 0.5)
                     (hash-table-synchronized-p table)
                     (hash-table-test table)))",
        )
        .to_string(),
        "(4 T T T EQUAL)"
    );
}

#[test]
fn evaluates_with_hash_table_iterator() {
    assert_eq!(
        evaluate(
            "(let ((table (make-hash-table)))
               (setf (gethash 'a table) 1)
               (setf (gethash 'b table) 2)
               (with-hash-table-iterator (next table)
                 (list (multiple-value-list (next))
                       (multiple-value-list (next))
                       (multiple-value-list (next)))))",
        )
        .to_string(),
        "((T A 1) (T B 2) (NIL))"
    );
}

#[test]
fn evaluates_maphash() {
    assert_eq!(
        evaluate(
            "(let ((table (make-hash-table)))
               (setf (gethash 'a table) 1)
               (list (eq table
                         (maphash (lambda (key value)
                                    (setf (gethash 'b table) value))
                                  table))
                     (hash-table-count table) (gethash 'b table)))",
        )
        .to_string(),
        "(T 2 1)"
    );
}

#[test]
fn evaluates_setf_gethash_optional_default_contract() {
    assert_eq!(
        evaluate(
            "(let ((table (make-hash-table))
                   (key-reads 0)
                   (table-reads 0)
                   (default-reads 0)
                   (value-reads 0))
               (list
                 (setf
                   (gethash (progn (incf key-reads) 'key)
                            (progn (incf table-reads) table)
                            (progn (incf default-reads) 99))
                   (progn (incf value-reads) 42))
                 (gethash 'key table)
                 key-reads table-reads default-reads value-reads))",
        )
        .to_string(),
        "(42 42 1 1 1 1)"
    );
}

#[test]
fn evaluates_handler_case_and_handler_bind() {
    assert_eq!(
        evaluate(
            "(handler-case (+ 1 \"x\")
               (type-error (condition) (list (type-of condition) 'caught)))",
        )
        .to_string(),
        "(CONDITION CAUGHT)"
    );
    assert_eq!(
        evaluate(
            "(multiple-value-bind (first second)
                (handler-case (values 1 2) (error (condition) 9))
              (list first second))",
        )
        .to_string(),
        "(1 2)"
    );
    assert_eq!(
        evaluate(
            "(handler-case (values 10 20)
               (:no-error (first second) (list second first)))",
        )
        .to_string(),
        "(20 10)"
    );
    assert_eq!(
        evaluate(
            "(handler-bind ((type-error (lambda (condition)
                                         (list (type-of condition) 'handled))))
               (+ 1 \"x\"))",
        )
        .to_string(),
        "(CONDITION HANDLED)"
    );
    assert_eq!(
        evaluate(
            "(handler-case (block done (return-from done 7))
               (error (condition) 9))",
        )
        .to_string(),
        "7"
    );
}

#[test]
fn evaluates_error_through_condition_handlers() {
    assert_eq!(
        evaluate(
            "(handler-case (error \"boom\")
               (simple-error (condition) (list (type-of condition) 'caught)))",
        )
        .to_string(),
        "(CONDITION CAUGHT)"
    );
    assert_eq!(
        evaluate(
            "(multiple-value-bind (value condition)
                (ignore-errors (error \"boom\"))
              (list value (type-of condition)))",
        )
        .to_string(),
        "(NIL CONDITION)"
    );
    assert_eq!(
        evaluate(
            "(handler-bind ((simple-error
                               (lambda (condition)
                                 (declare (ignore condition))
                                 (invoke-restart 'continue))))
               (restart-case (error \"boom\")
                 (continue () 42)))",
        )
        .to_string(),
        "42"
    );
}

#[test]
fn evaluates_signal_warn_cerror_and_dynamic_handlers() {
    assert_eq!(
        evaluate(
            "(handler-case (signal \"boom\")
               (simple-condition (condition) (list (type-of condition) 'signal-caught)))",
        )
        .to_string(),
        "(CONDITION SIGNAL-CAUGHT)"
    );
    assert_eq!(
        evaluate(
            "(handler-case (warn \"careful\")
               (warning (condition) (list (type-of condition) 'warning-caught)))",
        )
        .to_string(),
        "(CONDITION WARNING-CAUGHT)"
    );
    assert_eq!(
        evaluate(
            "(handler-bind ((simple-condition
                               (lambda (condition)
                                 (declare (ignore condition))
                                 (invoke-restart 'continue))))
               (restart-case (signal \"continue\")
                 (continue () 37)))",
        )
        .to_string(),
        "37"
    );
    assert_eq!(
        evaluate(
            "(restart-case (cerror \"continue\" \"boom\")
               (continue () 42))",
        )
        .to_string(),
        "42"
    );
    assert_eq!(
        evaluate(
            "(handler-bind ((simple-error
                               (lambda (condition)
                                 (declare (ignore condition))
                                 (invoke-restart 'continue))))
               (cerror \"continue\" \"boom\"))",
        )
        .to_string(),
        "NIL"
    );
}

#[test]
fn evaluates_condition_format_arguments() {
    assert_eq!(
        evaluate(
            r#"(handler-case
                   (error "failed: ~A (~D)" 'name 7)
                   (simple-error (condition)
                     (list
                       (simple-condition-format-control condition)
                       (simple-condition-format-arguments condition)
                       (typep condition 'simple-condition)
                       (typep condition 'simple-error))))"#,
        )
        .to_string(),
        "(\"failed: ~A (~D)\" (NAME 7) T T)"
    );
    assert_eq!(
        evaluate(
            r#"(handler-case
                   (signal "warning: ~A" 'careful)
                   (simple-condition (condition)
                     (list
                       (simple-condition-format-control condition)
                       (simple-condition-format-arguments condition))))"#,
        )
        .to_string(),
        "(\"warning: ~A\" (CAREFUL))"
    );
    assert_eq!(
        evaluate(
            r#"(let ((seen nil))
                   (handler-bind ((simple-error
                                   (lambda (condition)
                                     (setq seen
                                       (list
                                         (simple-condition-format-control condition)
                                         (simple-condition-format-arguments condition))))))
                     (restart-case
                       (cerror "continue ~A" "failed ~A" 'again)
                       (continue () (list 42 seen)))))"#,
        )
        .to_string(),
        "(42 (\"failed ~A\" (AGAIN)))"
    );
    assert_eq!(
        evaluate(
            r#"(handler-case
                   (error (make-condition 'simple-error
                            :format-control "constructed: ~A"
                            :format-arguments (list 'condition)))
                   (simple-error (condition)
                     (list
                       (simple-condition-format-control condition)
                       (simple-condition-format-arguments condition)
                       (typep condition 'condition)
                       (typep condition 'simple-error))))"#,
        )
        .to_string(),
        "(\"constructed: ~A\" (CONDITION) T T)"
    );
    assert_eq!(
        evaluate(
            r#"(let ((condition (make-condition 'user-condition)))
                   (typep condition 'condition))"#,
        )
        .to_string(),
        "T"
    );
}

#[test]
fn evaluates_type_error_standard_slots() {
    assert_eq!(
        evaluate(
            r#"(let ((condition
                       (make-condition 'type-error
                         :datum 42
                         :expected-type 'integer)))
                   (list
                     (typep condition 'condition)
                     (typep condition 'error)
                     (typep condition 'type-error)
                     (type-error-datum condition)
                     (type-error-expected-type condition)))"#,
        )
        .to_string(),
        "(T T T 42 INTEGER)"
    );
}

#[test]
fn evaluates_type_error_standard_slots_accept_escaped_initargs() {
    assert_eq!(
        evaluate(
            r#"(let ((condition
                       (make-condition 'type-error
                         :|DATUM| 42
                         :|EXPECTED-TYPE| 'integer)))
                   (list
                     (type-error-datum condition)
                     (type-error-expected-type condition)))"#,
        )
        .to_string(),
        "(42 INTEGER)"
    );
}

#[test]
fn evaluates_simple_type_error_standard_slots_and_hierarchy() {
    assert_eq!(
        evaluate(
            r#"(let ((condition
                       (make-condition 'simple-type-error
                         :datum 42
                         :expected-type 'integer)))
                  (list
                    (typep condition 'condition)
                    (typep condition 'error)
                    (typep condition 'type-error)
                    (typep condition 'simple-condition)
                    (typep condition 'simple-error)
                    (type-error-datum condition)
                    (type-error-expected-type condition)))"#,
        )
        .to_string(),
        "(T T T T NIL 42 INTEGER)"
    );
    assert_eq!(
        evaluate(
            r#"(let ((condition (make-condition 'control-error)))
                  (list
                    (typep condition 'condition)
                    (typep condition 'error)
                    (typep condition 'serious-condition)))"#,
        )
        .to_string(),
        "(T T T)"
    );
}

#[test]
fn evaluates_user_condition_inheritance_from_standard_conditions() {
    assert_eq!(
        evaluate(
            r#"(progn
                   (define-condition child-type-error (type-error) ())
                   (define-condition child-simple-error (simple-error) ())
                   (define-condition child-simple-warning (simple-warning) ())
                   (list
                     (let ((condition (make-condition 'child-type-error)))
                       (list (typep condition 'type-error)
                             (typep condition 'error)
                             (typep condition 'condition)))
                     (let ((condition (make-condition 'child-simple-error)))
                       (list (typep condition 'simple-error)
                             (typep condition 'simple-condition)
                             (typep condition 'error)))
                     (let ((condition (make-condition 'child-simple-warning)))
                       (list (typep condition 'simple-warning)
                             (typep condition 'simple-condition)
                             (typep condition 'warning)))))"#,
        )
        .to_string(),
        "((T T T) (T T T) (T T T))"
    );
}

#[test]
fn evaluates_define_condition_and_slots() {
    assert_eq!(
        evaluate(
            r#"(progn
                   (define-condition app-error (error)
                     ((code :initarg :code :initform 1
                            :reader app-error-code
                            :writer set-app-error-code)))
                   (let ((condition (make-condition 'app-error :code 7)))
                     (list
                       (typep condition 'app-error)
                       (typep condition 'error)
                       (app-error-code condition)
                       (set-app-error-code 9 condition)
                       (app-error-code condition)
                       (app-error-code (make-condition 'app-error)))))"#,
        )
        .to_string(),
        "(T T 7 9 9 1)"
    );
}

#[test]
fn preserves_escaped_condition_initarg_identity() {
    assert_eq!(
        evaluate(
            r#"(progn
                   (define-condition app-error (error)
                     ((code :initarg :|code| :initform 1 :reader app-error-code)))
                   (app-error-code (make-condition 'app-error :|code| 7)))"#,
        )
        .to_string(),
        "7"
    );
    let error = Runtime::new()
        .eval_source(
            r#"(progn
                   (define-condition app-error (error)
                     ((code :initarg :|code| :initform 1)))
                   (make-condition 'app-error :code 7))"#,
        )
        .unwrap_err();
    assert!(matches!(
        error,
        RuntimeError::InvalidForm { message, .. }
            if message.contains("unknown make-condition initarg")
    ));
}

#[test]
fn evaluates_define_condition_inheritance_and_slots() {
    assert_eq!(
        evaluate(
            r#"(progn
                   (define-condition base-error (error)
                     ((code :initarg :code :initform 1 :reader base-error-code)
                      (tag :initarg :tag :initform "base" :reader base-error-tag)))
                   (define-condition child-error (base-error)
                     ((detail :initarg :detail :initform 2 :reader child-error-detail)))
                   (let ((condition (make-condition 'child-error :code 7)))
                     (list
                       (typep condition 'child-error)
                       (typep condition 'base-error)
                       (typep condition 'error)
                       (base-error-code condition)
                       (base-error-tag condition)
                       (child-error-detail condition))))"#,
        )
        .to_string(),
        "(T T T 7 \"base\" 2)"
    );
}

#[test]
fn evaluates_c3_condition_precedence_for_inherited_slots() {
    assert_eq!(
        evaluate(
            r#"(progn
                   (define-condition c3-root-condition (condition) ())
                   (define-condition c3-left-condition (c3-root-condition)
                     ((shared :initform :left :reader c3-left-shared)))
                   (define-condition c3-right-condition (c3-root-condition)
                     ((shared :initform :right :reader c3-right-shared)))
                   (define-condition c3-diamond-condition
                       (c3-left-condition c3-right-condition) ())
                   (let ((condition (make-condition 'c3-diamond-condition)))
                     (list
                       (typep condition 'c3-left-condition)
                       (typep condition 'c3-right-condition)
                       (c3-left-shared condition)
                       (c3-right-shared condition))))"#,
        )
        .to_string(),
        "(T T :LEFT :LEFT)"
    );
}

#[test]
fn evaluates_catch_and_throw() {
    assert_eq!(
        evaluate(
            "(let ((seen nil))
               (list
                 (catch 'tag (throw 'tag 42))
                 (catch 7 (throw 7 9))
                 (catch 'outer (catch 'inner (throw 'outer 8)))
                 (catch 'tag
                   (unwind-protect (throw 'tag 5) (setq seen t)))
                 seen))",
        )
        .to_string(),
        "(42 9 8 5 T)"
    );
}

#[test]
fn evaluates_character_and_string_operations() {
    assert_eq!(
        evaluate(
            "(list (string #\\a) (string 'hello) (make-string 3 #\\x) (char \"abc\" 1) (char-code #\\A) (code-char 98) (char= #\\a #\\a) (char-equal #\\A #\\a) (char< #\\a #\\c) (string= \"abc\" \"abc\") (string-equal \"AbC\" \"aBc\") (string< \"abc\" \"abd\") (string-upcase \"Abc\") (string-downcase \"AbC\"))"
        )
        .to_string(),
        "(\"a\" \"HELLO\" \"xxx\" #\\b 65 #\\b T T T T T 2 \"ABC\" \"abc\")"
    );
    assert_eq!(
        evaluate(
            "(list (string= \"zabc\" \"xabc\" :start1 1 :end1 4 :start2 1 :end2 4)
                   (string< \"zabc\" \"xabd\" :start1 1 :end1 4 :start2 1 :end2 4)
                   (string<= \"za\" \"yb\" :start1 1 :start2 1))"
        )
        .to_string(),
        "(T 3 1)"
    );
    assert_eq!(
        evaluate(
            "(list (string-trim \" x\" \"xx Hello x\")
                   (string-left-trim \" x\" \"xx Hello x\")
                   (string-right-trim \" x\" \"xx Hello x\")
                   (string-capitalize \"hello, WORLD-42 foo_bar\")
                   (string-upcase \"abcdef\" :start 1 :end 4)
                   (string-downcase \"ABCDEF\" :start 1 :end 4)
                   (nstring-capitalize \"hELLO wORLD\"))"
        )
        .to_string(),
        "(\"Hello\" \"Hello x\" \"xx Hello\" \"Hello, World-42 Foo_Bar\" \"aBCDef\" \"AbcdEF\" \"Hello World\")"
    );
}

#[test]
fn evaluates_extended_character_operations() {
    assert_eq!(
        evaluate(
            r#"(list
                   (character "A")
                   (character 'Z)
                   (char-int #\A)
                   (int-char 98)
                   (char/= #\a #\b #\c)
                   (char/= #\a #\b #\a)
                   (char-not-equal #\A #\a)
                   (char-lessp #\A #\b)
                   (char-greaterp #\b #\A)
                   (char-not-lessp #\B #\a)
                   (char-not-greaterp #\A #\b)
                   (alpha-char-p #\A)
                   (alphanumericp #\7)
                   (digit-char 10 16)
                   (digit-char-p #\f 16)
                   (digit-char-p #\g 16)
                   (graphic-char-p #\Space)
                   (standard-char-p #\Newline)
                   (upper-case-p #\A)
                   (lower-case-p #\a)
                   (both-case-p #\A)
                   (char-name #\Newline)
                   (name-char "space")
                   (name-char "?")
                   char-code-limit
                   most-positive-char-code)"#,
        )
        .to_string(),
        "(#\\A #\\Z 65 #\\b T NIL NIL T T T T T T #\\A 15 NIL T T T T T \"Newline\" #\\SPACE #\\? 1114112 1114111)"
    );
}

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
        evaluate("(let ((values #(1 2))) (setf (elt (progn values) 1) 8) values)").to_string(),
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
        evaluate("(let ((bits #*010)) (setf (bit bits 1) 0) (bit bits 1))").to_string(),
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
            "(let ((values #(1 2 3)))
               (setf (subseq (progn values) 1 3) '(8 9))
               values)",
        )
        .to_string(),
        "#(1 8 9)"
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
            "(let ((plist (list :a 1)) (reads 0))
               (list
                 (setf (getf plist :a (incf reads)) 2)
                 (setf (getf plist :b (incf reads)) 3)
                 plist
                 reads))",
        )
        .to_string(),
        "(2 3 (:B 3 :A 2) 2)"
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
fn evaluates_cxr_operations_and_setf() {
    assert_eq!(
        evaluate(
            "(let ((xs (list (list 1 2) 3 4)))
               (list (caar xs) (cadr xs) (cdar xs) (cddr xs)))",
        )
        .to_string(),
        "(1 3 (2) (4))"
    );
    assert_eq!(
        evaluate("(let ((xs (list (list 1 2) (list 3 4)))) (setf (caar xs) 9) xs)").to_string(),
        "((9 2) (3 4))"
    );
    assert_eq!(
        evaluate("(let ((xs (list (list 1 2) 3 4))) (setf (cadr xs) 9) xs)").to_string(),
        "((1 2) 9 4)"
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list (list 1 2) (list 3 4))))
               (setf (cdar xs) (list 9 10))
               xs)",
        )
        .to_string(),
        "((1 9 10) (3 4))"
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list 1 2 3 4)))
               (setf (cddr xs) (list 9 10))
               xs)",
        )
        .to_string(),
        "(1 2 9 10)"
    );
    assert_eq!(
        evaluate(
            "(let ((count 0) (xs (list (list (list 1 2)))))
               (setf (caar (nth (progn (incf count) 0) xs)) 9)
               (list count xs))",
        )
        .to_string(),
        "(1 (((9 2))))"
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list (list (list 1 2) 3) (list (list 4 5) 6))))
               (list
                 (caaar xs) (caadr xs) (cadar xs) (caddr xs)
                 (cdaar xs) (cdadr xs) (cddar xs) (cdddr xs)))",
        )
        .to_string(),
        "(1 (4 5) 3 NIL (2) (6) NIL NIL)"
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list
                         (list (list (list 1 2) (list 3 4))
                               (list (list 5 6) (list 7 8)))
                         (list (list (list 9 10) (list 11 12))
                               (list (list 13 14) (list 15 16))))))
               (list
                 (caaaar xs) (caaadr xs) (caadar xs) (caaddr xs)
                 (cadaar xs) (cadadr xs) (caddar xs) (cadddr xs)
                 (cdaaar xs) (cdaadr xs) (cdadar xs) (cdaddr xs)
                 (cddaar xs) (cddadr xs) (cdddar xs) (cddddr xs)))",
        )
        .to_string(),
        "(1 (9 10) (5 6) NIL (3 4) ((13 14) (15 16)) NIL NIL (2) ((11 12)) ((7 8)) NIL NIL NIL NIL NIL)"
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list (list (list 1 2) 3) (list (list 4 5) 6))))
               (setf (caaar xs) 9)
               xs)",
        )
        .to_string(),
        "(((9 2) 3) ((4 5) 6))"
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list 1 2 3 4 5)))
               (setf (cdddr xs) (list 9 10))
               xs)",
        )
        .to_string(),
        "(1 2 3 9 10)"
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list 1 2 3 4 5 6)))
               (setf (cddddr xs) (list 9 10))
               xs)",
        )
        .to_string(),
        "(1 2 3 4 9 10)"
    );
}

#[test]
fn evaluates_setf_getf_optional_default_contract() {
    assert_eq!(
        evaluate(
            "(let ((key-reads 0)
                   (plist-reads 0)
                   (default-reads 0)
                   (value-reads 0))
               (let ((plist (progn (incf plist-reads) (list :key 0))))
                 (list
                   (setf
                     (getf plist
                           (progn (incf key-reads) :key)
                           (progn (incf default-reads) 99))
                     (progn (incf value-reads) 42))
                   (getf plist :key)
                   key-reads plist-reads default-reads value-reads)))",
        )
        .to_string(),
        "(42 42 1 1 1 1)"
    );
}

#[test]
fn evaluates_push_pop_and_psetf() {
    assert_eq!(
        evaluate(
            "(let ((xs (list 2 3)))
               (list (push 1 xs) xs (pop xs) xs))",
        )
        .to_string(),
        "((1 2 3) (1 2 3) 1 (2 3))"
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list 10 20)))
               (list (push 5 (cdr xs)) xs))",
        )
        .to_string(),
        "((5 20) (10 5 20))"
    );
    assert_eq!(
        evaluate(
            "(let ((a 0) (b 0))
               (list (psetf a 1 b 2) a b))",
        )
        .to_string(),
        "(NIL 1 2)"
    );
}

#[test]
fn evaluates_single_pair_psetf_returns_nil() {
    assert_eq!(
        evaluate("(let ((a 0)) (list (psetf a 3) a))").to_string(),
        "(NIL 3)"
    );
}

#[test]
fn evaluates_psetf_places_and_values_in_order_before_stores() {
    assert_eq!(
        evaluate(
            "(let ((events nil) (cell (vector 0 0)))
               (list
                 (psetf
                   (aref cell (progn (push :place-1 events) 0))
                   (progn (push :value-1 events) 1)
                   (aref cell (progn (push (aref cell 0) events) 1))
                   (progn (push :value-2 events) 2))
                 (reverse events)
                 cell))",
        )
        .to_string(),
        "(NIL (:PLACE-1 :VALUE-1 0 :VALUE-2) #(1 2))"
    );
}

#[test]
fn evaluates_pushnew() {
    assert_eq!(
        evaluate(
            "(let ((xs (list 1 2)))
               (list (pushnew 2 xs) (pushnew 3 xs) xs))",
        )
        .to_string(),
        "((1 2) (3 1 2) (3 1 2))"
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list (list 1 :a))))
               (list (pushnew (list 1 :b) xs :key #'car :test #'eql)
                     (pushnew (list 1 :c) xs :key #'car :test-not #'equal)))",
        )
        .to_string(),
        "(((1 :A)) ((1 :C) (1 :A)))"
    );
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
fn evaluates_long_form_defsetf() {
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *long-defsetf-cells* (list 1 2))
               (defun long-defsetf-reader (index)
                 (nth index *long-defsetf-cells*))
               (defsetf long-defsetf-reader (index) (new-value)
                 `(setf (nth ,index *long-defsetf-cells*) ,new-value))
               (setf (long-defsetf-reader 1) 42)
               (long-defsetf-reader 1))",
        )
        .to_string(),
        "42"
    );
}

#[test]
fn evaluates_long_form_defsetf_with_multiple_stores() {
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *multiple-long-defsetf-cells* (list 1 2))
               (defun multiple-long-defsetf-reader (index)
                 (nth index *multiple-long-defsetf-cells*))
               (defsetf multiple-long-defsetf-reader (index) (first-value second-value)
                 `(progn
                    (setf (nth ,index *multiple-long-defsetf-cells*) ,first-value)
                    (setf (nth (+ ,index 1) *multiple-long-defsetf-cells*) ,second-value)))
               (setf (multiple-long-defsetf-reader 0) (values 7 8))
               (let ((value nil))
                 (setf value (values 9 10))
                 (list *multiple-long-defsetf-cells* value)))",
        )
        .to_string(),
        "((7 8) 9)"
    );
}

#[test]
fn evaluates_setf_function_designator() {
    assert_eq!(
        evaluate(
            "(progn
               (defun setf-function-writer (value) (+ value 10))
               (defsetf setf-function-reader setf-function-writer)
               (funcall #'(setf setf-function-reader) 5))",
        )
        .to_string(),
        "15"
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
fn evaluates_symbol_properties_and_setf_get() {
    assert_eq!(
        evaluate(
            r#"(let ((symbol (make-symbol "foo"))
                    (other (make-symbol "foo")))
                (list
                  (get symbol :missing)
                  (get symbol :missing :default)
                  (putprop symbol 10 :answer)
                  (get symbol :answer)
                  (setf (get symbol :answer) 11)
                  (get symbol :answer)
                  (symbol-plist symbol)
                  (get other :answer)
                  (remprop symbol :answer)
                  (get symbol :answer :default)
                  (remprop symbol :answer)
                  (symbol-plist symbol)))"#,
        )
        .to_string(),
        "(NIL :DEFAULT 10 10 11 11 (:ANSWER 11) NIL T :DEFAULT NIL NIL)",
    );
}

#[test]
fn evaluates_incf_and_decf_symbol_places() {
    assert_eq!(
        evaluate(
            "(let ((x 10) (delta 2))
               (list (incf x) x (incf x delta) (decf x) (decf x delta) x))",
        )
        .to_string(),
        "(11 11 13 12 10 10)"
    );
}

#[test]
fn evaluates_incf_and_decf_generalized_places() {
    assert_eq!(
        evaluate(
            "(let ((xs (list 10)) (delta 2))
               (list (incf (car xs) delta) xs (decf (car xs)) xs))",
        )
        .to_string(),
        "(12 (12) 11 (11))"
    );
}

#[test]
fn evaluates_rotatef_and_shiftf() {
    assert_eq!(
        evaluate(
            "(let ((a 1) (b 2) (c 3))
               (list (rotatef a b c) a b c))",
        )
        .to_string(),
        "(NIL 2 3 1)"
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list 1 2)))
               (list (shiftf (car xs) (car (cdr xs)) 9) xs))",
        )
        .to_string(),
        "(1 (2 9))"
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list 10 20 30)) (i 0))
               (list (shiftf (nth i xs) (nth (incf i) xs) 99) i xs))",
        )
        .to_string(),
        "(10 1 (20 99 30))"
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list 10 20 30)) (i 0))
               (list (rotatef (nth i xs) (nth (incf i) xs)) i xs))",
        )
        .to_string(),
        "(NIL 1 (20 10 30))"
    );
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
fn packages_reject_unknown_used_packages() {
    let error = Runtime::new()
        .eval_source(
            "(defpackage :unknown-use-eval
                (:use :package-does-not-exist-eval))",
        )
        .unwrap_err();

    assert!(matches!(error, ncl_runtime::RuntimeError::Package { .. }));
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
                         (symbol-name found) (symbol-package found)
                         (packagep (symbol-package found))
                         (string= (package-name (symbol-package found)) "SYMBOLS")
                         (eq (symbol-package found) (find-package :symbols)))))
               (multiple-value-bind (symbol status) (intern "foo" :keyword)
                 (list symbol status (symbol-name symbol) (symbol-package symbol)
                       (packagep (symbol-package symbol))
                       (string= (package-name (symbol-package symbol)) "KEYWORD")))
               (multiple-value-bind (missing status) (find-symbol "missing" :symbols)
                 (list missing status))"#,
        )
        .unwrap();

    assert_eq!(
        values[1].to_string(),
        "(T NIL :INTERNAL \"foo\" #<PACKAGE \"SYMBOLS\"> T T T)"
    );
    assert_eq!(
        values[2].to_string(),
        "(:|foo| NIL \"foo\" #<PACKAGE \"KEYWORD\"> T T)"
    );
    assert_eq!(values[3].to_string(), "(NIL NIL)");
}

#[test]
fn finds_all_symbols_across_packages() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(defpackage :find-all-one-eval)
               (defpackage :find-all-two-eval)
               (intern "NCL-FIND-ALL-SYMBOLS-TEST" :find-all-one-eval)
               (intern "NCL-FIND-ALL-SYMBOLS-TEST" :find-all-two-eval)
               (let ((symbols (find-all-symbols "NCL-FIND-ALL-SYMBOLS-TEST")))
                 (list (length symbols)
                       (symbol-name (car symbols))
                       (package-name (symbol-package (car symbols)))
                       (package-name (symbol-package (car (cdr symbols))))))"#,
        )
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        r#"(2 "NCL-FIND-ALL-SYMBOLS-TEST" "FIND-ALL-ONE-EVAL" "FIND-ALL-TWO-EVAL")"#
    );
}

#[test]
fn finds_all_symbols_deduplicates_imports_and_returns_keywords() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(defpackage :find-all-source-eval)
               (defpackage :find-all-target-eval)
               (let ((source (intern "NCL-FIND-ALL-IMPORTED-TEST" :find-all-source-eval)))
                 (import (list source) :find-all-target-eval)
                 (let ((imported (find-all-symbols "NCL-FIND-ALL-IMPORTED-TEST"))
                       (keyword (intern "NCL-FIND-ALL-KEYWORD-TEST" :keyword)))
                   (list (length imported)
                         (eq source (car imported))
                         (package-name (symbol-package (car imported)))
                         (length (find-all-symbols "NCL-FIND-ALL-KEYWORD-TEST"))
                         (eq keyword (car (find-all-symbols "NCL-FIND-ALL-KEYWORD-TEST")))
                         (keywordp (car (find-all-symbols "NCL-FIND-ALL-KEYWORD-TEST"))))))"#,
        )
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        "(1 T \"FIND-ALL-SOURCE-EVAL\" 1 T T)"
    );
}

#[test]
fn interns_string_symbol_names_without_case_folding() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(defpackage :case-probe-eval)
               (let ((lower (intern "foo" :case-probe-eval))
                     (upper (intern "FOO" :case-probe-eval)))
                 (list (symbol-name lower)
                       (symbol-name upper)
                       (not (eq lower upper))
                       (eq lower (find-symbol "foo" :case-probe-eval))
                       (eq upper (find-symbol "FOO" :case-probe-eval))))"#,
        )
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        "(\"foo\" \"FOO\" T T T)"
    );
}

#[test]
fn package_import_preserves_exact_symbol_identity() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(defpackage :exact-source-eval)
               (defpackage :exact-target-eval)
               (let ((source (intern "foo" :exact-source-eval)))
                 (import (list source) :exact-target-eval)
                 (let ((target (find-symbol "foo" :exact-target-eval)))
                   (list (eq source target)
                         (symbol-name target)
                         (symbolp target)
                         (typep target 'symbol))))"#,
        )
        .unwrap();

    assert_eq!(values.last().unwrap().to_string(), r#"(T "foo" T T)"#);
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
        r#"(T T "PACKAGE-INSPECT-EVAL" NIL NIL "COMMON-LISP" T)"#
    );
}

#[test]
fn package_introspection_lists_nicknames_shadowing_symbols_and_users() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(defpackage :package-api-source-eval
                 (:use :common-lisp)
                 (:nicknames :package-api-alias-eval))
               (defpackage :package-api-consumer-eval
                 (:use :common-lisp :package-api-source-eval))
               (shadow '(:local) :package-api-consumer-eval)
               (let ((source (find-package :package-api-source-eval))
                     (consumer (find-package :package-api-consumer-eval)))
                 (list (package-nicknames source)
                       (mapcar (function symbol-name)
                               (package-shadowing-symbols consumer))
                       (mapcar (function package-name)
                               (package-used-by-list source))))"#,
        )
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        r#"(("PACKAGE-API-ALIAS-EVAL") ("LOCAL") ("PACKAGE-API-CONSUMER-EVAL"))"#
    );
}

#[test]
fn make_package_supports_name_options_and_documentation() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(let ((package (make-package :make-package-eval
                                  :nicknames '(:make-package-alias-eval)
                                  :use '(:common-lisp)
                                  :size 32
                                  :documentation "make-package-doc")))
               (list (packagep package)
                     (string= (package-name package) "MAKE-PACKAGE-EVAL")
                     (eq package (find-package :make-package-alias-eval))
                     (string= (documentation package t) "make-package-doc")
                     (package-name (car (package-use-list package)))))"#,
        )
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        r#"(T T T T "COMMON-LISP")"#
    );

    let error = runtime
        .eval_source("(make-package :make-package-size-invalid-eval :size -1)")
        .unwrap_err();
    assert!(error.to_string().contains("make-package :size"));
}

#[test]
fn delete_package_removes_package_and_nicknames() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(let ((package (make-package :delete-package-eval
                                  :nicknames '(:delete-package-alias-eval)
                                  :use '(:common-lisp))))
               (list (delete-package package)
                     (find-package :delete-package-eval)
                     (find-package :delete-package-alias-eval)
                     (packagep package)))"#,
        )
        .unwrap();

    assert_eq!(values.last().unwrap().to_string(), r#"(T NIL NIL T)"#);

    let used_by_error = runtime
        .eval_source(
            r#"(let ((package (make-package :delete-package-used-eval :use nil)))
               (make-package :delete-package-user-eval
                             :use '(:delete-package-used-eval))
               (delete-package package))"#,
        )
        .unwrap_err();
    assert!(used_by_error.to_string().contains("used by"));

    let current_error = runtime
        .eval_source(
            r#"(let ((package (make-package :delete-package-current-eval :use nil)))
               (let ((*package* package))
                 (delete-package package)))"#,
        )
        .unwrap_err();
    assert!(
        current_error.to_string().contains("*PACKAGE*"),
        "unexpected current-package error: {}",
        current_error
    );

    let missing_error = runtime
        .eval_source("(delete-package :delete-package-missing-eval)")
        .unwrap_err();
    assert!(missing_error.to_string().contains("unknown package"));
}

#[test]
fn evaluates_rename_package() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(let ((package (make-package :rename-package-eval
                                  :nicknames '(:rename-package-alias-eval)
                                  :use '(:common-lisp))))
               (let ((renamed (rename-package package
                                               :renamed-package-eval
                                               '(:renamed-package-alias-eval))))
                 (list (eq package renamed)
                       (string= (package-name package)
                                "RENAMED-PACKAGE-EVAL")
                       (null (find-package :rename-package-eval))
                       (null (find-package :rename-package-alias-eval))
                       (string= (package-name
                                 (find-package :renamed-package-alias-eval))
                                "RENAMED-PACKAGE-EVAL")
                       (equal (package-nicknames package)
                              '("RENAMED-PACKAGE-ALIAS-EVAL"))
                       (progn
                         (rename-package package :renamed-package-second-eval)
                         (null (package-nicknames package)))
                       (null (find-package :renamed-package-alias-eval))
                       (string= (package-name package)
                                "RENAMED-PACKAGE-SECOND-EVAL")
                       (string= (package-name (car (package-use-list package)))
                                "COMMON-LISP"))))"#,
        )
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        r#"(T T T T T T T T T T)"#
    );
}

#[test]
fn evaluates_clos_standard_metaclass_option() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(progn
                 (defclass standard-metaclass-eval () ()
                   (:metaclass standard-class))
                 (class-name (find-class 'standard-metaclass-eval)))"#,
        )
        .unwrap();
    assert_eq!(
        values.last().unwrap().to_string(),
        "STANDARD-METACLASS-EVAL"
    );

    let error = runtime
        .eval_source(
            r#"(defclass invalid-metaclass-eval () ()
                 (:metaclass standard-object))"#,
        )
        .unwrap_err();
    assert!(error.to_string().contains("metaclass"));
}

#[test]
fn make_package_defaults_to_no_used_packages() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(let ((package (make-package :make-package-default-eval)))
               (list (package-name package) (package-use-list package)))"#,
        )
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        r#"("MAKE-PACKAGE-DEFAULT-EVAL" NIL)"#
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
                         (unintern '(package-consumer-import-eval::answer)
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
        r#"((42 T) (42 T) 7 T T (NIL NIL))"#
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
fn defpackage_string_symbol_designators_preserve_case() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(defpackage :package-string-designator-eval
                 (:use :common-lisp)
                 (:intern "foo")
                 (:export "FOO"))
               (multiple-value-bind (symbol status)
                   (find-symbol "foo" :package-string-designator-eval)
                 (multiple-value-bind (upper upper-status)
                     (find-symbol "FOO" :package-string-designator-eval)
                   (list (and symbol (symbol-name symbol))
                         status
                         (and upper (symbol-name upper))
                         upper-status)))"#,
        )
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        r#"("foo" :INTERNAL "FOO" :EXTERNAL)"#
    );
}

#[test]
fn package_name_designators_preserve_case() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(progn
                  (defpackage "package-string-case-eval"
                    (:use :common-lisp)
                    (:nicknames "package-string-alias-case-eval"))
                  (make-package "package-make-case-eval"
                    :nicknames '("package-make-alias-case-eval"))
                  (defpackage |Package-Escaped-Case-Eval|)
                  (list
                    (package-name (find-package "package-string-case-eval"))
                    (package-name (find-package "package-string-alias-case-eval"))
                    (find-package "PACKAGE-STRING-CASE-EVAL")
                    (package-name (find-package "package-make-case-eval"))
                    (package-name (find-package "package-make-alias-case-eval"))
                    (find-package "PACKAGE-MAKE-CASE-EVAL")
                    (package-name (find-package '|Package-Escaped-Case-Eval|))
                    (find-package "PACKAGE-ESCAPED-CASE-EVAL")))"#,
        )
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        r#"("package-string-case-eval" "package-string-case-eval" NIL "package-make-case-eval" "package-make-case-eval" NIL "Package-Escaped-Case-Eval" NIL)"#
    );
}

#[test]
fn defpackage_string_import_designators_preserve_case() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(defpackage :package-string-source-eval
                 (:use :common-lisp)
                 (:export "foo"))
               (defpackage :package-string-target-eval
                 (:use :common-lisp)
                 (:import-from :package-string-source-eval "foo"))
               (multiple-value-bind (symbol status)
                   (find-symbol "foo" :package-string-target-eval)
                 (multiple-value-bind (upper upper-status)
                     (find-symbol "FOO" :package-string-target-eval)
                   (list (and symbol (symbol-name symbol))
                         status
                         (and upper (symbol-name upper))
                         upper-status)))"#,
        )
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        r#"("foo" :INTERNAL NIL NIL)"#
    );
}

#[test]
fn unintern_checks_the_source_package_identity() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(defpackage :package-identity-provider-eval (:use :common-lisp))
               (defpackage :package-identity-target-eval (:use :common-lisp))
               (in-package :package-identity-provider-eval)
               (define answer 41)
               (export '(:answer))
               (in-package :package-identity-target-eval)
               (define answer 7)
               (multiple-value-bind (provider-symbol provider-status)
                   (find-symbol "ANSWER" :package-identity-provider-eval)
                 (multiple-value-bind (target-symbol target-status)
                     (find-symbol "ANSWER" :package-identity-target-eval)
                   (list (unintern provider-symbol :package-identity-target-eval)
                         (eq target-symbol
                             (find-symbol "ANSWER" :package-identity-target-eval))
                         (eq provider-symbol target-symbol)
                         provider-status
                         target-status
                         (unintern target-symbol :package-identity-target-eval)
                         (find-symbol "ANSWER" :package-identity-target-eval))))"#,
        )
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        r#"(NIL T NIL :EXTERNAL :INTERNAL T NIL)"#
    );
}

#[test]
fn rejects_string_unintern_designator() {
    let error = Runtime::new()
        .eval_source(r#"(unintern "answer")"#)
        .unwrap_err();

    assert!(matches!(
        error,
        RuntimeError::Type {
            expected,
            actual,
            ..
        } if expected == "SYMBOL" && actual == "STRING"
    ));
}

#[test]
fn package_special_variable_tracks_in_package() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(list (boundp '*package*)
                     (package-name *package*)
                     (progn
                       (defpackage :package-special-eval (:use :common-lisp))
                       (in-package :package-special-eval)
                       (package-name *package*)))"#,
        )
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        r#"(T "NCL-USER" "PACKAGE-SPECIAL-EVAL")"#
    );
}

#[test]
fn defpackage_unintern_option_removes_symbols() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(defpackage :package-unintern-eval
                 (:use :common-lisp)
                 (:intern :temporary :kept)
                 (:export :kept)
                 (:unintern :temporary))
               (multiple-value-bind (temporary temporary-status)
                   (find-symbol "TEMPORARY" :package-unintern-eval)
                 (multiple-value-bind (kept kept-status)
                     (find-symbol "KEPT" :package-unintern-eval)
                   (list temporary temporary-status
                         (symbol-name kept) kept-status)))"#,
        )
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        r#"(NIL NIL "KEPT" :EXTERNAL)"#
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
fn string_streams_support_output_ranges() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(let ((output (make-string-output-stream)))
               (list (write-string "abcdef" output :start 1 :end 4)
                     (write-line "abcdef" output :start 2 :end 5)
                     (get-output-stream-string output)))"#,
        )
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        r#"("abcdef" "abcdef" "bcdcde\n")"#
    );
}

#[test]
fn composite_streams_delegate_to_component_streams() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(let* ((input-one (make-string-input-stream "ab"))
                    (input-two (make-string-input-stream "cd"))
                    (input (make-concatenated-stream input-one input-two))
                    (output-one (make-string-output-stream))
                    (output-two (make-string-output-stream))
                    (output (make-broadcast-stream output-one output-two))
                    (two-way-input (make-string-input-stream "q"))
                    (two-way (make-two-way-stream two-way-input output-one))
                    (echo-input (make-string-input-stream "z"))
                    (echo-output (make-string-output-stream))
                    (echo (make-echo-stream echo-input echo-output)))
               (list (input-stream-p input)
                     (output-stream-p input)
                     (read-char input nil)
                     (read-char input nil)
                     (read-char input nil)
                     (read-char input nil)
                     (read-char input nil)
                     (write-string "!" output)
                     (get-output-stream-string output-one)
                     (get-output-stream-string output-two)
                     (read-char two-way)
                     (write-char #\x two-way)
                     (get-output-stream-string output-one)
                     (read-char echo)
                     (get-output-stream-string echo-output)
                     (typep (make-broadcast-stream) 'stream)
                     (input-stream-p (make-broadcast-stream))))"#,
        )
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        r#"(T NIL #\a #\b #\c #\d NIL "!" "!" "!" #\q #\x "x" #\z "z" T NIL)"#
    );
}

#[test]
fn direct_concatenated_stream_streams_preserves_order_identity_and_shared_state() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(let* ((one (make-string-input-stream "ab"))
                    (two (make-string-input-stream "cd"))
                    (concatenated (make-concatenated-stream one two))
                    (components (concatenated-stream-streams concatenated))
                    (first (nth 0 components))
                    (second (nth 1 components)))
               (list (length components)
                     (eq one first)
                     (eq two second)
                     (read-char concatenated)
                     (read-char first)
                     (read-char concatenated)
                     (read-char second)
                     (read-char concatenated nil :eof)))"#,
        )
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        r#"(2 T T #\a #\b #\c #\d :EOF)"#
    );
}

#[test]
fn direct_two_way_stream_input_stream_returns_original_input() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(let* ((input (make-string-input-stream "q"))
                    (output (make-string-output-stream))
                    (two-way (make-two-way-stream input output)))
               (list (eq input (two-way-stream-input-stream two-way))
                     (read-char (two-way-stream-input-stream two-way) nil)))"#,
        )
        .unwrap();

    assert_eq!(values.last().unwrap().to_string(), r#"(T #\q)"#);
}

#[test]
fn direct_two_way_stream_output_stream_preserves_identity_and_state() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(let* ((input (make-string-input-stream "q"))
                    (output (make-string-output-stream))
                    (two-way (make-two-way-stream input output))
                    (accessor (two-way-stream-output-stream two-way)))
               (write-char #\x accessor)
               (list (eq output accessor)
                     (get-output-stream-string output)))"#,
        )
        .unwrap();

    assert_eq!(values.last().unwrap().to_string(), r#"(T "x")"#);
}

#[test]
fn direct_broadcast_stream_streams_preserves_order_identity_and_shared_state() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(let* ((one (make-string-output-stream))
                    (two (make-string-output-stream))
                    (broadcast (make-broadcast-stream one two))
                    (components (broadcast-stream-streams broadcast))
                    (first (nth 0 components))
                    (second (nth 1 components)))
               (list (length components)
                     (eq one first)
                     (eq two second)
                     (write-char #\x first)
                     (get-output-stream-string one)
                     (write-char #\y broadcast)
                     (get-output-stream-string first)
                     (get-output-stream-string second)))"#,
        )
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        r#"(2 T T #\x "x" #\y "y" "y")"#
    );
}

#[test]
fn direct_echo_stream_input_stream_preserves_identity_and_delegation() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(let* ((input (make-string-input-stream "z"))
                    (output (make-string-output-stream))
                    (echo (make-echo-stream input output))
                    (accessor (echo-stream-input-stream echo)))
               (list (eq input accessor)
                     (read-char echo)
                     (get-output-stream-string output)
                     (read-char accessor nil :eof)))"#,
        )
        .unwrap();

    assert_eq!(values.last().unwrap().to_string(), r#"(T #\z "z" :EOF)"#);
}

#[test]
fn direct_echo_stream_output_stream_preserves_identity_and_shared_state() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(let* ((input (make-string-input-stream "q"))
                    (output (make-string-output-stream))
                    (echo (make-echo-stream input output))
                    (accessor (echo-stream-output-stream echo)))
               (list (eq output accessor)
                     (write-char #\x accessor)
                     (get-output-stream-string output)
                     (write-char #\y echo)
                     (get-output-stream-string accessor)))"#,
        )
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        r#"(T #\x "x" #\y "y")"#
    );
}

#[test]
fn string_streams_report_open_state() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(let ((input (make-string-input-stream "x"))
                   (output (make-string-output-stream)))
               (list (open-stream-p input)
                     (open-stream-p output)
                     (progn (close input) (open-stream-p input))
                     (progn (close output) (open-stream-p output))))"#,
        )
        .unwrap();

    assert_eq!(values.last().unwrap().to_string(), "(T T NIL NIL)");
}

#[test]
fn string_streams_report_availability_and_element_type() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(let ((input (make-string-input-stream "a"))
                   (output (make-string-output-stream)))
               (list (listen input)
                     (read-char input)
                     (listen input)
                     (read-char input nil)
                     (listen input)
                     (stream-element-type input)
                     (stream-element-type output)
                     (stream-element-type (make-broadcast-stream))))"#,
        )
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        r"(T #\a NIL NIL NIL CHARACTER CHARACTER T)"
    );
}

#[test]
fn string_streams_reject_listen_on_closed_input() {
    assert!(Runtime::new()
        .eval_source(
            r#"(let ((input (make-string-input-stream "a")))
               (close input)
               (listen input))"#,
        )
        .is_err());
}

#[test]
fn string_streams_clear_pending_input_and_output() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(let ((input (make-string-input-stream "a"))
                   (output (make-string-output-stream)))
               (list (progn (read-char input)
                            (unread-char #\a input)
                            (clear-input input)
                            (read-char input nil))
                     (progn (write-string "discard" output)
                            (clear-output output)
                            (get-output-stream-string output))
                     (clear-output)))"#,
        )
        .unwrap();

    assert_eq!(values.last().unwrap().to_string(), r#"(NIL "" NIL)"#);
}

#[test]
fn string_streams_clear_output_propagates_to_broadcast_streams() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(let ((one (make-string-output-stream))
                   (two (make-string-output-stream)))
               (let ((broadcast (make-broadcast-stream one two)))
                 (write-string "discard" broadcast)
                 (clear-output broadcast)
                 (list (get-output-stream-string one)
                       (get-output-stream-string two))))"#,
        )
        .unwrap();

    assert_eq!(values.last().unwrap().to_string(), r#"("" "")"#);
}

#[test]
fn string_streams_reject_clear_input_on_closed_input() {
    assert!(Runtime::new()
        .eval_source(
            r#"(let ((input (make-string-input-stream "a")))
               (close input)
               (clear-input input))"#,
        )
        .is_err());
}

#[test]
fn string_streams_ignore_clear_output_on_input() {
    let values = Runtime::new()
        .eval_source(
            r#"(let ((input (make-string-input-stream "a")))
               (list (clear-output input)
                     (read-char input nil)))"#,
        )
        .unwrap();

    assert_eq!(values.last().unwrap().to_string(), "(NIL #\\a)");
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
fn file_streams_overwrite_preserves_tail() {
    let path = std::env::temp_dir().join(format!(
        "ncl-overwrite-preserves-tail-evaluator-{}",
        std::process::id()
    ));
    let pathname = format!("{:?}", path.to_string_lossy().to_string());
    std::fs::write(&path, "abc").unwrap();
    let source = format!(
        r#"(let ((stream (open {pathname}
                              :direction :output
                              :if-exists :overwrite)))
               (write-char #\X stream)
               (close stream))"#,
        pathname = pathname
    );

    assert_eq!(evaluate(&source).to_string(), "T");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "Xbc");
    let _ = std::fs::remove_file(path);
}

#[test]
fn file_streams_validate_element_type() {
    let path = std::env::temp_dir().join(format!(
        "ncl-file-stream-element-type-evaluator-{}",
        std::process::id()
    ));
    let pathname = format!("{:?}", path.to_string_lossy().to_string());
    std::fs::write(&path, "abc").unwrap();
    let source = format!(
        r#"(let ((stream (open {pathname} :element-type 'character)))
               (list (stream-element-type stream)
                     (read-char stream)
                     (close stream)))"#,
        pathname = pathname
    );

    assert_eq!(evaluate(&source).to_string(), "(CHARACTER #\\a T)");
    let invalid_source = format!(r#"(open {pathname} :element-type 'bit)"#, pathname = pathname);
    let error = Runtime::new().eval_source(&invalid_source).unwrap_err();
    assert!(error
        .to_string()
        .contains("open only supports :element-type CHARACTER"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn file_streams_validate_external_format() {
    let path = std::env::temp_dir().join(format!(
        "ncl-file-stream-external-format-evaluator-{}",
        std::process::id()
    ));
    let pathname = format!("{:?}", path.to_string_lossy().to_string());
    std::fs::write(&path, "abc").unwrap();
    let source = format!(
        r#"(let ((default (open {pathname} :external-format :default))
                   (utf8 (open {pathname} :external-format :utf-8)))
               (list (read-char default)
                     (read-char utf8)
                     (close default)
                     (close utf8)))"#,
        pathname = pathname
    );

    assert_eq!(evaluate(&source).to_string(), "(#\\a #\\a T T)");
    let invalid_source = format!(r#"(open {pathname} :external-format :latin-1)"#, pathname = pathname);
    let error = Runtime::new().eval_source(&invalid_source).unwrap_err();
    assert!(error
        .to_string()
        .contains("open only supports :external-format DEFAULT or UTF-8"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn with_open_stream_closes_on_normal_and_nonlocal_exit() {
    assert_eq!(
        evaluate(
            r#"(let ((normal nil)
                   (escaped nil))
               (list
                 (with-open-stream (stream (make-string-output-stream))
                   (setq normal stream)
                   (write-string "hello" stream)
                   (get-output-stream-string stream))
                 (open-stream-p normal)
                 (block done
                   (with-open-stream (stream (make-string-output-stream))
                     (setq escaped stream)
                     (return-from done :escaped)))
                 (open-stream-p escaped)))"#,
        )
        .to_string(),
        "(\"hello\" NIL :ESCAPED NIL)"
    );
}

#[test]
fn with_output_to_string_returns_string_and_closes_on_nonlocal_exit() {
    assert_eq!(
        evaluate(
            r#"(let ((normal nil)
                   (escaped nil))
               (list
                 (with-output-to-string (stream)
                   (setq normal stream)
                   (write-string "hello" stream)
                   (values :ignored :second))
                 (open-stream-p normal)
                 (block done
                   (with-output-to-string (stream)
                     (setq escaped stream)
                     (return-from done :escaped)))
                 (open-stream-p escaped)))"#,
        )
        .to_string(),
        "(\"hello\" NIL :ESCAPED NIL)"
    );
}

#[test]
fn with_output_to_string_accepts_literal_nil_string_form() {
    assert_eq!(
        evaluate(
            r#"(with-output-to-string (stream nil)
                 (write-string "hello" stream))"#,
        )
        .to_string(),
        "\"hello\""
    );
}

#[test]
fn with_output_to_string_rejects_empty_binding() {
    assert!(Runtime::new()
        .eval_source("(with-output-to-string ())")
        .is_err());
}

#[test]
fn with_output_to_string_accepts_character_element_type() {
    assert_eq!(
        evaluate("(stream-element-type (make-string-output-stream :element-type 'character))")
            .to_string(),
        "CHARACTER"
    );
    assert_eq!(
        evaluate(
            r#"(with-output-to-string (stream nil :element-type 'character)
                 (write-string "hello" stream))"#,
        )
        .to_string(),
        "\"hello\""
    );
}

#[test]
fn with_input_from_string_reads_and_closes_on_nonlocal_exit() {
    assert_eq!(
        evaluate(
            r#"(let ((normal nil)
                   (escaped nil))
               (list
                 (with-input-from-string (stream "hello")
                   (setq normal stream)
                   (list (char= (read-char stream) #\h)
                         (char= (read-char stream) #\e)))
                 (open-stream-p normal)
                 (block done
                   (with-input-from-string (stream "ignored")
                     (setq escaped stream)
                     (return-from done :escaped)))
                 (open-stream-p escaped)))"#,
        )
        .to_string(),
        "((T T) NIL :ESCAPED NIL)"
    );
}

#[test]
fn with_input_from_string_honors_bounds_and_updates_index_only_on_normal_exit() {
    assert_eq!(
        evaluate(
            r#"(let ((normal-index -1)
                   (escaped-index -1))
               (list
                 (with-input-from-string (stream "abcdef"
                                            :start 1
                                            :end 4
                                            :index normal-index)
                   (list (char= (read-char stream) #\b)
                         (char= (read-char stream) #\c)
                         (char= (read-char stream) #\d)
                         (null (read-char stream nil))))
                 normal-index
                 (block done
                   (with-input-from-string (stream "abcdef" :index escaped-index)
                     (read-char stream)
                     (return-from done :escaped)))
                 escaped-index))"#,
        )
        .to_string(),
        "((T T T T) 4 :ESCAPED -1)"
    );
}

#[test]
fn string_input_streams_use_unicode_character_bounds() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r#"(let ((index -1))
                 (list
                   (with-input-from-string (stream "Aé😀B"
                                              :start 1
                                              :end 3
                                              :index index)
                     (list (read-char stream)
                           (read-char stream)
                           (read-char stream nil :eof)))
                   index
                   (length "Aé😀")))"#,
        )
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        "((#\\é #\\😀 :EOF) 3 3)"
    );
}

#[test]
fn file_streams_flush_output_before_close() {
    let path = std::env::temp_dir().join(format!(
        "ncl-flush-output-evaluator-{}",
        std::process::id()
    ));
    let pathname = format!("{:?}", path.to_string_lossy().to_string());
    let source = format!(
        r#"(let ((output (open {pathname}
                              :direction :output
                              :if-exists :supersede)))
               (write-string "hello" output)
               (force-output output)
               (let ((first (with-open-file (input {pathname})
                              (file-length input))))
                 (write-char #\! output)
                 (finish-output output)
                 (let ((second (with-open-file (input {pathname})
                                 (file-length input))))
                   (close output)
                   (list first second))))"#,
        pathname = pathname
    );

    assert_eq!(evaluate(&source).to_string(), "(5 6)");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello!");
    let _ = std::fs::remove_file(path);
}

#[test]
fn file_streams_report_and_set_position_and_length() {
    let path = std::env::temp_dir().join(format!(
        "ncl-file-stream-position-evaluator-{}",
        std::process::id()
    ));
    let pathname = format!("{:?}", path.to_string_lossy().to_string());
    let source = format!(
        r#"(progn
               (with-open-file (stream {pathname}
                                :direction :output
                                :if-exists :supersede)
                 (write-string "abc" stream))
               (list
                 (with-open-file (input {pathname})
                   (list (file-position input)
                         (read-char input)
                         (file-position input)
                         (file-length input)
                         (file-position input :end)
                         (file-position input)
                         (file-position input :start)
                         (read-char input)
                         (file-position (make-string-input-stream "x"))))
                 (with-open-file (output {pathname}
                                  :direction :output
                                  :if-exists :append)
                   (list (file-position output)
                         (file-length output)
                         (file-position output 1)
                         (write-char #\Z output)
                         (file-position output)))))"#,
        pathname = pathname
    );

    assert_eq!(
        evaluate(&source).to_string(),
        "((0 #\\a 1 3 T 3 T #\\a NIL) (3 3 T #\\Z 2))"
    );
    assert!(Runtime::new()
        .eval_source("(file-length (make-string-input-stream \"x\"))")
        .is_err());
    let too_large = format!(
        r#"(with-open-file (input {pathname}) (file-position input 4))"#,
        pathname = pathname
    );
    assert!(Runtime::new().eval_source(&too_large).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "aZc");
    let _ = std::fs::remove_file(path);
}

#[test]
fn file_streams_use_unicode_character_positions_for_append_and_overwrite() {
    let path = std::env::temp_dir().join(format!(
        "ncl-unicode-file-stream-evaluator-{}",
        std::process::id()
    ));
    let pathname = format!("{:?}", path.to_string_lossy().to_string());
    std::fs::write(&path, "Aé😀B").unwrap();

    let source = format!(
        r#"(list
             (let ((append (open {pathname}
                                  :direction :output
                                  :if-exists :append)))
               (list (file-position append)
                     (file-length append)
                     (close append)))
             (let ((overwrite (open {pathname}
                                     :direction :output
                                     :if-exists :overwrite)))
               (list (file-position overwrite 1)
                     (write-char #\X overwrite)
                     (close overwrite)))
             (let ((append (open {pathname}
                                  :direction :output
                                  :if-exists :append)))
               (list (write-char #\! append)
                     (close append))))"#,
        pathname = pathname
    );
    let values = Runtime::new().eval_source(&source).unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        "((4 4 T) (T #\\X T) (#\\! T))"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "AX😀B!");
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
               (let ((probe-result
                       (let ((existing (open {pathname} :direction :probe))
                             (missing (open {missing_pathname} :direction :probe)))
                         (prog1 (list (streamp existing)
                                      (open-stream-p existing)
                                      (input-stream-p existing)
                                      (output-stream-p existing)
                                      (null missing))
                           (close existing)))))
                 (let ((stream (open {missing_pathname}
                                     :direction :output
                                     :if-does-not-exist :create)))
                   (write-string "discard" stream)
                   (close stream :abort t))
                 (list probe-result
                       (null (open {missing_pathname} :direction :probe)))))"#,
        pathname = pathname,
        missing_pathname = missing_pathname
    );

    assert_eq!(evaluate(&source).to_string(), "((T NIL NIL NIL T) T)");
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
fn evaluates_fixed_radix_integer_literals() {
    assert_eq!(
        evaluate("(list #b101 #o17 #x1f #x-10 (+ #b1 #x2))").to_string(),
        "(5 15 31 -16 3)"
    );
}

#[test]
fn evaluates_general_radix_integer_literals() {
    assert_eq!(
        evaluate("(list #2r101 #10r42 #36rZ #16r-ff)").to_string(),
        "(5 42 35 -255)"
    );
}

#[test]
fn evaluates_dotimes_empty_result_and_final_binding() {
    assert_eq!(
        evaluate("(dotimes (index 0 index) (setq ran t))").to_string(),
        "0"
    );
    assert_eq!(
        evaluate("(dotimes (index -2 index) (setq ran t))").to_string(),
        "0"
    );
    assert_eq!(
        evaluate("(dotimes (index 3 index) (setq last index))").to_string(),
        "3"
    );
}

#[test]
fn evaluates_do_and_do_star_with_parallel_and_sequential_bindings() {
    assert_eq!(
        evaluate(
            "(list
               (let ((i 9))
                 (do ((i 1) (j i)) ((= i 1) j)))
               (let ((i 9))
                 (do* ((i 1) (j i)) ((= i 1) j)))
               (do ((i 0 (1+ i)) (j 0 i)) ((= i 3) j))
               (do* ((i 0 (1+ i)) (j 0 i)) ((= i 3) j)))",
        )
        .to_string(),
        "(9 1 2 3)"
    );
}

#[test]
fn evaluates_do_with_implicit_block_and_tagbody() {
    assert_eq!(
        evaluate(
            "(do ((i 0 (1+ i)))
                 ((= i 3) -1)
               (if (= i 2) (go finished) (go next))
               finished
               (return-from nil 42)
               next)"
        )
        .to_string(),
        "42"
    );
}

#[test]
fn evaluates_prog_and_prog_star_with_parallel_and_sequential_bindings() {
    assert_eq!(
        evaluate(
            "(list
               (let ((i 9))
                 (prog ((i 1) (j i)) (return-from nil (list i j))))
               (let ((i 9))
                 (prog* ((i 1) (j i)) (return-from nil (list i j))))
               (prog () 42))",
        )
        .to_string(),
        "((1 9) (1 1) NIL)"
    );
}

#[test]
fn evaluates_prog_with_implicit_block_and_tagbody() {
    assert_eq!(
        evaluate(
            "(prog ((i 0))
               start
               (setq i (1+ i))
               (if (= i 2) (return-from nil i) (go start)))",
        )
        .to_string(),
        "2"
    );
}

#[test]
fn evaluates_return_as_an_implicit_nil_block_exit() {
    assert_eq!(
        evaluate(
            "(prog ((value 1))
               (return (+ value 41))
               (setq value 0))",
        )
        .to_string(),
        "42"
    );
    assert_eq!(evaluate("(prog () (return))").to_string(), "NIL");
}

#[test]
fn evaluates_dolist_accumulation_empty_list_and_final_binding() {
    assert_eq!(
        evaluate(
            "(let ((total 0))
               (dolist (item '(1 2 3) (list total item))
                 (setq total (+ total item))))"
        )
        .to_string(),
        "(6 NIL)"
    );
    assert_eq!(
        evaluate("(dolist (item nil item) (setq item 1))").to_string(),
        "NIL"
    );
}

#[test]
fn evaluates_destructuring_bind_with_nested_and_dotted_patterns() {
    assert_eq!(
        evaluate(
            "(destructuring-bind (first (second third)) (list 1 (list 2 3))
               (+ first (+ second third)))"
        )
        .to_string(),
        "6"
    );
    assert_eq!(
        evaluate(
            "(destructuring-bind (head . tail) (list 1 2 3)
               (+ head (car tail)))"
        )
        .to_string(),
        "3"
    );
}

#[test]
fn evaluates_destructuring_bind_lambda_list_parameters() {
    assert_eq!(
        evaluate(
            "(destructuring-bind (&whole whole (first second) &optional third)
               (list (list 1 2) 3)
               (list whole first second third))"
        )
        .to_string(),
        "(((1 2) 3) 1 2 3)",
    );
    assert_eq!(
        evaluate(
            "(destructuring-bind (first &optional (second (+ first 1) second-p)
                                  &key (scale 2 scale-p)
                                  &allow-other-keys
                                  &aux (total (+ first second scale)))
               (list 3 :scale 4 :ignored 9)
               (list first second second-p scale scale-p total))",
        )
        .to_string(),
        "(3 4 NIL 4 T 11)",
    );
    assert_eq!(
        evaluate(
            "(destructuring-bind (first &optional (second (+ first 1) second-p)
                                  &key (scale 2 scale-p))
               (list 3 5 :scale 6)
               (list first second second-p scale scale-p))",
        )
        .to_string(),
        "(3 5 T 6 T)",
    );
    assert_eq!(
        evaluate(
            "(destructuring-bind (foo &key (|Scale| 2 |Scale-p|)) (list 1 :|Scale| 4)
               (list foo |Scale| |Scale-p|))",
        )
        .to_string(),
        "(1 4 T)",
    );
    assert_eq!(
        evaluate(
            "(destructuring-bind (first &rest rest &aux (count (length rest)))
               (list 3 4 5)
               (list first rest count))",
        )
        .to_string(),
        "(3 (4 5) 2)",
    );
    assert_eq!(
        evaluate(
            "(destructuring-bind (first &foo second)
               (list 1 2 3)
               (list first second))",
        )
        .to_string(),
        "(1 3)",
    );
}

#[test]
fn rejects_escaped_keyword_for_unescaped_destructuring_parameter() {
    let error = Runtime::new()
        .eval_source(
            "(destructuring-bind (foo &key (scale 2))
               (list 1 :|Scale| 4)
               scale)",
        )
        .unwrap_err();
    assert!(matches!(
        error,
        ncl_runtime::RuntimeError::InvalidForm { message, .. }
            if message == "unknown keyword :Scale"
    ));
}

#[test]
fn evaluates_prog1_and_prog2_in_order_and_returns_retained_value() {
    assert_eq!(
        evaluate(
            "(let ((events nil))
               (list
                 (prog1
                   (progn (setq events (cons :first events)) 10)
                   (setq events (cons :second events))
                   (setq events (cons :third events)))
                 events))"
        )
        .to_string(),
        "(10 (:THIRD :SECOND :FIRST))"
    );
    assert_eq!(
        evaluate(
            "(let ((events nil))
               (list
                 (prog2
                   (progn (setq events (cons :first events)) 10)
                   (progn (setq events (cons :second events)) 20)
                   (setq events (cons :third events))
                   (setq events (cons :fourth events)))
                 events))"
        )
        .to_string(),
        "(20 (:FOURTH :THIRD :SECOND :FIRST))"
    );
}

#[test]
fn rejects_prog1_and_prog2_without_required_forms() {
    let prog1 = Runtime::new().eval_source("(prog1)").unwrap_err();
    assert!(matches!(
        prog1,
        ncl_runtime::RuntimeError::Arity {
            function,
            expected,
            actual: 0,
        } if function == "prog1" && expected == "at least one"
    ));

    let prog2 = Runtime::new().eval_source("(prog2 1)").unwrap_err();
    assert!(matches!(
        prog2,
        ncl_runtime::RuntimeError::Arity {
            function,
            expected,
            actual: 1,
        } if function == "prog2" && expected == "at least two"
    ));
}

#[test]
fn rejects_invalid_dotimes_and_dolist_forms() {
    let invalid_binding = Runtime::new().eval_source("(dotimes item)").unwrap_err();
    assert!(matches!(
        invalid_binding,
        ncl_runtime::RuntimeError::InvalidForm { .. }
    ));

    let invalid_count = Runtime::new()
        .eval_source("(dotimes (index nil) index)")
        .unwrap_err();
    assert!(matches!(
        invalid_count,
        ncl_runtime::RuntimeError::Type { expected, .. } if expected == "INTEGER"
    ));

    let invalid_list = Runtime::new()
        .eval_source("(dolist (item 42) item)")
        .unwrap_err();
    assert!(matches!(
        invalid_list,
        ncl_runtime::RuntimeError::Type { expected, .. } if expected == "LIST"
    ));
}

#[test]
fn evaluates_tagbody_and_go_with_forward_and_backward_jumps() {
    let source = r#"
        (let ((count 0))
          (tagbody
            start
            (setq count (+ count 1))
            (if (= count 3) (go done) (go start))
            done)
          count)
    "#;

    assert_eq!(evaluate(source).to_string(), "3");
}

#[test]
fn propagates_unmatched_go_through_ignore_errors() {
    let error = Runtime::new()
        .eval_source("(ignore-errors (go missing))")
        .unwrap_err();

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
fn tagbody_returns_nil_and_does_not_evaluate_labels() {
    assert_eq!(
        evaluate("(list (tagbody start done) 42)").to_string(),
        "(NIL 42)"
    );
}

#[test]
fn supports_integer_and_keyword_tags() {
    let source = r#"
        (let ((count 0))
          (tagbody
            10
            (setq count (+ count 1))
            (if (= count 2) (go :done) (go 10))
            :done)
          count)
    "#;

    assert_eq!(evaluate(source).to_string(), "2");
}

#[test]
fn captures_an_active_tagbody_target_in_a_closure() {
    let source = r#"
        (let ((value 0))
          (tagbody
            start
            (setq value 1)
            (funcall (lambda () (go done)))
            (setq value 99)
            done)
          value)
    "#;

    assert_eq!(evaluate(source).to_string(), "1");
}

#[test]
fn rejects_invalid_go_shapes_and_tags() {
    for source in ["(go)", "(go missing extra)", "(go 1.5)"] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
    assert!(Runtime::new().eval_source("(tagbody start start)").is_err());
}
