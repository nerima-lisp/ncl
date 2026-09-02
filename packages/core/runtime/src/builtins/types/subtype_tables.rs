#![allow(clippy::wildcard_imports)]
use super::*;

pub(super) fn compound_subtype_named(operator: &str, supertype_name: &str) -> bool {
    match operator {
        "INTEGER" => matches!(
            supertype_name,
            "INTEGER" | "RATIONAL" | "NUMBER" | "REAL" | "ATOM"
        ),
        "MOD" | "SIGNED-BYTE" | "UNSIGNED-BYTE" => matches!(
            supertype_name,
            "INTEGER" | "RATIONAL" | "NUMBER" | "REAL" | "ATOM"
        ),
        "CONS" => matches!(supertype_name, "CONS" | "LIST" | "SEQUENCE"),
        "VECTOR" | "SIMPLE-VECTOR" => matches!(
            supertype_name,
            "VECTOR" | "SIMPLE-VECTOR" | "ARRAY" | "SIMPLE-ARRAY" | "SEQUENCE" | "ATOM"
        ),
        "BIT-VECTOR" | "SIMPLE-BIT-VECTOR" => matches!(
            supertype_name,
            "BIT-VECTOR"
                | "SIMPLE-BIT-VECTOR"
                | "VECTOR"
                | "SIMPLE-VECTOR"
                | "ARRAY"
                | "SIMPLE-ARRAY"
                | "SEQUENCE"
                | "ATOM"
        ),
        "ARRAY" | "SIMPLE-ARRAY" => {
            matches!(supertype_name, "ARRAY" | "SIMPLE-ARRAY" | "ATOM")
        }
        _ => false,
    }
}

pub(super) fn named_subtype_relation(
    subtype_name: &str,
    supertype_name: &str,
    environment: &Environment,
) -> Option<bool> {
    if subtype_name == supertype_name
        || matches!(supertype_name, "T" | "OBJECT")
        || builtin_subtype(subtype_name, supertype_name)
    {
        return Some(true);
    }

    if let Some(class) = environment.lookup_class(subtype_name)
        && class
            .precedence
            .iter()
            .any(|name| name.eq_ignore_ascii_case(supertype_name))
    {
        return Some(true);
    }
    if let Some(structure) = environment.lookup_structure(subtype_name)
        && (supertype_name == "STRUCTURE"
            || structure
                .type_names
                .iter()
                .any(|name| name.eq_ignore_ascii_case(supertype_name)))
    {
        return Some(true);
    }
    if environment.lookup_condition(subtype_name).is_some() {
        return Some(condition_subtype_relation(
            subtype_name,
            supertype_name,
            environment,
            &mut std::collections::HashSet::new(),
        ));
    }
    if known_type_name(subtype_name, environment) && known_type_name(supertype_name, environment) {
        Some(false)
    } else {
        None
    }
}

fn condition_subtype_relation(
    subtype_name: &str,
    supertype_name: &str,
    environment: &Environment,
    visited: &mut std::collections::HashSet<String>,
) -> bool {
    if !visited.insert(subtype_name.to_owned()) {
        return false;
    }
    let Some(definition) = environment.lookup_condition(subtype_name) else {
        return false;
    };
    definition.parents.iter().any(|parent| {
        parent.eq_ignore_ascii_case(supertype_name)
            || builtin_subtype(parent, supertype_name)
            || condition_subtype_relation(parent, supertype_name, environment, visited)
    })
}

