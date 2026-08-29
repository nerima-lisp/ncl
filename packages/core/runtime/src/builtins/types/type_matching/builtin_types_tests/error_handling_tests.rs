use crate::Value;
use crate::builtins::types::predicates::{
    characterp, endp, keywordp, simple_condition_format_arguments, simple_condition_format_control,
    simple_vector_p, symbol_name_value, symbol_package_value, vectorp,
};
use crate::builtins::types::special_form_support::{ecase_error, etypecase_error, the_check};
use crate::builtins::types::type_designator::type_designator_name;

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
