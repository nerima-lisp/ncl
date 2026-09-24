#![allow(clippy::unwrap_used, reason = "tests assert on concrete values")]

//! System area pointer arithmetic and comparisons.

use ncl_ffi::{FfiError, SystemAreaPointer, sap_eq, sap_ge, sap_gt, sap_le, sap_lt};
use ncl_object::Word;

#[test]
fn null_pointer_is_recognized() {
    assert!(SystemAreaPointer::null().is_null());
    assert!(!SystemAreaPointer::new(1).is_null());
}

#[test]
fn offsets_add_and_subtract_bytes() {
    let base = SystemAreaPointer::new(0x1000);
    assert_eq!(base.sap_plus(16).address(), 0x1010);
    assert_eq!(base.sap_minus(16).address(), 0x0ff0);
    assert_eq!(base.sap_plus(16).sap_minus(16), base);
}

#[test]
fn difference_is_signed() {
    let high = SystemAreaPointer::new(0x1020);
    let low = SystemAreaPointer::new(0x1000);
    assert_eq!(high.sap_difference(low), 32);
    assert_eq!(low.sap_difference(high), -32);
}

#[test]
fn sap_int_and_from_word_round_trip() {
    let sap = SystemAreaPointer::new(0x1234);
    assert_eq!(sap.sap_int().as_fixnum(), Some(0x1234));
    assert_eq!(SystemAreaPointer::from_word(sap.as_word()).unwrap(), sap);
}

#[test]
fn int_sap_rejects_negative_and_non_integer() {
    assert!(matches!(
        SystemAreaPointer::from_word(Word::fixnum(-1)),
        Err(FfiError::ValueOutOfRange { .. })
    ));
    assert!(matches!(
        SystemAreaPointer::from_word(Word::NIL),
        Err(FfiError::TypeMismatch { .. })
    ));
}

#[test]
fn comparisons_follow_address_order() {
    let low = SystemAreaPointer::new(0x10);
    let high = SystemAreaPointer::new(0x20);
    assert!(sap_eq(low, SystemAreaPointer::new(0x10)));
    assert!(sap_lt(low, high));
    assert!(sap_le(low, low));
    assert!(sap_gt(high, low));
    assert!(sap_ge(high, high));
    assert!(!sap_lt(high, low));
}
