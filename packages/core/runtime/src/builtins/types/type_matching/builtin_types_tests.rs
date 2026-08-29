use super::{
    characterp, ecase_error, endp, equalp_value, etypecase_error, keywordp,
    simple_condition_format_arguments, simple_condition_format_control, simple_vector_p,
    subtypep_value, symbol_name_value, symbol_package_value, the_check, type_designator_name,
    typep_value, vectorp,
};
use crate::{Environment, RuntimeError, Value};

fn compound(operator: &str, arguments: Vec<Value>) -> Value {
    let mut items = vec![Value::symbol(operator)];
    items.extend(arguments);
    Value::list(items)
}

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
fn typep_supports_logical_and_numeric_designators() {
    let integer = Value::Integer(7);
    let and_result = match typep_value(
        &integer,
        &compound(
            "and",
            vec![Value::symbol("number"), Value::symbol("integer")],
        ),
    ) {
        Ok(result) => result,
        Err(error) => panic!("valid AND type designator: {error}"),
    };
    assert!(and_result);
    let integer_result = match typep_value(
        &integer,
        &compound("integer", vec![Value::Integer(0), Value::Integer(10)]),
    ) {
        Ok(result) => result,
        Err(error) => panic!("valid INTEGER type designator: {error}"),
    };
    assert!(integer_result);
    let mod_result = match typep_value(&integer, &compound("mod", vec![Value::Integer(4)])) {
        Ok(result) => result,
        Err(error) => panic!("valid MOD type designator: {error}"),
    };
    assert!(!mod_result);
}

#[test]
fn typep_classifies_atomic_values_from_table_cases() {
    let cases = [
        (Value::Nil, "null", true),
        (Value::Unbound, "unbound", true),
        (Value::Boolean(true), "boolean", true),
        (Value::Integer(7), "integer", true),
        (Value::Integer(7), "float", false),
        (Value::Float(2.5), "real", true),
        (Value::Character('x'), "character", true),
        (Value::symbol("answer"), "symbol", true),
        (Value::keyword("answer"), "keyword", true),
        (Value::list(vec![Value::Integer(1)]), "cons", true),
        (Value::vector(vec![Value::Integer(1)]), "vector", true),
    ];

    for (value, designator, expected) in cases {
        let actual = typep_value(&value, &Value::symbol(designator))
            .unwrap_or_else(|error| panic!("{designator} rejected for {value:?}: {error}"));
        assert_eq!(actual, expected, "{value:?} against {designator}");
    }
}

