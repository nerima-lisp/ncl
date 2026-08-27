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
fn compiled_propagates_unhandled_conditions_from_handler_scopes() {
    let cases = [
        "(handler-case (error \"boom\") (type-error (condition) condition))",
        "(handler-bind ((type-error (lambda (condition) condition))) (error \"boom\"))",
    ];

    for source in cases {
        let error = Runtime::new().eval_compiled_source(source).must_fail();
        assert!(
            matches!(error, RuntimeError::Signaled(_)),
            "{source}: {error:?}"
        );
    }
}

#[test]
fn compiled_propagates_unmatched_control_transfers_from_scopes() {
    for source in [
        "(catch 'tag (throw 'other 9))",
        "(with-simple-restart (abort \"abort\") (invoke-restart 'other 9))",
        "(restart-bind ((abort (lambda () 42))) (invoke-restart 'other))",
    ] {
        assert!(
            Runtime::new().eval_compiled_source(source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn compiled_rejects_malformed_condition_primitives_from_table_cases() {
    for source in [
        "(error)",
        "(signal)",
        "(warn)",
        "(cerror)",
        "(cerror \"continue\")",
        "(signal (make-condition 'simple-condition) \"extra\")",
        "(warn (make-condition 'simple-warning) \"extra\")",
        "(cerror \"continue\" (make-condition 'simple-error) \"extra\")",
    ] {
        assert!(
            Runtime::new().eval_compiled_source(source).is_err(),
            "{source}"
        );
    }
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
            r"(let ((condition (make-condition 'user-condition)))
                   (typep condition 'condition))",
        )
        .to_string(),
        "T"
    );
}

#[test]
fn compiled_rejects_malformed_with_condition_restarts_arguments() {
    let cases = [
        "(with-condition-restarts 1 nil 1)",
        "(with-condition-restarts (make-condition 'error) 1 1)",
        "(with-condition-restarts (make-condition 'error) (list 1) 1)",
    ];

    for source in cases {
        assert!(
            Runtime::new().eval_compiled_source(source).is_err(),
            "{source}"
        );
    }
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
fn compiled_rejects_malformed_character_operations_at_their_boundaries() {
    for source in support::MALFORMED_CHARACTER_FORMS {
        assert!(
            Runtime::new().eval_compiled_source(source).is_err(),
            "{source}"
        );
    }
}

use super::*;
