#![allow(missing_docs)]

use ncl_object::{ObjectError, ObjectErrorKind, ObjectRef, Word, WordView, classify};
use ncl_sys::StorageCondition;

#[test]
fn word_view_round_trips_existing_object_ref_values() {
    for reference in [
        ObjectRef::Fixnum(7),
        ObjectRef::Character(65),
        ObjectRef::Immediate(Word::TRUE),
    ] {
        let view = WordView::from(reference);
        assert_eq!(
            view.as_word(),
            match reference {
                ObjectRef::Fixnum(value) => Word::fixnum(value),
                ObjectRef::Character(value) => Word::character(value),
                ObjectRef::Immediate(value) => value,
                _ => unreachable!(),
            }
        );
    }
}

#[test]
fn error_kind_preserves_existing_object_error_values() {
    assert_eq!(ObjectError::TypeError.kind(), ObjectErrorKind::Type);
    assert_eq!(
        ObjectError::Storage(StorageCondition::ThreadNotRegistered).kind(),
        ObjectErrorKind::Storage(StorageCondition::ThreadNotRegistered)
    );
}

#[test]
fn classify_can_be_adapted_without_changing_the_abi() {
    let reference = classify(Word::fixnum(3));
    assert_eq!(WordView::from(reference).as_word(), Word::fixnum(3));
}

#[test]
fn typed_conversion_reports_datum_and_expected_type() {
    let Err(error) = WordView::try_from_word(Word::NIL, ncl_object::ObjectType::Character) else {
        panic!("NIL is not a character");
    };
    assert_eq!(error.datum, Word::NIL);
    assert_eq!(error.expected, ncl_object::ObjectType::Character);
}
