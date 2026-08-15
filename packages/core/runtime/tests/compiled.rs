use ncl_runtime::{Runtime, RuntimeError, Value};

fn evaluate(source: &str) -> Value {
    Runtime::new()
        .eval_compiled_source(source)
        .unwrap()
        .pop()
        .unwrap()
}

#[test]
fn compiled_supports_uninterned_symbols_and_gensym() {
    assert_eq!(
        evaluate(
            r#"(let ((symbol (make-symbol "foo")))
                (list (symbolp symbol)
                      (symbol-package symbol)
                      (symbol-name symbol)
                      (eq symbol symbol)
                      (eq '#:foo '#:foo)))"#,
        )
        .to_string(),
        r#"(T NIL "foo" T NIL)"#,
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
fn compiled_preserves_escaped_symbol_identity_across_namespaces() {
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
        evaluate(r#"(list (symbol-name :|foo|) (symbol-name :FOO) (eq :|foo| :FOO))"#)
            .to_string(),
        r#"("foo" "FOO" NIL)"#,
    );
}

#[test]
fn compiled_preserves_exact_symbol_values_for_dynamic_operations() {
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
fn compiled_evaluates_arithmetic() {
    assert_eq!(evaluate("(+ 7 (* 6 5))").to_string(), "37");
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
        .unwrap();

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

    assert!(Runtime::new()
        .eval_compiled_source("(defconstant +answer+ 42) (setq +answer+ 7)")
        .is_err());
    assert!(Runtime::new()
        .eval_compiled_source("(defconstant +answer+ 42) (setf (symbol-value '+answer+) 7)")
        .is_err());
    assert!(Runtime::new()
        .eval_compiled_source("(defconstant +answer+ 42) (psetq +answer+ 7)")
        .is_err());
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
        .unwrap();

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
        .unwrap_err();
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
        .unwrap_err();
    assert!(matches!(
        error,
        RuntimeError::InvalidForm { message, .. } if message == "etypecase fell through"
    ));
}

#[test]
fn compiled_rejects_invalid_with_stream_binding_forms() {
    for (source, message) in [
        (
            "(with-open-file 1 42)",
            "with-open-file binding must be a list",
        ),
        (
            "(with-open-file (1 \"file\") 42)",
            "with-open-file stream variable must be a symbol",
        ),
        (
            "(with-output-to-string 1 42)",
            "with-output-to-string binding must be a list",
        ),
        (
            "(with-output-to-string (1) 42)",
            "with-output-to-string stream variable must be a symbol",
        ),
        (
            "(with-input-from-string 1 42)",
            "with-input-from-string binding must be a list",
        ),
        (
            "(with-input-from-string (1 \"abc\") 42)",
            "with-input-from-string stream variable must be a symbol",
        ),
    ] {
        let error = Runtime::new().eval_compiled_source(source).unwrap_err();
        assert!(matches!(
            error,
            RuntimeError::InvalidForm {
                message: ref actual_message,
                ..
            } if actual_message == message
        ));
    }
}

#[test]
fn compiled_special_variables_are_dynamically_bound_and_accessible_by_symbol_primitives() {
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
fn compiled_progv_temporarily_binds_symbols_and_restores_them() {
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
fn compiled_evaluates_with_simple_restart_and_invoke_restart() {
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
fn compiled_evaluates_restart_case_and_passes_restart_arguments() {
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
fn compiled_evaluates_restart_bind_invokes_function_and_propagates() {
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
fn compiled_evaluates_restart_introspection_and_object_invocation() {
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
fn compiled_evaluates_condition_restart_associations() {
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
fn compiled_evaluates_parallel_assignments_and_multiple_value_setq() {
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
fn compiled_evaluates_nested_quasiquote_vector_and_dotted_tail_splicing() {
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
fn compiled_evaluates_quasiquote_dotted_tail_as_proper_list() {
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
        .unwrap();

    assert_eq!(
        values[1].to_string(),
        "((1 NIL) (1 (2 3)) (7 (8 9)) (4 (5 6)) (11 (2 3)) NIL)"
    );
}

#[test]
fn compiled_declarations_are_accepted_in_function_bodies() {
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
fn compiled_optional_parameters_use_defaults_and_supplied_p() {
    let values = Runtime::new()
        .eval_compiled_source(
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
        .unwrap();

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
        .unwrap();

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
        .unwrap();

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
        .unwrap();

    assert_eq!(values[1].to_string(), "3");
}

#[test]
fn compiled_keyword_parameters_reject_unknown_and_malformed_arguments() {
    let unknown = Runtime::new()
        .eval_compiled_source("(defun read-value (&key value) value) (read-value :ignored 2)")
        .unwrap_err();
    assert!(matches!(
        unknown,
        RuntimeError::InvalidForm { message, .. } if message.contains("unknown keyword")
    ));

    let malformed = Runtime::new()
        .eval_compiled_source("(defun read-value (&key value) value) (read-value 'value 2)")
        .unwrap_err();
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
        .unwrap();

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
        .unwrap();
    assert_eq!(values[1].to_string(), "T");
}

#[test]
fn compiled_optional_parameters_report_missing_and_extra_arguments() {
    let missing = Runtime::new()
        .eval_compiled_source(
            "(defun bounded (required &optional optional) optional)
             (bounded)",
        )
        .unwrap_err();
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
        .unwrap_err();
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
        let error = Runtime::new().eval_compiled_source(source).unwrap_err();

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
        .unwrap();

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
        .unwrap_err();

    assert!(matches!(error, RuntimeError::InvalidForm { .. }));
}

#[test]
fn compiled_reports_compile_errors() {
    let error = Runtime::new()
        .eval_compiled_source("(if t 1 2 3)")
        .unwrap_err();

    assert!(matches!(error, RuntimeError::Compile(_)));
}

#[test]
fn compiled_evaluates_forms_and_maps_functions_over_lists() {
    assert_eq!(evaluate("(eval '(+ 2 3))").to_string(), "5");
    assert_eq!(
        evaluate("(let ((form '(+ 2 3))) (eval form))").to_string(),
        "5"
    );
    assert_eq!(evaluate("(funcall #'eval '(+ 2 3))").to_string(), "5");
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
fn compiled_evaluates_map_over_sequence_types() {
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
fn compiled_evaluates_reduce_over_sequences() {
    assert_eq!(
        evaluate("(reduce #'+ '(1 2 3 4))").to_string(),
        "10"
    );
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
fn compiled_evaluates_sequence_searches() {
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
fn compiled_evaluates_sequence_search_and_mismatch() {
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
        evaluate("(search \"ab\" \"xxABab\" :test #'char-equal :from-end t)")
            .to_string(),
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
fn compiled_evaluates_sequence_sort_and_stable_sort() {
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
fn compiled_evaluates_sequence_merge() {
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
fn compiled_evaluates_sequence_quantifiers() {
    assert_eq!(evaluate("(every #'numberp '(1 2))").to_string(), "T");
    assert_eq!(
        evaluate("(every #'= '(1 2) #(1 2))").to_string(),
        "T"
    );
    assert_eq!(
        evaluate("(some #'identity '(nil 2 4))").to_string(),
        "2"
    );
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
fn compiled_evaluates_list_membership_and_association_searches() {
    assert_eq!(evaluate("(member 2 '(1 2 3))").to_string(), "(2 3)");
    assert_eq!(
        evaluate("(member 2 '((1) (2) (3)) :key #'car)").to_string(),
        "((2) (3))"
    );
    assert_eq!(
        evaluate("(member-if-not #'evenp '(2 4 5 6))").to_string(),
        "(5 6)"
    );
    assert_eq!(
        evaluate("(adjoin 4 '(1 2 3))").to_string(),
        "(4 1 2 3)"
    );
    assert_eq!(
        evaluate("(assoc 'b '((a . 1) (b . 2)))").to_string(),
        "(B . 2)"
    );
    assert_eq!(
        evaluate("(assoc-if (lambda (key) (eq key 'b)) '((a . 1) (b . 2)))").to_string(),
        "(B . 2)"
    );
    assert_eq!(
        evaluate("(rassoc-if #'evenp '((a . 1) (b . 2)))").to_string(),
        "(B . 2)"
    );
    assert_eq!(
        evaluate("(funcall #'member 2 '(1 2 3))").to_string(),
        "(2 3)"
    );
}

#[test]
fn compiled_evaluates_sequence_removals() {
    assert_eq!(evaluate("(remove 2 '(1 2 2 3))").to_string(), "(1 3)");
    assert_eq!(
        evaluate("(remove 2 '(1 2 3 2) :from-end t :count 1)").to_string(),
        "(1 2 3)"
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
        evaluate("(remove-duplicates '(1 2 1 3 2) :from-end t)").to_string(),
        "(1 3 2)"
    );
    assert_eq!(
        evaluate("(delete-if #'evenp '(1 2 4 3))").to_string(),
        "(1 3)"
    );
    assert_eq!(
        evaluate("(funcall #'remove 2 '(1 2 3))").to_string(),
        "(1 3)"
    );
}

#[test]
fn compiled_evaluates_sequence_substitutions() {
    assert_eq!(
        evaluate("(substitute 9 2 '(1 2 2 3))").to_string(),
        "(1 9 9 3)"
    );
    assert_eq!(
        evaluate("(substitute 9 2 '(1 2 2 3) :from-end t :count 1)").to_string(),
        "(1 2 9 3)"
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
        evaluate("(nsubstitute 8 2 '(1 2 3))").to_string(),
        "(1 8 3)"
    );
    assert_eq!(
        evaluate("(funcall #'substitute 9 2 '(1 2 3))").to_string(),
        "(1 9 3)"
    );
}

#[test]
fn compiled_evaluates_list_set_operations() {
    assert_eq!(
        evaluate("(union '(1 2 2) '(2 3 3))").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate("(intersection '(1 2 2 3) '(2 3 4))").to_string(),
        "(2 3)"
    );
    assert_eq!(
        evaluate("(set-difference '(1 2 2 3) '(2))").to_string(),
        "(1 3)"
    );
    assert_eq!(
        evaluate("(set-exclusive-or '(1 2 2 3) '(2 4))").to_string(),
        "(1 3 4)"
    );
    assert_eq!(
        evaluate("(subsetp '(1 2) '(3 2 1 4))").to_string(),
        "T"
    );
    assert_eq!(
        evaluate("(union '(1 2) '(2 3) :test #'=)").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate("(union '((1 a) (2 b)) '((1 c) (3 d)) :key #'car)").to_string(),
        "((1 A) (2 B) (3 D))"
    );
    assert_eq!(
        evaluate("(nunion '(1 2) '(2 3))").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate("(funcall #'union '(1) '(2))").to_string(),
        "(1 2)"
    );
}

#[test]
fn compiled_evaluates_list_construction_and_partitioning() {
    assert_eq!(
        evaluate("(list* 1 2 '(3 4))").to_string(),
        "(1 2 3 4)"
    );
    assert_eq!(evaluate("(list* 1 2 3)").to_string(), "(1 2 . 3)");
    assert_eq!(
        evaluate("(make-list 3 :initial-element 'x)").to_string(),
        "(X X X)"
    );
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
        evaluate("(multiple-value-list (get-properties '(:a 1 :b 2) '(:b :a)))")
            .to_string(),
        "(:A 1 (:A 1 :B 2))"
    );
    assert_eq!(
        evaluate("(multiple-value-list (get-properties '(:a 1) '(:z)))").to_string(),
        "(NIL NIL NIL)"
    );
    assert_eq!(evaluate("(last '(1 2 3) 2)").to_string(), "(2 3)");
    assert_eq!(evaluate("(butlast '(1 2 3))").to_string(), "(1 2)");
    assert_eq!(evaluate("(nreverse '(1 2 3))").to_string(), "(3 2 1)");
    assert_eq!(
        evaluate("(nconc '(1 2) '(3 4))").to_string(),
        "(1 2 3 4)"
    );
    assert_eq!(
        evaluate("(revappend '(1 2) '(3 4))").to_string(),
        "(2 1 3 4)"
    );
    assert_eq!(
        evaluate("(funcall #'list* 1 '(2 3))").to_string(),
        "(1 2 3)"
    );
    assert_eq!(evaluate("(funcall #'nthcdr 1 '(4 5))").to_string(), "(5)");
}

#[test]
fn compiled_evaluates_sequence_fill_replace_and_concatenate() {
    assert_eq!(
        evaluate("(fill 0 '(1 2 3 4) :start 1 :end 3)").to_string(),
        "(1 0 0 4)"
    );
    assert_eq!(
        evaluate("(fill #\\x \"abcd\" :start 1)").to_string(),
        "\"axxx\""
    );
    assert_eq!(
        evaluate("(fill 9 #(1 2 3) :end 2)").to_string(),
        "#(9 9 3)"
    );
    assert_eq!(
        evaluate(
            "(replace '(9 9 9) '(1 2 3 4) :start1 1 :end1 3 :start2 0 :end2 2)"
        )
        .to_string(),
        "(9 1 2)"
    );
    assert_eq!(
        evaluate("(replace \"xxxx\" \"abcd\" :start1 1 :end1 3 :start2 0 :end2 2)")
            .to_string(),
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
fn compiled_evaluates_map_into_over_sequences() {
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
        .unwrap_err();
    assert!(matches!(
        error,
        RuntimeError::UnboundVariable { name, .. } if name == "MISSING-FUNCTION"
    ));
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
            r#"(list (load "{}") *NCL-LOAD-VALUE* (NCL-LOAD-TARGET 1))"#,
            path
        ))
        .to_string(),
        "(T 41 42)"
    );
}

#[test]
fn compiled_evaluates_load_time_value() {
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
fn compiled_evaluates_nth_value() {
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
fn compiled_evaluates_function_and_macro_introspection() {
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
fn compiled_evaluates_function_namespace_mutation() {
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
fn compiled_evaluates_numeric_predicates_and_extrema() {
    assert_eq!(
        evaluate("(list (zerop 0) (plusp 1) (minusp -1) (evenp 4) (oddp 3) (min 3 1 2) (max 3 1 2) (abs -5))").to_string(),
        "(T T T T T 1 3 5)"
    );
}

#[test]
fn compiled_evaluates_common_lisp_integer_arithmetic_and_bit_operations() {
    assert_eq!(
        evaluate(
            "(list (mod -7 3) (mod 7 -3) (rem -7 3) (rem 7 -3)
                    (ash 3 2) (ash -8 -2)
                    (logand 7 3) (logior 4 1) (logxor 7 3) (lognot 0)
                    (logtest 6 2) (logtest 4 2)
                    (logcount 13) (logcount -8)
                    (integer-length 8) (integer-length -8)
                    (logand) (logior) (logxor))",
        )
        .to_string(),
        "(2 -2 -1 1 12 -2 3 5 4 -1 T NIL 3 3 4 3 -1 0 0)"
    );
}

#[test]
fn compiled_evaluates_common_lisp_quotients_gcd_and_rational_parts() {
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
fn compiled_evaluates_common_lisp_expt_across_numeric_types() {
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
fn compiled_evaluates_common_lisp_sqrt_across_exact_and_float_numbers() {
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
fn compiled_evaluates_common_lisp_signum_and_rationalize() {
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
fn compiled_evaluates_common_lisp_float_and_rational_conversion() {
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
fn compiled_evaluates_basic_format_directives() {
    assert_eq!(
        evaluate(r#"(format nil "~A/~S" "text" "text")"#).to_string(),
        r#""text/\"text\"""#,
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
    assert_eq!(evaluate(r#"(format t "")"#).to_string(), "NIL");
    assert_eq!(
        evaluate(r#"(format nil "~?/~*" "~A ~D" '(foo 7) 99 100)"#).to_string(),
        r#""FOO 7/""#,
    );
}

#[test]
fn compiled_evaluates_plural_format_directive() {
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
fn compiled_evaluates_dollar_float_format_directive() {
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
fn compiled_evaluates_general_float_format_directive() {
    assert_eq!(
        evaluate(r#"(format nil "~G|~,3G|~10,3G|~10,3G|~10,3,0G|~10,3,1G|~10,3,2G|~@G" 12.3456 1.25 12.3456 0.0123456 12.3456 12.3456 12.3456 1.25)"#)
            .to_string(),
        r#""12.3456    |1.25    |  12.3    |  1.235e-2|    12.3  |   12.3   |  12.3    |+1.25    ""#,
    );
}

#[test]
fn compiled_evaluates_format_tabulation_modifiers() {
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
fn compiled_evaluates_format_write_directive() {
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
fn compiled_evaluates_fixed_float_format_directive() {
    assert_eq!(
        evaluate(r#"(format nil "~F|~,2F|~10,2F|~@F|~4,2,,'*F" 1.25 1.25 1.25 1.25 123.4)"#)
            .to_string(),
        r#""1.25|1.25|      1.25|+1.25|****""#,
    );
    assert_eq!(evaluate(r#"(format nil "~,0F" 1.25)"#).to_string(), r#""1.""#);
    assert_eq!(evaluate(r#"(format nil "~F" 3)"#).to_string(), r#""3.0""#);
}

#[test]
fn compiled_evaluates_exponential_float_format_directive() {
    assert_eq!(
        evaluate(r#"(format nil "~E|~,2E|~10,2E|~@E" 1.25 1.25 1.25 1.25)"#).to_string(),
        r#""1.25E+0|1.25E+0|   1.25E+0|+1.25E+0""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~,2,3E|~,2,,0E|~,2,,-1E" 0.0125 637.5 637.5)"#)
            .to_string(),
        r#""1.25E-002|0.64E+3|0.06E+4""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "~6,2,,,'*E" 123.4)"#).to_string(),
        r#""******""#,
    );
}

#[test]
fn compiled_evaluates_parameterized_format_directives() {
    assert_eq!(
        evaluate(r#"(format nil "~10A|~10@A|~10,'0D|~:D|~@D" "x" "y" 42 1234567 8)"#)
            .to_string(),
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
        evaluate(r#"(format nil "~:C/~@C" #\Newline #\Space)"#).to_string(),
        r#""Newline/#\\Space""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "a~2%b")"#).to_string(),
        r#""a\n\nb""#,
    );
}

#[test]
fn compiled_evaluates_format_iteration_directives() {
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
fn compiled_evaluates_format_recursive_processing_directive() {
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
        evaluate(
            r#"(format nil "~:{ ~@?~:^ ...~} " '(("a") ("b")))"#,
        )
        .to_string(),
        r#"" a ... b ""#,
    );
}

#[test]
fn compiled_evaluates_format_justification_directive() {
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
fn compiled_evaluates_format_conditional_newline_directive() {
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
fn compiled_evaluates_format_case_conversion_directive() {
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
fn compiled_evaluates_format_escape_upward_directive() {
    assert_eq!(
        evaluate(r#"(format nil "~{~A~^, ~}" '(one two three))"#).to_string(),
        r#""ONE, TWO, THREE""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "done~^ignored")"#).to_string(),
        r#""done""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "a~1,1^b")"#).to_string(),
        r#""a""#,
    );
    assert_eq!(
        evaluate(r#"(format nil "a~1,2^b")"#).to_string(),
        r#""ab""#,
    );
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
fn compiled_evaluates_format_choice_directives() {
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
fn compiled_evaluates_format_choice_parameters() {
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
fn compiled_evaluates_write_to_string() {
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
fn compiled_evaluates_write_escape_options() {
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
fn compiled_evaluates_print_variants_to_string_stream() {
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
fn compiled_evaluates_write_to_stream() {
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
fn compiled_evaluates_read_from_string() {
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
fn compiled_evaluates_read_from_string_options() {
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
fn compiled_evaluates_read_from_string_stream() {
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
fn compiled_evaluates_read_whitespace_consumption() {
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
fn compiled_evaluates_character_stream_options_and_eof() {
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
fn compiled_evaluates_sequence_operations_and_type_predicates() {
    assert_eq!(
        evaluate("(list (first '(a b c)) (rest '(a b c)) (nth 1 '(a b c)) (elt \"abc\" 1) (subseq '(a b c d) 1 3) (subseq \"abcd\" 1 3) (member 'b '(a b c)) (assoc 'b '((a 1) (b 2))) (getf '(:a 1 :b 2) :b) (length \"abc\"))").to_string(),
        "(A (B C) B #\\b (B C) \"bc\" (B C) (B 2) 2 3)"
    );
    assert_eq!(
        evaluate("(list (typep 1 'integer) (typep \"abc\" 'sequence) (characterp #\\a) (keywordp :x) (vectorp #(1 2)) (endp nil) (endp '(1)))").to_string(),
        "(T T T T T T NIL)"
    );
}

#[test]
fn compiled_evaluates_compound_type_designators() {
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
        "(T T T T T T T T T T T T T NIL 7 #(1 2))"
    );
}

#[test]
fn compiled_evaluates_subtypep() {
    let values = Runtime::new()
        .eval_compiled_source(
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
                   (multiple-value-list (subtypep 'string 'sequence))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "((T T) (T T) (NIL T) (T T) (T T) (T T))"
    );
}

#[test]
fn compiled_evaluates_sequence_construction_and_coercion() {
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
}

#[test]
fn compiled_evaluates_parse_integer() {
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
fn compiled_evaluates_basic_clos_instances_and_accessors() {
    let values = Runtime::new()
        .eval_compiled_source(
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
fn compiled_evaluates_clos_with_slots_and_accessors() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_compiled_source(
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
        .eval_compiled_source("(with-accessors (x) object x)")
        .is_err());
}

#[test]
fn compiled_evaluates_clos_slot_initialization_options() {
    let values = Runtime::new()
        .eval_compiled_source(
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
fn compiled_evaluates_clos_class_allocated_slots() {
    let values = Runtime::new()
        .eval_compiled_source(
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
fn compiled_evaluates_clos_default_initargs() {
    let values = Runtime::new()
        .eval_compiled_source(
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
fn compiled_evaluates_clos_setf_and_generic_methods() {
    let values = Runtime::new()
        .eval_compiled_source(
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
fn compiled_evaluates_clos_methods_with_ordinary_lambda_lists() {
    let values = Runtime::new()
        .eval_compiled_source(
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
fn compiled_evaluates_clos_inheritance_and_specialization() {
    let values = Runtime::new()
        .eval_compiled_source(
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
fn compiled_evaluates_clos_unbound_slots() {
    let values = Runtime::new()
        .eval_compiled_source(
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
fn compiled_evaluates_clos_method_combination() {
    let values = Runtime::new()
        .eval_compiled_source(
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
fn compiled_evaluates_the_with_type_designators() {
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
fn compiled_evaluates_locally_and_eval_when() {
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
fn compiled_evaluates_defstruct_constructors_accessors_and_copies() {
    let values = Runtime::new()
        .eval_compiled_source(
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
fn compiled_evaluates_defstruct_name_and_options() {
    let values = Runtime::new()
        .eval_compiled_source(
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
fn compiled_evaluates_defstruct_read_only_slots() {
    let values = Runtime::new()
        .eval_compiled_source(
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
        .eval_compiled_source(
            r#"(progn
                 (defstruct immutable (id 0 t))
                 (let ((record (make-immutable :id 1)))
                   (setf (immutable-id record) 2)))"#,
        )
        .unwrap_err();
    assert!(matches!(
        error,
        RuntimeError::InvalidForm { message, .. }
            if message == "cannot SETF a read-only structure slot"
    ));
}

#[test]
fn compiled_evaluates_defstruct_included_slots_and_type_hierarchy() {
    let values = Runtime::new()
        .eval_compiled_source(
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
fn compiled_evaluates_defstruct_boa_constructors() {
    let values = Runtime::new()
        .eval_compiled_source(
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
fn compiled_evaluates_arrays_and_multidimensional_setf() {
    assert_eq!(
        evaluate(
            "(let ((array (make-array '(2 2) :initial-element 0))
                   (vector (make-array 3 :initial-element 5)))
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
}

#[test]
fn compiled_evaluates_hash_tables_and_gethash_setf() {
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
fn compiled_evaluates_handler_case_and_handler_bind() {
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
fn compiled_evaluates_error_through_condition_handlers() {
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
fn compiled_evaluates_signal_warn_cerror_and_dynamic_handlers() {
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
fn compiled_evaluates_condition_format_arguments() {
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
fn compiled_evaluates_catch_and_throw() {
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
fn compiled_evaluates_character_and_string_operations() {
    assert_eq!(
        evaluate(
            "(list (string #\\a) (string 'hello) (make-string 3 #\\x) (char \"abc\" 1) (char-code #\\A) (code-char 98) (char= #\\a #\\a) (char-equal #\\A #\\a) (char< #\\a #\\c) (string= \"abc\" \"abc\") (string-equal \"AbC\" \"aBc\") (string< \"abc\" \"abd\") (string-upcase \"Abc\") (string-downcase \"AbC\"))"
        )
        .to_string(),
        "(\"a\" \"HELLO\" \"xxx\" #\\b 65 #\\b T T T T T 2 \"ABC\" \"abc\")"
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
fn compiled_evaluates_extended_character_operations() {
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
fn compiled_evaluates_setf_places() {
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
               (defparameter *compiled-setf-symbol-value-target* 1)
               (list
                 (setf (symbol-value '*compiled-setf-symbol-value-target*) 7)
                 (symbol-value '*compiled-setf-symbol-value-target*)))",
        )
        .to_string(),
        "(7 7)"
    );
}

#[test]
fn compiled_evaluates_push_pop_and_psetf() {
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
        "(2 1 2)"
    );
}

#[test]
fn compiled_evaluates_pushnew() {
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
fn compiled_evaluates_simple_defsetf() {
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *compiled-defsetf-cell* 1)
               (defun compiled-defsetf-reader () *compiled-defsetf-cell*)
               (defun compiled-defsetf-writer (value) (setq *compiled-defsetf-cell* value))
               (defsetf compiled-defsetf-reader compiled-defsetf-writer)
               (setf (compiled-defsetf-reader) 42)
               (compiled-defsetf-reader))",
        )
        .to_string(),
        "42"
    );
}

#[test]
fn compiled_evaluates_defsetf_passes_place_arguments_before_value() {
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *compiled-defsetf-arguments* nil)
               (defun compiled-defsetf-argument-reader (first second) nil)
               (defun compiled-defsetf-argument-writer (&rest arguments)
                 (setq *compiled-defsetf-arguments* arguments))
               (defsetf compiled-defsetf-argument-reader compiled-defsetf-argument-writer)
               (setf (compiled-defsetf-argument-reader :first :second) :new)
               *compiled-defsetf-arguments*)",
        )
        .to_string(),
        "(:FIRST :SECOND :NEW)"
    );
}

#[test]
fn compiled_evaluates_define_setf_expander_and_get_setf_expansion() {
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *compiled-custom-setf-cell* 1)
               (define-setf-expander compiled-custom-setf-place ()
                 (values nil nil '(new-value)
                         '(progn
                            (setq *compiled-custom-setf-cell* new-value)
                            new-value)
                         '*compiled-custom-setf-cell*))
               (setf (compiled-custom-setf-place) 42)
               (multiple-value-bind (temporaries value-forms stores store-form access-form)
                   (get-setf-expansion '(compiled-custom-setf-place))
                 (list *compiled-custom-setf-cell*
                       (length temporaries)
                       (length value-forms)
                       (length stores)
                       (car stores)
                       store-form
                       access-form)))",
        )
        .to_string(),
        "(42 0 0 1 NEW-VALUE (PROGN (SETQ *COMPILED-CUSTOM-SETF-CELL* NEW-VALUE) NEW-VALUE) *COMPILED-CUSTOM-SETF-CELL*)"
    );
}

#[test]
fn compiled_evaluates_define_modify_macro_on_generalized_place() {
    assert_eq!(
        evaluate(
            "(progn
               (define-modify-macro compiled-add-to-place (&optional (delta 1)) +)
               (let ((cell (list 10)))
                 (list (compiled-add-to-place (car cell) 2)
                       (compiled-add-to-place (car cell))
                       cell)))",
        )
        .to_string(),
        "(12 13 (13))"
    );
    assert_eq!(
        evaluate(
            "(progn
               (define-modify-macro compiled-add-to-nested-place (&optional (delta 1)) +)
               (let ((cells (list (list 10))))
                 (list (compiled-add-to-nested-place (car (nth 0 cells)) 2)
                       cells)))",
        )
        .to_string(),
        "(12 ((12)))"
    );
}

#[test]
fn compiled_evaluates_define_symbol_macro_and_generalized_places() {
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *compiled-symbol-macro-cell* (list 1))
               (define-symbol-macro *compiled-symbol-macro-item*
                 (car *compiled-symbol-macro-cell*))
               (list *compiled-symbol-macro-item*
                     (progn
                       (setq *compiled-symbol-macro-item* 7)
                       *compiled-symbol-macro-item*)
                     *compiled-symbol-macro-cell*))",
        )
        .to_string(),
        "(1 7 (7))"
    );
}

#[test]
fn compiled_evaluates_symbol_properties_and_setf_get() {
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
fn compiled_evaluates_incf_and_decf_symbol_places() {
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
fn compiled_evaluates_incf_and_decf_generalized_places() {
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
fn compiled_evaluates_rotatef_and_shiftf() {
    assert_eq!(
        evaluate(
            "(let ((a 1) (b 2) (c 3))
               (list (rotatef a b c) a b c))",
        )
        .to_string(),
        "(NIL 3 1 2)"
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list 1 2)))
               (list (shiftf (car xs) (car (cdr xs)) 9) xs))",
        )
        .to_string(),
        "(1 (2 9))"
    );
}

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
fn compiled_evaluates_do_and_do_star_with_parallel_and_sequential_bindings() {
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
fn compiled_evaluates_do_with_implicit_block_and_tagbody() {
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
fn compiled_evaluates_prog_and_prog_star_with_parallel_and_sequential_bindings() {
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
fn compiled_evaluates_prog_with_implicit_block_and_tagbody() {
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
fn compiled_evaluates_return_as_an_implicit_nil_block_exit() {
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
fn compiled_evaluates_destructuring_bind_with_nested_and_dotted_patterns() {
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
fn compiled_evaluates_destructuring_bind_lambda_list_parameters() {
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
            "(destructuring-bind (first &rest rest &aux (count (length rest)))
               (list 3 4 5)
               (list first rest count))",
        )
        .to_string(),
        "(3 (4 5) 2)",
    );
}

#[test]
fn compiled_destructuring_bind_binds_environment_parameter() {
    assert_eq!(
        evaluate(
            "(progn
               (macrolet ((local () '(quote local)))
                 (destructuring-bind (&environment environment) nil
                   (list
                     (macroexpand-1 '(local) environment)
                     (macroexpand '(local) environment)))))",
        )
        .to_string(),
        "((QUOTE LOCAL) (QUOTE LOCAL))"
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
        .unwrap();

    assert_eq!(values[3].to_string(), "42");
    assert_eq!(runtime.current_package(), "COMPILED-DEMO");

    let values = runtime
        .eval_compiled_source("(in-package :ncl-user) compiled-demo:answer")
        .unwrap();
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
        .unwrap_err();

    assert!(matches!(error, RuntimeError::Package { .. }));
    assert_eq!(
        runtime
            .eval_compiled_source("compiled-hidden::secret")
            .unwrap()
            .pop()
            .unwrap()
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
        .unwrap();

    assert_eq!(values.last().unwrap().to_string(), "(41 2)");
    assert_eq!(
        runtime
            .eval_compiled_source("(define answer 99) (list answer (plus-one 1))")
            .unwrap()
            .last()
            .unwrap()
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
        .unwrap();

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
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
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
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
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
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        r#"((42 T) (42 T) 7 T NIL (NIL NIL))"#
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
        .unwrap();

    assert_eq!(values.last().unwrap().to_string(), r#"(T T T 41 41)"#);
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
        .unwrap();

    assert_eq!(values.last().unwrap().to_string(), r#"(42 42 42 T)"#);
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
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
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
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
        r#"(42 "local owner documentation" NIL)"#
    );
}

#[test]
fn compiled_defpackage_size_option_is_accepted_and_validated() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_compiled_source(
            r#"(defpackage :package-size-compiled
                 (:use :common-lisp)
                 (:size 0))
               (package-name (find-package :package-size-compiled))"#,
        )
        .unwrap();

    assert_eq!(values.last().unwrap().to_string(), "\"PACKAGE-SIZE-COMPILED\"");

    let error = runtime
        .eval_compiled_source("(defpackage :package-size-invalid-compiled (:size -1))")
        .unwrap_err();
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
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
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
        .unwrap();

    assert_eq!(
        values.last().unwrap().to_string(),
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
        pathname = pathname
    );

    assert_eq!(evaluate(&source).to_string(), "T");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello");
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
fn compiled_file_io_stream_reads_writes_and_appends() {
    let path = std::env::temp_dir().join(format!(
        "ncl-file-io-stream-compiled-{}",
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
fn compiled_tagbody_and_go_with_forward_and_backward_jumps() {
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
fn compiled_unmatched_go_is_not_swallowed_by_ignore_errors() {
    let error = Runtime::new()
        .eval_compiled_source("(ignore-errors (go missing))")
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
fn compiled_tagbody_returns_nil_and_does_not_evaluate_labels() {
    assert_eq!(
        evaluate("(list (tagbody start done) 42)").to_string(),
        "(NIL 42)"
    );
}

#[test]
fn compiled_supports_integer_and_keyword_tags() {
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
fn compiled_captures_an_active_tagbody_target_in_a_closure() {
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
fn compiled_rejects_invalid_go_shapes_and_tags() {
    for source in ["(go)", "(go missing extra)", "(go 1.5)"] {
        assert!(
            Runtime::new().eval_compiled_source(source).is_err(),
            "{source}"
        );
    }
    assert!(Runtime::new()
        .eval_compiled_source("(tagbody start start)")
        .is_err());
}
