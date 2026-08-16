use super::{Runtime, RuntimeError, evaluate};

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
    assert_eq!(
        evaluate("(dolist (item nil item) (setq item 1))").to_string(),
        "NIL"
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
fn compiled_rejects_malformed_do_forms() {
    assert!(matches!(
        Runtime::new().eval_compiled_source("(do 1 ((= 1 1)) 42)"),
        Err(RuntimeError::InvalidForm { .. }),
    ));
    assert!(matches!(
        Runtime::new().eval_compiled_source("(do ((i 0)) 1 42)"),
        Err(RuntimeError::InvalidForm { .. }),
    ));
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
fn compiled_rejects_malformed_prog_forms() {
    assert!(matches!(
        Runtime::new().eval_compiled_source("(prog 1 42)"),
        Err(RuntimeError::InvalidForm { .. }),
    ));
    assert!(matches!(
        Runtime::new().eval_compiled_source("(prog ((1 0)) 42)"),
        Err(RuntimeError::InvalidForm { .. }),
    ));
}

#[test]
fn compiled_rejects_let_and_local_function_forms_without_required_bindings() {
    for (source, function) in [
        ("(let)", "let"),
        ("(let*)", "let*"),
        ("(flet)", "flet"),
        ("(labels)", "labels"),
    ] {
        let error = Runtime::new().eval_compiled_source(source).unwrap_err();
        assert!(matches!(
            error,
            RuntimeError::Arity {
                expected,
                actual: 0,
                function: ref actual_function,
            } if actual_function == function && expected == "at least one"
        ));
    }
}

#[test]
fn compiled_rejects_when_and_unless_without_required_condition() {
    for (source, function) in [("(when)", "when"), ("(unless)", "unless")] {
        let error = Runtime::new().eval_compiled_source(source).unwrap_err();
        assert!(matches!(
            error,
            RuntimeError::Arity {
                expected,
                actual: 0,
                function: ref actual_function,
            } if actual_function == function && expected == "at least one"
        ));
    }
}

#[test]
fn compiled_rejects_non_list_let_and_local_function_bindings() {
    for (source, message) in [
        ("(let 1 42)", "let bindings must be a list"),
        ("(let (1) 42)", "let binding must be a list"),
        ("(flet 1 42)", "local function bindings must be a list"),
        ("(flet (1) 42)", "local function binding must be a list"),
        ("(labels 1 42)", "local function bindings must be a list"),
        ("(labels (1) 42)", "local function binding must be a list"),
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
fn compiled_rejects_non_list_handler_restart_and_eval_when_forms() {
    for (source, message) in [
        (
            "(handler-bind 1 42)",
            "handler-bind handler list must be a list",
        ),
        (
            "(restart-bind 1 42)",
            "restart-bind binding list must be a list",
        ),
        ("(eval-when 1 42)", "eval-when situations must be a list"),
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
fn compiled_rejects_invalid_multiple_value_variable_forms() {
    for (source, message) in [
        (
            "(multiple-value-bind 1 (values 1) 42)",
            "multiple-value-bind variables must be a list",
        ),
        (
            "(multiple-value-bind (1) (values 1) 42)",
            "multiple-value-bind variable must be a symbol",
        ),
        (
            "(multiple-value-setq 1 (values 1))",
            "multiple-value-setq variables must be a list",
        ),
        (
            "(multiple-value-setq (1) (values 1))",
            "multiple-value-setq variable must be a symbol",
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
fn compiled_rejects_invalid_handler_restart_and_conditional_clauses() {
    for (source, message) in [
        ("(handler-case 1 2)", "handler-case clause must be a list"),
        (
            "(handler-case 1 (error 2))",
            "handler-case variable list must be a list",
        ),
        (
            "(handler-case 1 (1 () 42))",
            "condition name must be a symbol",
        ),
        (
            "(handler-bind (1) 42)",
            "handler-bind clause must be a list",
        ),
        (
            "(handler-bind ((error)) 42)",
            "handler-bind clause needs a condition and function",
        ),
        (
            "(handler-bind ((1 (lambda (condition) condition))) 42)",
            "condition name must be a symbol",
        ),
        (
            "(restart-bind (1) 42)",
            "restart-bind clause must be a list",
        ),
        (
            "(restart-bind ((1 (lambda () 42))) 42)",
            "restart name must be a symbol",
        ),
        (
            "(with-simple-restart 1 42)",
            "with-simple-restart restart clause must be a list",
        ),
        (
            "(with-simple-restart (1 \"nope\") 42)",
            "restart name must be a symbol",
        ),
        ("(restart-case 1 2)", "restart-case clause must be a list"),
        ("(restart-case 1 (1 () 2))", "restart name must be a symbol"),
        ("(cond 1)", "cond clauses must be lists"),
        ("(case 1 2)", "case clauses must be lists"),
        ("(typecase 1 2)", "typecase clauses must be lists"),
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
fn compiled_rejects_invalid_block_names() {
    for source in ["(block 1 42)", "(return-from 1 42)"] {
        let error = Runtime::new().eval_compiled_source(source).unwrap_err();
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { ref message, .. }
                if message == "block name must be a symbol"
        ));
    }
}

#[test]
fn compiled_rejects_invalid_iteration_eval_when_and_stream_binding_forms() {
    for (source, message) in [
        ("(prog 1)", "prog bindings must be a list"),
        ("(prog ((1)) nil)", "prog binding name must be a symbol"),
        ("(do 1 (t))", "do bindings must be a list"),
        ("(do ((1)) (t))", "do binding name must be a symbol"),
        ("(dotimes 1 42)", "dotimes binding must be a list"),
        (
            "(dotimes (1 3) 42)",
            "dotimes binding name must be a symbol",
        ),
        ("(dolist 1 42)", "dolist binding must be a list"),
        (
            "(dolist (1 '(1)) 42)",
            "dolist binding name must be a symbol",
        ),
        ("(eval-when 1 42)", "eval-when situations must be a list"),
        (
            "(eval-when (1) 42)",
            "eval-when situations must contain symbols",
        ),
        (
            "(with-open-file (stream) 42)",
            "with-open-file binding needs a stream variable and pathname",
        ),
        (
            "(with-output-to-string () 42)",
            "with-output-to-string binding needs a stream variable and optional string place",
        ),
        (
            "(with-input-from-string (stream \"abc\" :start) 42)",
            "with-input-from-string options need keyword/value pairs",
        ),
        (
            "(with-input-from-string (stream \"abc\" 1 2) 42)",
            "with-input-from-string option must be a keyword",
        ),
        (
            "(with-input-from-string (stream \"abc\" :index x :index y) 42)",
            "with-input-from-string :index may appear only once",
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
fn compiled_rejects_prog1_and_prog2_without_required_forms() {
    let prog1 = Runtime::new().eval_compiled_source("(prog1)").unwrap_err();
    assert!(
        matches!(
            prog1,
            ncl_runtime::RuntimeError::Arity {
                ref function,
                ref expected,
                actual: 0,
            } if function == "prog1" && expected == "at least one"
        ),
        "got: {prog1:?}"
    );

    let prog2 = Runtime::new()
        .eval_compiled_source("(prog2 1)")
        .unwrap_err();
    assert!(
        matches!(
            prog2,
            ncl_runtime::RuntimeError::Arity {
                ref function,
                ref expected,
                actual: 1,
            } if function == "prog2" && expected == "at least two"
        ),
        "got: {prog2:?}"
    );
}

#[test]
fn compiled_rejects_invalid_dotimes_and_dolist_forms() {
    let invalid_binding = Runtime::new()
        .eval_compiled_source("(dotimes item)")
        .unwrap_err();
    assert!(
        matches!(
            invalid_binding,
            ncl_runtime::RuntimeError::InvalidForm { .. }
        ),
        "got: {invalid_binding:?}"
    );

    let invalid_count = Runtime::new()
        .eval_compiled_source("(dotimes (index nil) index)")
        .unwrap_err();
    assert!(
        matches!(
            invalid_count,
            ncl_runtime::RuntimeError::Type {
                ref expected, ..
            } if expected == "INTEGER"
        ),
        "got: {invalid_count:?}"
    );

    let invalid_list = Runtime::new()
        .eval_compiled_source("(dolist (item 42) item)")
        .unwrap_err();
    assert!(
        matches!(
            invalid_list,
            ncl_runtime::RuntimeError::Type {
                ref expected, ..
            } if expected == "LIST"
        ),
        "got: {invalid_list:?}"
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
    assert_eq!(
        evaluate(
            "(destructuring-bind ((&whole whole name &body forms))
               (list (list 'when '(print 1) '(print 2)))
               (list name forms whole))",
        )
        .to_string(),
        "(WHEN ((PRINT 1) (PRINT 2)) (WHEN (PRINT 1) (PRINT 2)))",
    );
    assert_eq!(
        evaluate(
            "(destructuring-bind ((first &optional (second (+ first 1) second-p))
                                  (&key (scale 2 scale-p) &allow-other-keys))
               (list (list 3) (list :scale 4 :ignored 9))
               (list first second second-p scale scale-p))",
        )
        .to_string(),
        "(3 4 NIL 4 T)",
    );
}

#[test]
fn compiled_rejects_invalid_destructuring_bind_parameter_names() {
    for (source, message) in [
        (
            "(destructuring-bind (&optional (value nil \"oops\")) nil value)",
            "destructuring supplied-p name must be a symbol",
        ),
        (
            "(destructuring-bind (&key (((quote :name) value))) nil value)",
            "destructuring keyword designator must be a symbol",
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
