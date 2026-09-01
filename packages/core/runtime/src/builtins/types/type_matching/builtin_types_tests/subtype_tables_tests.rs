use crate::Environment;
use crate::builtins::types::subtype_tables::{compound_subtype_named, named_subtype_relation};
use crate::value::StructureDefinition;

#[test]
fn named_subtype_relation_covers_builtin_atomic_hierarchy() {
    let environment = Environment::new();
    let true_cases = [
        ("NIL", "SYMBOL"),
        ("NIL", "LIST"),
        ("NIL", "SEQUENCE"),
        ("NIL", "BOOLEAN"),
        ("NULL", "NIL"),
        ("BOOLEAN", "SYMBOL"),
        ("NUMBER", "REAL"),
        ("REAL", "NUMBER"),
        ("RATIO", "RATIONAL"),
        ("RATIO", "NUMBER"),
        ("RATIO", "REAL"),
        ("RATIONAL", "NUMBER"),
        ("RATIONAL", "REAL"),
        ("FLOAT", "NUMBER"),
        ("FLOAT", "REAL"),
        ("SHORT-FLOAT", "FLOAT"),
        ("SINGLE-FLOAT", "FLOAT"),
        ("DOUBLE-FLOAT", "FLOAT"),
        ("LONG-FLOAT", "FLOAT"),
        ("BASE-CHAR", "CHARACTER"),
        ("STANDARD-CHAR", "BASE-CHAR"),
        ("STANDARD-CHAR", "CHARACTER"),
        ("EXTENDED-CHAR", "CHARACTER"),
        ("CHARACTER", "ATOM"),
        ("SYMBOL", "ATOM"),
        ("SIMPLE-STRING", "STRING"),
        ("SIMPLE-STRING", "BASE-STRING"),
        ("SIMPLE-STRING", "SIMPLE-BASE-STRING"),
        ("SIMPLE-STRING", "SEQUENCE"),
        ("KEYWORD", "SYMBOL"),
        ("CONS", "LIST"),
        ("CONS", "SEQUENCE"),
        ("LIST", "SEQUENCE"),
        ("VECTOR", "SIMPLE-VECTOR"),
        ("VECTOR", "ARRAY"),
        ("VECTOR", "SIMPLE-ARRAY"),
        ("VECTOR", "SEQUENCE"),
        ("BIT-VECTOR", "SIMPLE-BIT-VECTOR"),
        ("BIT-VECTOR", "VECTOR"),
        ("BIT-VECTOR", "SIMPLE-VECTOR"),
        ("BIT-VECTOR", "ARRAY"),
        ("BIT-VECTOR", "SIMPLE-ARRAY"),
        ("BIT-VECTOR", "SEQUENCE"),
        ("ARRAY", "SIMPLE-ARRAY"),
        ("COMPILED-FUNCTION", "FUNCTION"),
        ("UNDEFINED-FUNCTION", "CELL-ERROR"),
        ("UNBOUND-SLOT", "CELL-ERROR"),
        ("END-OF-FILE", "STREAM-ERROR"),
        ("STREAM", "ATOM"),
        ("RESTART", "ATOM"),
    ];
    for (subtype, supertype) in true_cases {
        assert_eq!(
            named_subtype_relation(subtype, supertype, &environment),
            Some(true),
            "{subtype} <: {supertype} should be known and true"
        );
    }

    let false_cases = [("INTEGER", "STRING"), ("RATIO", "CHARACTER")];
    for (subtype, supertype) in false_cases {
        assert_eq!(
            named_subtype_relation(subtype, supertype, &environment),
            Some(false),
            "{subtype} <: {supertype} should be known and false"
        );
    }

    assert_eq!(
        named_subtype_relation("INTEGER", "NOT-A-REAL-TYPE", &environment),
        None,
        "unknown supertype must report an unknown relation"
    );
}

#[test]
fn named_subtype_relation_consults_structure_type_names() {
    let environment = Environment::new();
    environment.define_structure(
        "custom-struct",
        StructureDefinition {
            slots: Vec::new(),
            type_names: vec!["CUSTOM-STRUCT".to_string(), "CUSTOM-PARENT".to_string()],
        },
    );

    assert_eq!(
        named_subtype_relation("CUSTOM-STRUCT", "CUSTOM-PARENT", &environment),
        Some(true),
        "structure subtype relation must consult declared type names"
    );
    assert_eq!(
        named_subtype_relation("CUSTOM-STRUCT", "STRUCTURE", &environment),
        Some(true),
        "every structure instance is a STRUCTURE"
    );
    assert_eq!(
        named_subtype_relation("CUSTOM-STRUCT", "UNRELATED-PARENT", &environment),
        None,
        "an unlisted, unknown parent name reports an unknown relation"
    );
}

#[test]
fn compound_subtype_named_covers_every_operator_family() {
    let true_cases = [
        ("INTEGER", "INTEGER"),
        ("INTEGER", "RATIONAL"),
        ("INTEGER", "NUMBER"),
        ("INTEGER", "REAL"),
        ("INTEGER", "ATOM"),
        ("MOD", "NUMBER"),
        ("MOD", "ATOM"),
        ("SIGNED-BYTE", "REAL"),
        ("UNSIGNED-BYTE", "ATOM"),
        ("CONS", "CONS"),
        ("CONS", "LIST"),
        ("CONS", "SEQUENCE"),
        ("VECTOR", "VECTOR"),
        ("VECTOR", "SIMPLE-VECTOR"),
        ("VECTOR", "ARRAY"),
        ("VECTOR", "SIMPLE-ARRAY"),
        ("VECTOR", "SEQUENCE"),
        ("VECTOR", "ATOM"),
        ("SIMPLE-VECTOR", "ATOM"),
        ("BIT-VECTOR", "BIT-VECTOR"),
        ("BIT-VECTOR", "SIMPLE-BIT-VECTOR"),
        ("BIT-VECTOR", "ATOM"),
        ("SIMPLE-BIT-VECTOR", "ATOM"),
        ("ARRAY", "ARRAY"),
        ("ARRAY", "SIMPLE-ARRAY"),
        ("ARRAY", "ATOM"),
        ("SIMPLE-ARRAY", "ATOM"),
    ];
    for (operator, supertype) in true_cases {
        assert!(
            compound_subtype_named(operator, supertype),
            "{operator} should be a recognized subtype of {supertype}"
        );
    }

    let false_cases = [
        ("INTEGER", "STRING"),
        ("MOD", "STRING"),
        ("CONS", "ATOM"),
        ("VECTOR", "STRING"),
        ("UNKNOWN-OPERATOR", "ATOM"),
    ];
    for (operator, supertype) in false_cases {
        assert!(
            !compound_subtype_named(operator, supertype),
            "{operator} should not be a recognized subtype of {supertype}"
        );
    }
}
