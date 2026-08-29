use crate::builtins::types::subtype_entry::subtypep_value;
use crate::{Environment, Value};

use super::support::compound;

fn truthy(result: Value) -> bool {
    let Value::Values(values) = result else {
        panic!("SUBTYPEP must return two values");
    };
    values.as_ref()[0].is_truthy()
}

fn definite(result: &Value) -> bool {
    let Value::Values(values) = result else {
        panic!("SUBTYPEP must return two values");
    };
    values.as_ref()[1].is_truthy()
}

#[test]
fn subtype_relation_propagates_unknown_and_false_through_or_and_and() {
    let environment = Environment::new();

    // OR: a branch whose relation is undecidable (here, an atomic type
    // against an unhandled compound supertype) forces the overall relation
    // to unknown rather than false or an error.
    let unknown_or = subtypep_value(
        &compound("or", vec![Value::symbol("integer")]),
        &compound("mod", vec![Value::Integer(4)]),
        &environment,
    )
    .unwrap_or_else(|error| panic!("OR with an undecidable branch must not error: {error}"));
    assert!(
        !truthy(unknown_or.clone()),
        "unknown relations are not truthy"
    );
    assert!(!definite(&unknown_or), "unknown relations are not definite");

    // OR: a branch that is definitely not a subtype makes the whole OR false.
    let false_or = subtypep_value(
        &compound(
            "or",
            vec![Value::symbol("string"), Value::symbol("character")],
        ),
        &Value::symbol("integer"),
        &environment,
    )
    .unwrap_or_else(|error| panic!("OR with a false branch must not error: {error}"));
    assert!(!truthy(false_or));

    // AND: falls through to None (unknown) when no branch is a known subtype.
    let unknown_and = subtypep_value(
        &compound(
            "and",
            vec![Value::symbol("stream"), Value::symbol("restart")],
        ),
        &Value::symbol("integer"),
        &environment,
    )
    .unwrap_or_else(|error| panic!("AND with no matching branch must not error: {error}"));
    assert!(!truthy(unknown_and));
}

#[test]
fn subtype_relation_member_and_eql_report_false_for_non_subtype_candidates() {
    let environment = Environment::new();
    let member_false = subtypep_value(
        &compound("member", vec![Value::Integer(1), Value::Integer(2)]),
        &Value::symbol("string"),
        &environment,
    )
    .unwrap_or_else(|error| panic!("MEMBER against an unrelated type must not error: {error}"));
    assert!(!truthy(member_false));

    let eql_false = subtypep_value(
        &compound("eql", vec![Value::Integer(1)]),
        &Value::symbol("string"),
        &environment,
    )
    .unwrap_or_else(|error| panic!("EQL against an unrelated type must not error: {error}"));
    assert!(!truthy(eql_false));
}

#[test]
fn subtype_relation_matches_or_and_integer_on_the_supertype_side() {
    let environment = Environment::new();
    let matches_or_supertype = subtypep_value(
        &Value::symbol("integer"),
        &compound(
            "or",
            vec![Value::symbol("string"), Value::symbol("integer")],
        ),
        &environment,
    )
    .unwrap_or_else(|error| panic!("integer <: (or string integer) must not error: {error}"));
    assert!(truthy(matches_or_supertype));

    let named_integer_supertype = subtypep_value(
        &Value::symbol("fixnum"),
        &compound("integer", vec![]),
        &environment,
    )
    .unwrap_or_else(|error| panic!("fixnum <: (integer) must not error: {error}"));
    assert!(truthy(named_integer_supertype));

    let bounded_named_integer_supertype = subtypep_value(
        &Value::symbol("fixnum"),
        &compound("integer", vec![Value::Integer(0)]),
        &environment,
    )
    .unwrap_or_else(|error| panic!("fixnum <: (integer 0) must not error: {error}"));
    assert!(!truthy(bounded_named_integer_supertype));

    // An OR supertype where every branch is undecidable must itself report
    // the relation as unknown, not false.
    let unknown_or_supertype = subtypep_value(
        &Value::symbol("integer"),
        &compound("or", vec![compound("mod", vec![Value::Integer(4)])]),
        &environment,
    )
    .unwrap_or_else(|error| panic!("integer <: (or (mod 4)) must not error: {error}"));
    assert!(!truthy(unknown_or_supertype.clone()));
    assert!(!definite(&unknown_or_supertype));
}

#[test]
fn atomic_subtype_relation_declines_when_the_supertype_is_an_unhandled_compound() {
    let environment = Environment::new();
    // "MOD" is a valid compound designator, but the supertype-side match in
    // `subtype_relation` only special-cases OR/AND/INTEGER; anything else
    // falls through to the atomic fallback, which cannot name a type for a
    // compound (list) supertype and must report the relation as unknown.
    let relation = subtypep_value(
        &Value::symbol("integer"),
        &compound("mod", vec![Value::Integer(4)]),
        &environment,
    )
    .unwrap_or_else(|error| panic!("integer <: (mod 4) must not error: {error}"));
    assert!(
        !truthy(relation),
        "an atomic subtype vs. unhandled compound supertype is unknown"
    );
}
