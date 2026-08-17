use std::cell::RefCell;
use std::rc::Rc;

use crate::environment::Environment;
use crate::error::RuntimeError;

use super::{ClassDefinition, MethodDefinition, Value};

fn test_builtin(_: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Nil)
}

fn class(name: &str) -> Rc<ClassDefinition> {
    Rc::new(ClassDefinition {
        name: name.to_string(),
        precedence: Vec::new(),
        slots: Vec::new(),
        default_initargs: Vec::new(),
        documentation: Rc::new(RefCell::new(None)),
    })
}

fn method() -> Value {
    Value::Method(Rc::new(MethodDefinition {
        id: 1,
        generic_function: "TEST-GENERIC".to_string(),
        lambda_list: Value::Nil,
        qualifiers: Vec::new(),
        specializers: Vec::new(),
        function: Value::builtin("TEST-BUILTIN", test_builtin),
    }))
}

fn condition(
    actual_type: &str,
    type_names: &[&str],
    slots: &[(&str, Value)],
    format_control: Option<&str>,
    format_arguments: Vec<Value>,
) -> Value {
    Value::condition_from_definition(
        actual_type.to_string(),
        type_names
            .iter()
            .map(|type_name| (*type_name).to_string())
            .collect(),
        slots
            .iter()
            .map(|(name, value)| ((*name).to_string(), value.clone()))
            .collect(),
        "test message".to_string(),
        format_control.map(str::to_string),
        format_arguments,
    )
}

fn assert_value_option(actual: Option<Value>, expected: Option<Value>) {
    match (actual, expected) {
        (Some(actual), Some(expected)) => assert!(actual.equal_value(&expected)),
        (None, None) => {}
        (actual, expected) => panic!("values differ: {actual:?} vs {expected:?}"),
    }
}

#[test]
fn type_name_covers_every_value_variant() {
    let cases = vec![
        ("nil", Value::Nil, "NIL"),
        ("unbound", Value::Unbound, "UNBOUND"),
        ("boolean", Value::Boolean(true), "BOOLEAN"),
        ("integer", Value::Integer(1), "INTEGER"),
        (
            "rational",
            Value::rational(1, 2).expect("valid rational"),
            "RATIO",
        ),
        ("float", Value::Float(1.5), "FLOAT"),
        (
            "complex",
            Value::complex(Value::Integer(1), Value::Integer(2)),
            "COMPLEX",
        ),
        ("string", Value::string("text"), "STRING"),
        ("character", Value::Character('A'), "CHARACTER"),
        (
            "string input stream",
            Value::string_input_stream("text", 0, 4),
            "STREAM",
        ),
        ("package", Value::package("TEST"), "PACKAGE"),
        (
            "environment",
            Value::environment(Environment::new()),
            "ENVIRONMENT",
        ),
        ("symbol", Value::symbol("TEST"), "SYMBOL"),
        ("exact symbol", Value::symbol_exact("TEST"), "SYMBOL"),
        (
            "uninterned symbol",
            Value::uninterned_symbol("TEST"),
            "SYMBOL",
        ),
        ("keyword", Value::keyword(":TEST"), "KEYWORD"),
        ("exact keyword", Value::keyword_exact("TEST"), "KEYWORD"),
        ("list", Value::list(vec![Value::Integer(1)]), "LIST"),
        (
            "dotted list",
            Value::dotted_list(vec![Value::Integer(1)], Value::Integer(2)),
            "LIST",
        ),
        ("vector", Value::vector(vec![Value::Integer(1)]), "VECTOR"),
        (
            "array",
            Value::array(vec![1], vec![Value::Integer(1)]),
            "ARRAY",
        ),
        ("hash table", Value::hash_table("eq"), "HASH-TABLE"),
        ("values", Value::values(vec![Value::Integer(1)]), "VALUES"),
        (
            "condition",
            condition("ERROR", &["CONDITION", "ERROR"], &[], None, Vec::new()),
            "CONDITION",
        ),
        ("restart", Value::restart("ABORT"), "RESTART"),
        (
            "structure",
            Value::structure_with_types("POINT", Vec::new(), Vec::new()),
            "STRUCTURE",
        ),
        ("class", Value::class_object(class("POINT")), "CLASS"),
        (
            "instance",
            Value::instance(class("POINT"), Vec::new()),
            "STANDARD-OBJECT",
        ),
        ("method", method(), "METHOD"),
        (
            "function",
            Value::builtin("TEST-BUILTIN", test_builtin),
            "FUNCTION",
        ),
    ];

    for (name, value, expected) in cases {
        assert_eq!(value.type_name(), expected, "type name case: {name}");
    }
}

#[test]
fn is_truthy_uses_the_primary_multiple_value() {
    let cases = [
        ("nil", Value::Nil, false),
        ("false", Value::Boolean(false), false),
        ("true", Value::Boolean(true), true),
        ("integer", Value::Integer(1), true),
        ("empty values", Value::values(Vec::new()), false),
        (
            "values headed by nil",
            Value::values(vec![Value::Nil, Value::Integer(1)]),
            false,
        ),
        (
            "values headed by false",
            Value::values(vec![Value::Boolean(false), Value::Integer(1)]),
            false,
        ),
        (
            "values headed by true",
            Value::values(vec![Value::Boolean(true), Value::Nil]),
            true,
        ),
    ];

    for (name, value, expected) in cases {
        assert_eq!(value.is_truthy(), expected, "truthiness case: {name}");
    }
}