#[test]
fn typep_covers_atomic_designator_matrix() {
    let cases = [
        (Value::Nil, "list", true),
        (Value::Nil, "atom", true),
        (Value::list(vec![Value::Integer(1)]), "atom", false),
        (Value::String("text".into()), "string", true),
        (Value::String("text".into()), "sequence", true),
        (Value::String("text".into()), "vector", false),
        (Value::Float(1.0), "rational", false),
        (Value::Character('x'), "atom", true),
        (Value::Character('x'), "sequence", false),
        (
            Value::vector(vec![Value::Integer(0), Value::Integer(1)]),
            "bit-vector",
            true,
        ),
        (Value::vector(vec![Value::Integer(2)]), "bit-vector", false),
        (Value::Unbound, "unbound", true),
        (Value::Unbound, "values", false),
        (Value::Values(vec![Value::Nil].into()), "values", true),
        (Value::Values(vec![Value::Nil].into()), "atom", true),
    ];

    for (value, designator, expected) in cases {
        let actual = typep_value(&value, &Value::symbol(designator))
            .unwrap_or_else(|error| panic!("{designator} rejected for {value:?}: {error}"));
        assert_eq!(actual, expected, "{value:?} against {designator}");
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
fn subtypep_table_covers_integer_boundaries_and_logical_relations() {
    let environment = Environment::new();
    let cases = [
        (
            compound("integer", vec![]),
            compound("integer", vec![]),
            true,
        ),
        (
            compound("integer", vec![Value::Integer(0), Value::Integer(10)]),
            compound("integer", vec![Value::Integer(-1), Value::Integer(10)]),
            true,
        ),
        (
            compound("integer", vec![Value::Integer(0), Value::Integer(10)]),
            compound("integer", vec![Value::Integer(1), Value::Integer(9)]),
            false,
        ),
        (
            compound(
                "or",
                vec![Value::symbol("integer"), Value::symbol("string")],
            ),
            Value::symbol("atom"),
            true,
        ),
        (
            Value::symbol("integer"),
            compound("or", vec![Value::symbol("number"), Value::symbol("string")]),
            true,
        ),
    ];
    for (subtype, supertype, expected) in cases {
        let result = match subtypep_value(&subtype, &supertype, &environment) {
            Ok(result) => result,
            Err(error) => panic!("valid subtype relation: {error}"),
        };
        let Value::Values(values) = result else {
            panic!("SUBTYPEP must return two values")
        };
        assert_eq!(
            values.as_ref()[0].is_truthy(),
            expected,
            "{subtype:?} <: {supertype:?}"
        );
    }
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

#[test]
fn typep_rejects_malformed_compound_designators() {
    let malformed = compound("not", Vec::new());
    let error = match typep_value(&Value::Nil, &malformed) {
        Ok(value) => panic!("NOT should reject zero arguments, got {value}"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("expects between 1 and 1"));

    let dotted = Value::dotted_list(vec![Value::symbol("or")], Value::symbol("integer"));
    assert!(typep_value(&Value::Nil, &dotted).is_err());
}

#[test]
fn subtypep_reports_known_and_unknown_relations() {
    let environment = Environment::new();
    let known = match subtypep_value(
        &Value::symbol("integer"),
        &Value::symbol("number"),
        &environment,
    ) {
        Ok(result) => result,
        Err(error) => panic!("known subtype designators: {error}"),
    };
    let Value::Values(values) = known else {
        panic!("SUBTYPEP returns two values");
    };
    assert!(matches!(
        values.as_ref().as_slice(),
        [Value::Boolean(true), Value::Boolean(true)]
    ));

    let unknown = subtypep_value(
        &Value::symbol("integer"),
        &Value::symbol("not-a-type"),
        &environment,
    );
    assert!(unknown.is_err());
}

#[test]
fn subtypep_accepts_supported_compound_designators() {
    let environment = Environment::new();
    let designators = vec![
        compound(
            "or",
            vec![Value::symbol("integer"), Value::symbol("number")],
        ),
        compound(
            "and",
            vec![Value::symbol("integer"), Value::symbol("number")],
        ),
        compound("not", vec![Value::symbol("integer")]),
        compound("eql", vec![Value::Integer(1)]),
        compound("member", vec![Value::Integer(1), Value::Integer(2)]),
        compound("integer", vec![Value::Integer(0), Value::Integer(10)]),
        compound("mod", vec![Value::Integer(4)]),
        compound("signed-byte", vec![Value::Integer(8)]),
        compound("unsigned-byte", vec![Value::Integer(8)]),
        compound(
            "cons",
            vec![Value::symbol("integer"), Value::symbol("number")],
        ),
        compound("vector", vec![Value::symbol("integer"), Value::Integer(2)]),
        compound("simple-vector", vec![Value::Integer(2)]),
        compound("bit-vector", vec![Value::Integer(2)]),
        compound("simple-bit-vector", vec![Value::Integer(2)]),
        compound("array", vec![Value::symbol("integer"), Value::Integer(2)]),
        compound(
            "simple-array",
            vec![Value::symbol("integer"), Value::symbol("*")],
        ),
    ];

    for designator in designators {
        let result = subtypep_value(&designator, &Value::symbol("t"), &environment);
        assert!(result.is_ok(), "valid designator rejected: {designator:?}");
    }
}

#[test]
fn subtypep_rejects_invalid_compound_designators() {
    let environment = Environment::new();
    let designators = vec![
        compound("not", Vec::new()),
        compound("eql", vec![Value::Integer(1), Value::Integer(2)]),
        compound("mod", vec![Value::Integer(-1)]),
        compound("mod", vec![Value::symbol("integer")]),
        compound("signed-byte", vec![Value::Integer(-1)]),
        compound("vector", vec![Value::symbol("integer"), Value::Integer(-1)]),
        compound("array", vec![Value::symbol("integer"), Value::Integer(-1)]),
        compound(
            "array",
            vec![
                Value::symbol("integer"),
                Value::list(vec![Value::String("dimension".into())]),
            ],
        ),
        compound(
            "array",
            vec![
                Value::symbol("integer"),
                Value::list(vec![Value::Integer(-1)]),
            ],
        ),
        compound(
            "array",
            vec![Value::symbol("integer"), Value::symbol("invalid")],
        ),
        Value::dotted_list(vec![Value::symbol("or")], Value::symbol("integer")),
    ];

    for designator in designators {
        let result = subtypep_value(&designator, &Value::symbol("t"), &environment);
        assert!(
            result.is_err(),
            "invalid designator accepted: {designator:?}"
        );
    }
}

#[test]
fn type_builtins_reject_wrong_arity_and_invalid_designators() {
    let predicates = [characterp, endp, keywordp, simple_vector_p, vectorp];
    for predicate in predicates {
        assert!(predicate(&[]).is_err());
        assert!(predicate(&[Value::Nil, Value::Nil]).is_err());
    }

    assert!(type_designator_name("typep", &Value::Integer(1)).is_err());
    assert!(symbol_name_value(&[Value::Integer(1)]).is_err());
    assert!(symbol_package_value(&[Value::Integer(1)]).is_err());
    assert!(the_check(&[Value::Integer(1)]).is_err());
}

#[test]
fn case_fallback_builtins_always_report_invalid_forms() {
    for fallback in [ecase_error, etypecase_error] {
        let error = match fallback(&[]) {
            Err(error) => error,
            Ok(value) => panic!("fallback returned {value:?}"),
        };
        assert!(error.to_string().contains("fell through"));
        assert!(fallback(&[Value::Nil]).is_err());
    }
}

#[test]
fn condition_format_accessors_reject_invalid_values_and_arities() {
    for accessor in [
        simple_condition_format_control,
        simple_condition_format_arguments,
    ] {
        assert!(accessor(&[]).is_err());
        assert!(accessor(&[Value::Nil, Value::Nil]).is_err());
        assert!(accessor(&[Value::Integer(1)]).is_err());
    }
}
