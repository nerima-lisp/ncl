#![allow(
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "coverage tests assert on internal helper results"
)]

use super::*;
use crate::{condition_class, condition_class_of, make_condition};
use ncl_object::{
    MultipleValues, Package, car, cdr, make_instance, make_simple_vector, make_string,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    crate::register(&runtime).unwrap_or_else(|error| panic!("conditions: {error:?}"));
    (runtime, ctx)
}

fn symbol(ctx: &mut ThreadContext, runtime: &Runtime, package: &str, name: &str) -> Word {
    let package = runtime
        .ensure_package(ctx, package)
        .unwrap_or_else(|error| panic!("package: {error:?}"));
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap_or_else(|error| panic!("symbol: {error:?}"))
        .0
}

fn string(ctx: &mut ThreadContext, runtime: &Runtime, value: &str) -> Word {
    make_string(ctx, runtime, &value.chars().collect::<Vec<_>>())
        .unwrap_or_else(|error| panic!("string: {error:?}"))
}

#[test]
fn condition_arguments_cover_existing_string_and_symbol_designators() {
    let (runtime, mut ctx) = setup();
    let simple_condition = condition_class(&mut ctx, &runtime, "SIMPLE-CONDITION").unwrap();
    let existing = make_condition(&mut ctx, &runtime, simple_condition, &[])
        .unwrap_or_else(|error| panic!("condition: {error:?}"));
    assert_eq!(
        condition_argument(&mut ctx, &runtime, existing, "SIMPLE-CONDITION", &[]),
        Ok(existing)
    );

    let text = string(&mut ctx, &runtime, "hello ~a");
    let argument = Word::fixnum(8);
    let made = condition_argument(&mut ctx, &runtime, text, "SIMPLE-CONDITION", &[argument])
        .unwrap_or_else(|error| panic!("string condition: {error:?}"));
    assert_eq!(condition_class_of(&ctx, made), Ok(simple_condition));

    let designator = symbol(&mut ctx, &runtime, "COMMON-LISP", "SIMPLE-CONDITION");
    let from_symbol = condition_argument(&mut ctx, &runtime, designator, "SIMPLE-CONDITION", &[])
        .unwrap_or_else(|error| panic!("symbol condition: {error:?}"));
    assert_eq!(condition_class_of(&ctx, from_symbol), Ok(simple_condition));

    let unknown = symbol(&mut ctx, &runtime, "COMMON-LISP", "NO-SUCH-CONDITION");
    assert_eq!(
        condition_argument(&mut ctx, &runtime, unknown, "SIMPLE-CONDITION", &[]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn condition_helpers_and_builtin_error_paths_are_asserted() {
    let (runtime, mut ctx) = setup();
    let values = [Word::fixnum(1), Word::fixnum(2), Word::fixnum(3)];
    let list = argument_list(&mut ctx, &runtime, &values)
        .unwrap_or_else(|error| panic!("list: {error:?}"));
    assert_eq!(car(&ctx, list), Ok(values[0]));
    let tail = cdr(&ctx, list).unwrap_or(Word::NIL);
    assert_eq!(car(&ctx, tail), Ok(values[1]));
    assert_eq!(
        rest_arguments(&BuiltinArgs::new(&values), 1),
        Ok(values[1..].to_vec())
    );

    assert_eq!(
        condition_object_error(crate::ConditionError::Unhandled),
        ObjectError::Layout
    );
    assert_eq!(
        condition_object_error(crate::ConditionError::NotACondition),
        ObjectError::Layout
    );
    assert_eq!(
        condition_object_error(crate::ConditionError::RestartNotFound),
        ObjectError::Layout
    );
    assert_eq!(
        condition_object_error(crate::ConditionError::ChainCorrupt),
        ObjectError::Layout
    );
    assert_eq!(
        condition_object_error(crate::ConditionError::Object(ObjectError::TypeError)),
        ObjectError::TypeError
    );

    let text = string(&mut ctx, &runtime, "unhandled");
    assert_eq!(
        signal_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[text]),
            &mut MultipleValues::new()
        ),
        Ok(Word::NIL)
    );
    assert_eq!(
        warn_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[text]),
            &mut MultipleValues::new()
        ),
        Ok(Word::NIL)
    );
    assert_eq!(
        error_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[text]),
            &mut MultipleValues::new()
        ),
        Err(ObjectError::Unsupported)
    );
    assert_eq!(
        cerror_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[Word::NIL, text]),
            &mut MultipleValues::new(),
        ),
        Ok(Word::NIL)
    );
}

#[test]
fn condition_slot_and_registration_helpers_cover_success_and_invalid_inputs() {
    let (runtime, mut ctx) = setup();
    let class = make_simple_vector(&mut ctx, &runtime, &[Word::NIL]).unwrap();
    let instance = make_instance(&mut ctx, &runtime, class, &[Word::fixnum(11)]).unwrap();
    assert_eq!(
        cell_error_name_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[instance.as_word()]),
            &mut MultipleValues::new(),
        ),
        Ok(Word::fixnum(11))
    );

    let empty = symbol(&mut ctx, &runtime, "NCL-EXT", "EMPTY-ROW");
    let row = SymbolRow {
        package: "NCL-EXT",
        name: "EMPTY-ROW",
        kind: SymbolKind::Other,
    };
    register_symbol(&runtime, &mut ctx, &row).unwrap();
    assert_eq!(symbol_text(&ctx, empty).unwrap(), "EMPTY-ROW");
}