fn builtin_subtype(subtype_name: &str, supertype_name: &str) -> bool {
    match subtype_name {
        "NIL" | "NULL" => matches!(
            supertype_name,
            "SYMBOL" | "LIST" | "SEQUENCE" | "ATOM" | "BOOLEAN" | "NIL" | "NULL"
        ),
        "BOOLEAN" => matches!(supertype_name, "SYMBOL" | "ATOM"),
        "NUMBER" => matches!(supertype_name, "REAL" | "ATOM"),
        "REAL" => matches!(supertype_name, "NUMBER" | "ATOM"),
        "FIXNUM" | "BIGNUM" | "BIT" => matches!(
            supertype_name,
            "INTEGER" | "RATIONAL" | "NUMBER" | "REAL" | "ATOM"
        ),
        "INTEGER" => matches!(supertype_name, "RATIONAL" | "NUMBER" | "REAL" | "ATOM"),
        "RATIO" => matches!(supertype_name, "RATIONAL" | "NUMBER" | "REAL" | "ATOM"),
        "RATIONAL" => matches!(supertype_name, "NUMBER" | "REAL" | "ATOM"),
        "SHORT-FLOAT" | "SINGLE-FLOAT" | "DOUBLE-FLOAT" | "LONG-FLOAT" | "FLOAT" => {
            matches!(supertype_name, "FLOAT" | "NUMBER" | "REAL" | "ATOM")
        }
        "BASE-CHAR" => matches!(supertype_name, "CHARACTER" | "ATOM"),
        "STANDARD-CHAR" => matches!(supertype_name, "BASE-CHAR" | "CHARACTER" | "ATOM"),
        "EXTENDED-CHAR" => matches!(supertype_name, "CHARACTER" | "ATOM"),
        "CHARACTER" | "SYMBOL" => supertype_name == "ATOM",
        "STRING" | "BASE-STRING" => {
            matches!(
                supertype_name,
                "STRING" | "BASE-STRING" | "VECTOR" | "ARRAY" | "SEQUENCE" | "ATOM"
            )
        }
        "SIMPLE-STRING" | "SIMPLE-BASE-STRING" => matches!(
            supertype_name,
            "STRING" | "BASE-STRING" | "SIMPLE-STRING" | "SIMPLE-BASE-STRING"
                | "VECTOR" | "ARRAY" | "SIMPLE-ARRAY" | "SEQUENCE" | "ATOM"
        ),
        "KEYWORD" => matches!(supertype_name, "SYMBOL" | "ATOM"),
        "CONS" => matches!(supertype_name, "LIST" | "SEQUENCE"),
        "LIST" => supertype_name == "SEQUENCE",
        "VECTOR" | "SIMPLE-VECTOR" => matches!(
            supertype_name,
            "VECTOR" | "SIMPLE-VECTOR" | "ARRAY" | "SIMPLE-ARRAY" | "SEQUENCE" | "ATOM"
        ),
        "BIT-VECTOR" | "SIMPLE-BIT-VECTOR" => matches!(
            supertype_name,
            "BIT-VECTOR"
                | "SIMPLE-BIT-VECTOR"
                | "VECTOR"
                | "SIMPLE-VECTOR"
                | "ARRAY"
                | "SIMPLE-ARRAY"
                | "SEQUENCE"
                | "ATOM"
        ),
        "ARRAY" | "SIMPLE-ARRAY" => {
            matches!(supertype_name, "ARRAY" | "SIMPLE-ARRAY" | "ATOM")
        }
        "COMPILED-FUNCTION" => matches!(supertype_name, "FUNCTION" | "ATOM"),
        "DIVISION-BY-ZERO" => matches!(supertype_name, "ARITHMETIC-ERROR" | "ERROR" | "SERIOUS-CONDITION" | "CONDITION"),
        "END-OF-FILE" => matches!(supertype_name, "STREAM-ERROR" | "ERROR" | "SERIOUS-CONDITION" | "CONDITION"),
        "UNDEFINED-FUNCTION" | "UNBOUND-SLOT" => matches!(supertype_name, "CELL-ERROR" | "ERROR" | "SERIOUS-CONDITION" | "CONDITION"),
        "CELL-ERROR" => matches!(supertype_name, "ERROR" | "SERIOUS-CONDITION" | "CONDITION"),
        "STREAM-ERROR" => matches!(supertype_name, "ERROR" | "SERIOUS-CONDITION" | "CONDITION"),
        "STORAGE-CONDITION" | "PARSE-ERROR" => matches!(supertype_name, "SERIOUS-CONDITION" | "CONDITION"),
        "CONTROL-ERROR" => matches!(supertype_name, "ERROR" | "SERIOUS-CONDITION" | "CONDITION"),
        "FUNCTION" | "STREAM" | "PACKAGE" | "ENVIRONMENT" | "HASH-TABLE" | "CONDITION"
        | "RESTART" | "STRUCTURE" | "UNBOUND" | "VALUES" | "CLASS" | "STANDARD-OBJECT" => {
            supertype_name == "ATOM"
        }
        _ => false,
    }
}