#[test]
fn condition_is_type_handles_names_and_hierarchy() {
    let named = condition(
        "Test-Condition",
        &["Condition", "Base-Type"],
        &[],
        None,
        Vec::new(),
    );
    assert!(named.condition_is_type("test-condition"));
    assert!(named.condition_is_type(":BASE-TYPE"));
    assert!(named.condition_is_type("CONDITION"));
    assert!(!named.condition_is_type("MISSING"));
    assert!(!Value::Nil.condition_is_type("CONDITION"));

    let hierarchy_cases = [
        ("SIMPLE-ERROR", "ERROR", true),
        ("SIMPLE-ERROR", "SERIOUS-CONDITION", true),
        ("SIMPLE-ERROR", "SIMPLE-CONDITION", true),
        ("SIMPLE-ERROR", "WARNING", false),
        ("SIMPLE-WARNING", "WARNING", true),
        ("SIMPLE-WARNING", "SIMPLE-CONDITION", true),
        ("SIMPLE-WARNING", "ERROR", false),
        ("SIMPLE-CONDITION", "CONDITION", true),
        ("SIMPLE-CONDITION", "ERROR", false),
        ("DIVISION-BY-ZERO", "ARITHMETIC-ERROR", true),
        ("DIVISION-BY-ZERO", "SERIOUS-CONDITION", true),
        ("DIVISION-BY-ZERO", "WARNING", false),
        ("ARITHMETIC-ERROR", "ERROR", true),
        ("ARITHMETIC-ERROR", "SERIOUS-CONDITION", true),
        ("ARITHMETIC-ERROR", "WARNING", false),
        ("TYPE-ERROR", "ERROR", true),
        ("PROGRAM-ERROR", "ERROR", true),
        ("PACKAGE-ERROR", "ERROR", true),
        ("READER-ERROR", "ERROR", true),
        ("COMPILER-ERROR", "ERROR", true),
        ("FILE-ERROR", "ERROR", true),
        ("UNBOUND-VARIABLE", "ERROR", true),
        ("TYPE-ERROR", "WARNING", false),
        ("CONTROL-ERROR", "CONDITION", true),
        ("CONTROL-ERROR", "ERROR", false),
        ("UNKNOWN-CONDITION", "ERROR", false),
    ];

    for (actual_type, expected_type, expected) in hierarchy_cases {
        let value = condition(actual_type, &[actual_type], &[], None, Vec::new());
        assert_eq!(
            value.condition_is_type(expected_type),
            expected,
            "hierarchy case: {actual_type} is {expected_type}"
        );
    }
}

#[test]
fn condition_slots_and_formatting_metadata_are_accessible() {
    let arguments = vec![Value::Integer(7), Value::string("argument")];
    let value = condition(
        "TEST-CONDITION",
        &["CONDITION", "TEST-CONDITION"],
        &[
            ("Message", Value::string("old")),
            ("Count", Value::Integer(1)),
        ],
        Some("~A"),
        arguments.clone(),
    );

    assert_eq!(
        value.condition_type_names(),
        Some(vec!["CONDITION".to_string(), "TEST-CONDITION".to_string()])
    );
    assert_eq!(value.condition_type_name(), Some("TEST-CONDITION"));
    assert_eq!(value.condition_message(), Some("test message"));
    assert_eq!(value.simple_condition_format_control(), Some("~A"));
    let actual_arguments = value
        .simple_condition_format_arguments()
        .expect("format arguments are present");
    assert_eq!(actual_arguments.len(), arguments.len());
    assert!(
        actual_arguments
            .iter()
            .zip(arguments.iter())
            .all(|(actual, expected)| actual.equal_value(expected))
    );
    assert_value_option(
        value.condition_slot(":condition", "message"),
        Some(Value::string("old")),
    );
    assert_value_option(
        value.condition_slot("test-condition", "COUNT"),
        Some(Value::Integer(1)),
    );
    assert_value_option(value.condition_slot("MISSING", "Count"), None);
    assert_value_option(value.condition_slot("TEST-CONDITION", "Missing"), None);

    assert!(value.set_condition_slot("TEST-CONDITION", "message", Value::string("new")));
    assert_value_option(
        value.condition_slot("TEST-CONDITION", "MESSAGE"),
        Some(Value::string("new")),
    );
    assert!(value.set_condition_slot("CONDITION", "COUNT", Value::Integer(2)));
    assert_value_option(
        value.condition_slot("TEST-CONDITION", "count"),
        Some(Value::Integer(2)),
    );
    assert!(!value.set_condition_slot("TEST-CONDITION", "MISSING", Value::Nil));
    assert!(!value.set_condition_slot("MISSING", "COUNT", Value::Nil));
    assert!(!Value::Nil.set_condition_slot("CONDITION", "COUNT", Value::Nil));

    let without_format = condition("TEST-CONDITION", &["TEST-CONDITION"], &[], None, Vec::new());
    assert_eq!(without_format.simple_condition_format_control(), None);
    assert!(without_format.simple_condition_format_arguments().is_none());
    assert_eq!(
        without_format.condition_type_names(),
        Some(vec!["TEST-CONDITION".to_string()])
    );
    assert_eq!(without_format.condition_type_name(), Some("TEST-CONDITION"));
    assert_eq!(without_format.condition_message(), Some("test message"));

    let restart = Value::restart("ABORT");
    assert_eq!(restart.restart_name(), Some("ABORT"));
    assert_eq!(Value::Nil.restart_name(), None);
    assert_eq!(Value::Nil.condition_type_names(), None);
    assert_eq!(Value::Nil.condition_type_name(), None);
    assert_eq!(Value::Nil.condition_message(), None);
    assert_eq!(Value::Nil.simple_condition_format_control(), None);
    assert!(Value::Nil.simple_condition_format_arguments().is_none());
    assert!(Value::Nil.condition_slot("CONDITION", "COUNT").is_none());
}
