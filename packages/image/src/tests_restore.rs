#![allow(
    clippy::expect_used,
    clippy::needless_pass_by_value,
    clippy::unwrap_used,
    reason = "tests assert on restore failures"
)]

use super::{check_architecture, decode_test, decode_weakness, fix, fix_u64, resolve};
use crate::error::ImageError;
use crate::record::Ref;
use ncl_object::Word;

#[test]
fn references_and_numeric_fields_validate_boundaries() {
    let slots = [Word::fixnum(8)];
    assert_eq!(
        resolve(&slots, Ref::Immediate(9)).unwrap(),
        Word::from_bits(9)
    );
    assert_eq!(resolve(&slots, Ref::Object(0)).unwrap(), Word::fixnum(8));
    assert_eq!(
        resolve(&slots, Ref::Object(1)).unwrap_err(),
        ImageError::InvalidLayout {
            field: "object reference"
        }
    );
    assert_eq!(fix(12).unwrap(), Word::fixnum(12));
    assert_eq!(fix_u64(12).unwrap(), Word::fixnum(12));
    assert_eq!(
        fix_u64(u64::MAX).unwrap_err(),
        ImageError::InvalidLayout { field: "address" }
    );
}

#[test]
fn hash_table_tags_decode_exhaustively() {
    for (value, expected) in [
        (0, ncl_object::hash_table::HashTest::Eq),
        (1, ncl_object::hash_table::HashTest::Eql),
        (2, ncl_object::hash_table::HashTest::Equal),
        (3, ncl_object::hash_table::HashTest::Equalp),
    ] {
        assert_eq!(decode_test(value).unwrap(), expected);
    }
    assert_eq!(
        decode_test(4).unwrap_err(),
        ImageError::InvalidLayout { field: "hash test" }
    );
    for (value, expected) in [
        (0, ncl_object::hash_table::Weakness::None),
        (1, ncl_object::hash_table::Weakness::Key),
        (2, ncl_object::hash_table::Weakness::Value),
        (3, ncl_object::hash_table::Weakness::KeyAndValue),
        (4, ncl_object::hash_table::Weakness::KeyOrValue),
    ] {
        assert_eq!(decode_weakness(value).unwrap(), expected);
    }
    assert_eq!(
        decode_weakness(5).unwrap_err(),
        ImageError::InvalidLayout {
            field: "hash weakness"
        }
    );
}

#[test]
fn architecture_check_accepts_host_and_rejects_other_target() {
    let host = if cfg!(target_arch = "x86_64") {
        ncl_objfile::Architecture::X86_64
    } else {
        ncl_objfile::Architecture::Aarch64
    };
    assert_eq!(check_architecture(host), Ok(()));
    let other = if cfg!(target_arch = "x86_64") {
        ncl_objfile::Architecture::Aarch64
    } else {
        ncl_objfile::Architecture::X86_64
    };
    assert_eq!(
        check_architecture(other).unwrap_err(),
        ImageError::InvalidField {
            field: "architecture"
        }
    );
}
