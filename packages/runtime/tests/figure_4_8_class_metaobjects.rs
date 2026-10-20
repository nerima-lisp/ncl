#![allow(missing_docs)]

use ncl_runtime::Runtime;
use ncl_sys::Word;

#[test]
fn all_predefined_classes_are_findable_and_have_valid_metaclasses() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let classes = [
        "ARITHMETIC-ERROR",
        "ARRAY",
        "BIT-VECTOR",
        "BROADCAST-STREAM",
        "BUILT-IN-CLASS",
        "CELL-ERROR",
        "CHARACTER",
        "CLASS",
        "COMPLEX",
        "CONCATENATED-STREAM",
        "CONDITION",
        "CONS",
        "CONTROL-ERROR",
        "DIVISION-BY-ZERO",
        "ECHO-STREAM",
        "END-OF-FILE",
        "ERROR",
        "FILE-ERROR",
        "FILE-STREAM",
        "FLOAT",
        "FLOATING-POINT-INEXACT",
        "FLOATING-POINT-INVALID-OPERATION",
        "FLOATING-POINT-OVERFLOW",
        "FLOATING-POINT-UNDERFLOW",
        "FUNCTION",
        "GENERIC-FUNCTION",
        "HASH-TABLE",
        "INTEGER",
        "LIST",
        "LOGICAL-PATHNAME",
        "METHOD",
        "METHOD-COMBINATION",
        "NULL",
        "NUMBER",
        "PACKAGE",
        "PACKAGE-ERROR",
        "PARSE-ERROR",
        "PATHNAME",
        "PRINT-NOT-READABLE",
        "PROGRAM-ERROR",
        "RANDOM-STATE",
        "RATIO",
        "RATIONAL",
        "READER-ERROR",
        "READTABLE",
        "REAL",
        "RESTART",
        "SEQUENCE",
        "SERIOUS-CONDITION",
        "SIMPLE-CONDITION",
        "SIMPLE-ERROR",
        "SIMPLE-TYPE-ERROR",
        "SIMPLE-WARNING",
        "STANDARD-CLASS",
        "STANDARD-GENERIC-FUNCTION",
        "STANDARD-METHOD",
        "STANDARD-OBJECT",
        "STORAGE-CONDITION",
        "STREAM",
        "STREAM-ERROR",
        "STRING",
        "STRING-STREAM",
        "STRUCTURE-CLASS",
        "STRUCTURE-OBJECT",
        "STYLE-WARNING",
        "SYMBOL",
        "SYNONYM-STREAM",
        "T",
        "TWO-WAY-STREAM",
        "TYPE-ERROR",
        "UNBOUND-SLOT",
        "UNBOUND-VARIABLE",
        "UNDEFINED-FUNCTION",
        "VECTOR",
        "WARNING",
    ];
    assert_eq!(classes.len(), 75);

    for name in classes {
        let result = runtime
            .eval(&format!(
                "(let ((class (find-class '{name}))) \
                   (or (typep class 'built-in-class) \
                       (typep class 'standard-class)))"
            ))
            .unwrap_or_else(|error| panic!("{name} class lookup failed: {error}"));
        assert_eq!(result, Word::TRUE, "{name} did not have a class metaclass");
    }
}

#[test]
fn nil_empty_type_and_null_class_are_distinct_lisp_designators() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let result = runtime
        .eval(
            "(and (eq (find-class 'nil) (find-class 'null)) \
                 (not (typep nil nil)) \
                 (typep nil (find-class 'nil)) \
                 (typep nil 'null) \
                 (subtypep 'nil (find-class 'integer)) \
                 (typep 17 (list 'and (find-class 'integer) (find-class 't))) \
                 (subtypep (list 'or (find-class 'integer) (find-class 'null)) \
                           (find-class 't)))",
        )
        .unwrap_or_else(|error| panic!("NIL/NULL type integration failed: {error}"));
    assert_eq!(result, Word::TRUE);
}
