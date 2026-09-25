#![allow(missing_docs)]

use ncl_object::{
    ArithmeticError, CellError, Character, ControlError, FileError, Fixnum, LispError, ObjectError,
    CodeObject, Cons, ObjectErrorKind, ObjectRef, ObjectType, Package, PackageError,
    ProgramError, StreamError, TypeError, Word, WordView, classify,
};
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
fn heap_views_use_named_word_constructors() {
    let word = Word::fixnum(7);

    assert_eq!(Cons::from_word(word).as_word(), word);
    assert_eq!(Package::from_word(word).as_word(), word);
    assert_eq!(CodeObject::from_word(word).as_word(), word);
}

#[test]
fn typed_conversion_reports_datum_and_expected_type() {
    let Err(error) = WordView::try_from_word(Word::NIL, ObjectType::Character) else {
        panic!("NIL is not a character");
    };
    assert_eq!(error.datum, Word::NIL);
    assert_eq!(error.expected, ObjectType::Character);
}

#[test]
fn fixnum_and_character_views_validate_and_round_trip() {
    let fixnum =
        Fixnum::try_from_word(Word::fixnum(-12)).unwrap_or_else(|error| panic!("{error:?}"));
    assert_eq!(fixnum.value(), -12);
    assert_eq!(fixnum.as_word(), Word::fixnum(-12));
    assert_eq!(
        Fixnum::try_from_word(Word::character(65)),
        Err(TypeError {
            datum: Word::character(65),
            expected: ObjectType::Fixnum,
        })
    );

    let character = Character::try_from_word(Word::character(0x03bb))
        .unwrap_or_else(|error| panic!("{error:?}"));
    assert_eq!(character.value(), 0x03bb);
    assert_eq!(character.as_word(), Word::character(0x03bb));
    assert_eq!(
        Character::try_from_word(Word::fixnum(65)),
        Err(TypeError {
            datum: Word::fixnum(65),
            expected: ObjectType::Character,
        })
    );
}

#[test]
fn word_view_union_preserves_fixnum_character_and_immediate_cases() {
    let cases = [
        (Word::fixnum(3), WordView::Fixnum(3)),
        (Word::character(65), WordView::Character(65)),
        (Word::TRUE, WordView::Immediate(Word::TRUE)),
    ];
    for (word, expected) in cases {
        assert_eq!(
            WordView::try_from_word(word, ObjectType::Fixnum),
            if matches!(expected, WordView::Fixnum(_)) {
                Ok(expected)
            } else {
                Err(TypeError {
                    datum: word,
                    expected: ObjectType::Fixnum,
                })
            }
        );
        assert_eq!(WordView::from(classify(word)), expected);
        assert_eq!(expected.as_word(), word);
    }
}

#[test]
fn lisp_error_variants_preserve_their_payloads() {
    let errors = [
        LispError::TypeError {
            datum: Word::NIL,
            expected: ObjectType::Character,
        },
        LispError::ProgramError(ProgramError::UnknownKeyword),
        LispError::ArithmeticError(ArithmeticError::DivisionByZero),
        LispError::ControlError(ControlError::Throw),
        LispError::CellError(CellError::UnboundVariable),
        LispError::PackageError(PackageError::NotFound),
        LispError::StreamError(StreamError::Closed),
        LispError::EndOfFile,
        LispError::FileError(FileError::NotFound),
        LispError::Object(ObjectError::Unbound),
    ];
    assert_eq!(errors.len(), 10);
    assert_eq!(
        LispError::from(TypeError {
            datum: Word::NIL,
            expected: ObjectType::Character
        }),
        errors[0]
    );
    assert_eq!(LispError::from(ObjectError::Unbound), errors[9]);
}
