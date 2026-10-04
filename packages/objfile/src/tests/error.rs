use super::*;
use crate::RelocKind;

#[test]
fn object_error_display_reports_each_variant() {
    let errors = [
        ObjectError::Truncated {
            offset: 3,
            needed: 4,
        },
        ObjectError::InvalidField {
            field: "magic",
            value: 9,
        },
        ObjectError::OutOfBounds {
            section: "code",
            offset: 8,
            size: 4,
        },
        ObjectError::Overlap {
            first: "one",
            second: "two",
        },
        ObjectError::InvalidReference {
            kind: "symbol",
            index: 2,
        },
        ObjectError::UnsupportedRelocation(RelocKind::CodeEntry),
        ObjectError::InvalidName,
        ObjectError::InvalidStructure("bad structure"),
    ];
    let messages: Vec<String> = errors.iter().map(ToString::to_string).collect();
    assert_eq!(messages[0], "truncated input at 3, need 4 bytes");
    assert_eq!(messages[1], "invalid magic: 9");
    assert_eq!(messages[2], "code range 8..12 is out of bounds");
    assert_eq!(messages[3], "sections one and two overlap");
    assert_eq!(messages[4], "invalid symbol reference 2");
    assert_eq!(messages[5], "unsupported relocation CodeEntry");
    assert_eq!(messages[6], "name cannot be represented");
    assert_eq!(messages[7], "bad structure");
}
