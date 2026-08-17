use std::cell::RefCell;
use std::rc::Rc;

use crate::environment::Environment;
use crate::error::RuntimeError;

use super::{ClassDefinition, MethodDefinition, Value};

type ValueCase = (&'static str, Value, Value, bool);

fn assert_cases(
    cases: impl IntoIterator<Item = ValueCase>,
    compare: impl Fn(&Value, &Value) -> bool,
) {
    for (name, left, right, expected) in cases {
        assert_eq!(compare(&left, &right), expected, "case: {name}");
    }
}

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

fn method(id: u64) -> Value {
    Value::Method(Rc::new(MethodDefinition {
        id,
        generic_function: "TEST-GENERIC".to_string(),
        lambda_list: Value::Nil,
        qualifiers: Vec::new(),
        specializers: Vec::new(),
        function: Value::builtin("TEST-BUILTIN", test_builtin),
    }))
}

fn eq_cases() -> Vec<ValueCase> {
    let stream = Value::string_input_stream("stream", 0, 6);
    let displaced = Value::vector_with_fill_pointer_element_type_adjustable_and_displacement(
        vec![Value::Integer(1)],
        None,
        Value::symbol("T"),
        false,
        Some(Value::vector(vec![Value::Integer(1)])),
        0,
    );
    let displaced_storage = Rc::new(RefCell::new(vec![Value::Integer(1), Value::Integer(2)]));
    let displaced_with_same_storage =
        Value::vector_with_storage_fill_pointer_element_type_adjustable_and_displacement(
            displaced_storage.clone(),
            1,
            None,
            Value::symbol("T"),
            false,
            Some(Value::vector(vec![Value::Integer(1)])),
            0,
        );
    let displaced_with_same_storage_clone = displaced_with_same_storage.clone();
    let array_storage = Rc::new(RefCell::new(vec![Value::Integer(1), Value::Integer(2)]));
    let array_with_same_storage =
        Value::array_with_storage_element_type_adjustable_and_displacement(
            vec![2],
            array_storage.clone(),
            Value::symbol("T"),
            false,
            None,
            0,
        );
    let array_with_same_storage_clone = array_with_same_storage.clone();
    let condition = Value::condition_from_parts(
        "TEST-CONDITION".to_string(),
        "message".to_string(),
        None,
        Vec::new(),
    );
    let restart = Value::restart("TEST-RESTART");
    let structure = Value::structure_with_types(
        "TEST-STRUCTURE",
        vec![("SLOT".to_string(), Value::Integer(1))],
        Vec::new(),
    );
    let class_value = Value::class_object(class("TEST-CLASS"));
    let environment = Value::environment(Environment::new());
    let instance = Value::instance(
        class("TEST-CLASS"),
        vec![("SLOT".to_string(), Value::Integer(1))],
    );
    let dotted = Value::dotted_list(vec![Value::Integer(1)], Value::Integer(2));
    let function = Value::builtin("TEST-BUILTIN", test_builtin);

    vec![
        ("nil", Value::Nil, Value::Nil, true),
        ("unbound", Value::Unbound, Value::Unbound, true),
        ("nil and false", Value::Nil, Value::Boolean(false), true),
        (
            "boolean values",
            Value::Boolean(true),
            Value::Boolean(false),
            false,
        ),
        ("integers", Value::Integer(1), Value::Integer(1), true),
        (
            "rational values",
            Value::rational(1, 2).unwrap(),
            Value::rational(1, 2).unwrap(),
            true,
        ),
        ("floats", Value::Float(1.5), Value::Float(1.5), true),
        (
            "complex values",
            Value::complex(Value::Integer(1), Value::Integer(2)),
            Value::complex(Value::Integer(1), Value::Integer(2)),
            true,
        ),
        (
            "characters",
            Value::Character('a'),
            Value::Character('a'),
            true,
        ),
        ("streams by pointer", stream.clone(), stream, true),
        (
            "distinct streams",
            Value::string_input_stream("stream", 0, 6),
            Value::string_input_stream("stream", 0, 6),
            false,
        ),
        (
            "packages by name",
            Value::package("TEST"),
            Value::package("TEST"),
            true,
        ),
        (
            "strings by pointer",
            Value::string("same"),
            Value::string("same"),
            false,
        ),
        (
            "symbols by name",
            Value::symbol("same"),
            Value::symbol("same"),
            true,
        ),
        (
            "exact symbols by name",
            Value::symbol_exact("same"),
            Value::symbol_exact("same"),
            true,
        ),
        (
            "uninterned symbols by pointer",
            Value::uninterned_symbol("same"),
            Value::uninterned_symbol("same"),
            false,
        ),
        (
            "lists by pointer",
            Value::list(vec![Value::Integer(1)]),
            Value::list(vec![Value::Integer(1)]),
            false,
        ),
        (
            "vectors by pointer",
            Value::vector(vec![Value::Integer(1)]),
            Value::vector(vec![Value::Integer(1)]),
            false,
        ),
        (
            "vectors with displacement by pointer",
            displaced.clone(),
            displaced,
            true,
        ),
        (
            "vectors sharing storage and displacement",
            displaced_with_same_storage,
            displaced_with_same_storage_clone,
            true,
        ),
        (
            "arrays by pointer",
            Value::array(vec![1], vec![Value::Integer(1)]),
            Value::array(vec![1], vec![Value::Integer(1)]),
            false,
        ),
        (
            "arrays sharing storage",
            array_with_same_storage,
            array_with_same_storage_clone,
            true,
        ),
        (
            "hash tables by entries pointer",
            Value::hash_table("eq"),
            Value::hash_table("eq"),
            false,
        ),
        (
            "multiple values by pointer",
            Value::values(vec![Value::Nil]),
            Value::values(vec![Value::Nil]),
            false,
        ),
        ("conditions by pointer", condition.clone(), condition, true),
        (
            "distinct conditions",
            Value::condition_from_parts(
                "TEST-CONDITION".to_string(),
                "message".to_string(),
                None,
                Vec::new(),
            ),
            Value::condition_from_parts(
                "TEST-CONDITION".to_string(),
                "message".to_string(),
                None,
                Vec::new(),
            ),
            false,
        ),
        ("restarts by pointer", restart.clone(), restart, true),
        ("structures by pointer", structure.clone(), structure, true),
        ("classes by pointer", class_value.clone(), class_value, true),
        (
            "environments by identity",
            environment.clone(),
            environment,
            true,
        ),
        (
            "distinct environments",
            Value::environment(Environment::new()),
            Value::environment(Environment::new()),
            false,
        ),
        (
            "instances by storage pointer",
            instance.clone(),
            instance,
            true,
        ),
        ("methods by id", method(1), method(1), true),
        ("methods with different ids", method(1), method(2), false),
        ("dotted lists by pointer", dotted.clone(), dotted, true),
        ("functions by pointer", function.clone(), function, true),
        (
            "different variants",
            Value::Integer(1),
            Value::Float(1.0),
            false,
        ),
    ]
}

fn equal_cases() -> Vec<ValueCase> {
    vec![
        (
            "complex values",
            Value::complex(Value::Integer(1), Value::Integer(2)),
            Value::complex(Value::Integer(1), Value::Integer(2)),
            true,
        ),
        (
            "strings by contents",
            Value::string("same"),
            Value::string("same"),
            true,
        ),
        (
            "different strings",
            Value::string("left"),
            Value::string("right"),
            false,
        ),
        (
            "lists by contents",
            Value::list(vec![Value::Integer(1), Value::string("x")]),
            Value::list(vec![Value::Integer(1), Value::string("x")]),
            true,
        ),
        (
            "lists with different lengths",
            Value::list(vec![Value::Integer(1)]),
            Value::list(vec![Value::Integer(1), Value::Integer(2)]),
            false,
        ),
        (
            "vectors by contents",
            Value::vector(vec![Value::Integer(1), Value::string("x")]),
            Value::vector(vec![Value::Integer(1), Value::string("x")]),
            true,
        ),
        (
            "vectors with different fill pointers",
            Value::vector_with_fill_pointer(vec![Value::Integer(1)], 1),
            Value::vector_with_fill_pointer(vec![Value::Integer(1)], 0),
            false,
        ),
        (
            "vectors with different element types",
            Value::vector_with_fill_pointer_and_element_type(
                vec![Value::Integer(1)],
                None,
                Value::symbol("INTEGER"),
            ),
            Value::vector_with_fill_pointer_and_element_type(
                vec![Value::Integer(1)],
                None,
                Value::symbol("STRING"),
            ),
            false,
        ),
        (
            "vectors with displacement",
            Value::vector_with_fill_pointer_element_type_adjustable_and_displacement(
                vec![Value::Integer(1)],
                None,
                Value::symbol("T"),
                false,
                Some(Value::vector(vec![Value::Integer(1)])),
                0,
            ),
            Value::vector_with_fill_pointer_element_type_adjustable_and_displacement(
                vec![Value::Integer(1)],
                None,
                Value::symbol("T"),
                false,
                Some(Value::vector(vec![Value::Integer(1)])),
                0,
            ),
            true,
        ),
        (
            "arrays by contents",
            Value::array(vec![2], vec![Value::Integer(1), Value::Integer(2)]),
            Value::array(vec![2], vec![Value::Integer(1), Value::Integer(2)]),
            true,
        ),
        (
            "arrays with different dimensions",
            Value::array(vec![2], vec![Value::Integer(1), Value::Integer(2)]),
            Value::array(vec![1, 2], vec![Value::Integer(1), Value::Integer(2)]),
            false,
        ),
        (
            "multiple values by contents",
            Value::values(vec![Value::Integer(1), Value::string("x")]),
            Value::values(vec![Value::Integer(1), Value::string("x")]),
            true,
        ),
        (
            "conditions by contents",
            Value::condition_from_definition(
                "TEST-CONDITION".to_string(),
                vec!["CONDITION".to_string(), "TEST-CONDITION".to_string()],
                vec![("SLOT".to_string(), Value::string("x"))],
                "message".to_string(),
                Some("~A".to_string()),
                vec![Value::Integer(1)],
            ),
            Value::condition_from_definition(
                "TEST-CONDITION".to_string(),
                vec!["CONDITION".to_string(), "TEST-CONDITION".to_string()],
                vec![("SLOT".to_string(), Value::string("x"))],
                "message".to_string(),
                Some("~A".to_string()),
                vec![Value::Integer(1)],
            ),
            true,
        ),
        (
            "restarts keep identity semantics",
            Value::restart("TEST-RESTART"),
            Value::restart("TEST-RESTART"),
            false,
        ),
        (
            "structures by contents",
            Value::structure_with_types(
                "TEST-STRUCTURE",
                vec![("SLOT".to_string(), Value::string("x"))],
                Vec::new(),
            ),
            Value::structure_with_types(
                "TEST-STRUCTURE",
                vec![("SLOT".to_string(), Value::string("x"))],
                Vec::new(),
            ),
            true,
        ),
        (
            "classes ignore case",
            Value::class_object(class("test-class")),
            Value::class_object(class("TEST-CLASS")),
            true,
        ),
        (
            "instances ignore class and slot case",
            Value::instance(
                class("test-class"),
                vec![("slot".to_string(), Value::string("x"))],
            ),
            Value::instance(
                class("TEST-CLASS"),
                vec![("SLOT".to_string(), Value::string("x"))],
            ),
            true,
        ),
        (
            "dotted lists by contents",
            Value::dotted_list(vec![Value::Integer(1)], Value::string("tail")),
            Value::dotted_list(vec![Value::Integer(1)], Value::string("tail")),
            true,
        ),
        (
            "scalar fallback",
            Value::Integer(1),
            Value::Integer(1),
            true,
        ),
        (
            "different scalar fallback",
            Value::Integer(1),
            Value::Integer(2),
            false,
        ),
    ]
}

#[test]
fn symbol_names_and_references_are_table_driven() {
    let cases = [
        (
            "symbol",
            Value::symbol("name"),
            Some("NAME"),
            Some(("NAME", false)),
        ),
        (
            "exact symbol",
            Value::symbol_exact("name"),
            Some("name"),
            Some(("name", true)),
        ),
        (
            "uninterned symbol",
            Value::uninterned_symbol("name"),
            Some("name"),
            Some(("name", false)),
        ),
        (
            "keyword",
            Value::keyword(":name"),
            Some("NAME"),
            Some(("NAME", false)),
        ),
        (
            "exact keyword",
            Value::keyword_exact(":name"),
            Some("name"),
            Some(("name", true)),
        ),
        ("nil", Value::Nil, Some("NIL"), Some(("NIL", false))),
        (
            "false",
            Value::Boolean(false),
            Some("NIL"),
            Some(("NIL", false)),
        ),
        ("true", Value::Boolean(true), Some("T"), Some(("T", false))),
        ("not a symbol", Value::Integer(1), None, None),
    ];

    for (name, value, expected_name, expected_reference) in cases {
        assert_eq!(value.symbol_name(), expected_name, "name case: {name}");
        assert_eq!(
            value.symbol_reference(),
            expected_reference,
            "reference case: {name}"
        );
    }
}

#[test]
fn eq_value_covers_identity_and_scalar_rules() {
    assert_cases(eq_cases(), Value::eq_value);
}

#[test]
fn equal_value_covers_structural_rules() {
    assert_cases(equal_cases(), Value::equal_value);
}
