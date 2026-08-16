use super::evaluate;

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
            "(list (string #\\a)
                   (string 'hello)
                   (make-string 3 :initial-element #\\x :element-type 'character)
                   (make-string 2 :element-type 'base-char)
                   (char \"abc\" 1)
                   (char-code #\\A)
                   (code-char 98)
                   (char= #\\a #\\a)
                   (char-equal #\\A #\\a)
                   (char< #\\a #\\c)
                   (string= \"abc\" \"abc\")
                   (string-equal \"AbC\" \"aBc\")
                   (string< \"abc\" \"abd\")
                   (string-upcase \"Abc\")
                   (string-downcase \"AbC\"))"
        )
        .to_string(),
        "(\"a\" \"HELLO\" \"xxx\" \"  \" #\\b 65 #\\b T T T T T 2 \"ABC\" \"abc\")"
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
