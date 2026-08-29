use crate::builtins::type_predicates::equalp_value;
use crate::builtins::types::predicates::{
    characterp, endp, keywordp, simple_vector_p, symbol_name_value, symbol_package_value, vectorp,
};
use crate::builtins::types::special_form_support::the_check;
use crate::builtins::types::type_designator::type_designator_name;
use crate::{RuntimeError, Value};

type PredicateCase = (
    fn(&[Value]) -> Result<Value, RuntimeError>,
    Vec<Value>,
    bool,
);

fn valid_value(result: Result<Value, RuntimeError>) -> Value {
    match result {
        Ok(value) => value,
        Err(error) => panic!("valid builtin arguments: {error}"),
    }
}

#[test]
fn predicates_and_symbol_accessors_cover_value_categories() {
    let predicate_cases: &[PredicateCase] = &[
        (characterp, vec![Value::Character('x')], true),
        (characterp, vec![Value::Integer(1)], false),
        (keywordp, vec![Value::keyword("answer")], true),
        (keywordp, vec![Value::symbol("answer")], false),
        (vectorp, vec![Value::vector(vec![Value::Nil])], true),
        (simple_vector_p, vec![Value::vector(Vec::new())], true),
    ];
    for (predicate, arguments, expected) in predicate_cases {
        let actual = valid_value(predicate(arguments));
        assert_eq!(
            actual.is_truthy(),
            *expected,
            "unexpected result {actual:?} for {arguments:?}"
        );
    }

    assert!(valid_value(endp(&[Value::Nil])).is_truthy());
    assert!(!valid_value(endp(&[Value::list(vec![Value::Nil])])).is_truthy());
    assert_eq!(
        valid_value(symbol_name_value(&[Value::symbol("pkg::answer")])).to_string(),
        "\"ANSWER\""
    );
    assert_eq!(
        valid_value(symbol_package_value(&[Value::keyword("answer")])).to_string(),
        "KEYWORD"
    );
}

#[test]
fn equalp_compares_nested_values_case_insensitively_and_falls_back_to_eql() {
    let cases = [
        (Value::string("Hello"), Value::string("hELLO"), true),
        (Value::Character('A'), Value::Character('a'), true),
        (
            Value::list(vec![Value::string("A"), Value::Integer(1)]),
            Value::list(vec![Value::string("a"), Value::Integer(1)]),
            true,
        ),
        (
            Value::dotted_list(vec![Value::string("A")], Value::Integer(1)),
            Value::dotted_list(vec![Value::string("a")], Value::Integer(1)),
            true,
        ),
        (Value::Integer(1), Value::String("1".into()), false),
    ];

    for (left, right, expected) in cases {
        assert_eq!(equalp_value(&left, &right), expected);
    }
}

#[test]
fn symbol_accessors_handle_all_symbol_representations() {
    let cases = [
        (Value::UninternedSymbol("scratch".into()), "scratch", "NIL"),
        (Value::symbol("pkg::answer"), "ANSWER", "PKG"),
        (Value::symbol("answer"), "ANSWER", "NCL-USER"),
        (Value::keyword("answer"), "ANSWER", "KEYWORD"),
        (Value::Nil, "NIL", "COMMON-LISP"),
        (Value::Boolean(true), "T", "COMMON-LISP"),
    ];
    for (value, expected_name, expected_package) in cases {
        assert_eq!(
            valid_value(symbol_name_value(std::slice::from_ref(&value))).to_string(),
            format!("\"{expected_name}\"")
        );
        assert_eq!(
            valid_value(symbol_package_value(std::slice::from_ref(&value))).to_string(),
            expected_package
        );
    }
}

#[test]
fn type_designator_names_normalize_boolean_and_package_forms() {
    let cases = [
        (Value::Nil, "NIL"),
        (Value::Boolean(false), "NIL"),
        (Value::Boolean(true), "T"),
        (Value::UninternedSymbol("scratch".into()), "SCRATCH"),
        (Value::keyword("pkg::answer"), "ANSWER"),
        (Value::SymbolExact("pkg::exact".into()), "EXACT"),
        (Value::KeywordExact("pkg::keyword".into()), "KEYWORD"),
    ];
    for (value, expected) in cases {
        let actual = match type_designator_name("test", &value) {
            Ok(actual) => actual,
            Err(error) => panic!("valid type designator {value:?}: {error}"),
        };
        assert_eq!(actual, expected);
    }
}

#[test]
fn type_designator_name_rejects_non_designators() {
    let error = type_designator_name("test", &Value::Integer(7));
    assert!(error.is_err());
}

#[test]
fn the_check_preserves_matching_values_and_rejects_mismatches() {
    let value = Value::Integer(7);
    assert_eq!(
        valid_value(the_check(&[value.clone(), Value::symbol("integer")])).to_string(),
        value.to_string()
    );
    assert!(the_check(&[Value::Integer(7), Value::symbol("string")]).is_err());
}
