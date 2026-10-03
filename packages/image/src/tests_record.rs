#![allow(
    clippy::expect_used,
    clippy::needless_pass_by_value,
    clippy::unwrap_used,
    reason = "tests assert on codec failures"
)]

use super::{Record, Ref, get_record, get_ref, get_refs, put_record, put_ref, put_refs};
use crate::code::CodeImage;
use crate::error::ImageError;
use crate::format::Reader;

fn round_trip(record: Record) {
    let mut bytes = Vec::new();
    put_record(&mut bytes, &record).unwrap();
    assert_eq!(get_record(&mut Reader::new(&bytes)).unwrap(), record);
}

#[test]
fn every_record_variant_round_trips() {
    let immediate = Ref::Immediate(17);
    let object = Ref::Object(3);
    let records = [
        Record::Cons {
            car: immediate,
            cdr: object,
        },
        Record::Symbol {
            package: "NCL".into(),
            name: "X".into(),
            flags: 5,
            value: immediate,
            function: object,
            plist: immediate,
        },
        Record::Package {
            name: "NCL".into(),
            nicknames: vec!["N".into()],
        },
        Record::String("text".into()),
        Record::Vector(vec![immediate, object]),
        Record::SpecializedArray {
            element_type: 2,
            elements: vec![11, 12],
        },
        Record::HashTable {
            test: 1,
            weakness: 2,
            entries: vec![(immediate, object)],
        },
        Record::Structure {
            slots: vec![immediate],
        },
        Record::Instance {
            class: object,
            slots: vec![immediate],
        },
        Record::Function {
            closure: true,
            entry: 9,
            name: immediate,
            lambda_list: object,
            code: immediate,
            captures: vec![object],
        },
        Record::CodeObject {
            entry: 10,
            size: 11,
            constants: immediate,
            stack_map: object,
            debug: immediate,
        },
        Record::Bignum {
            negative: true,
            limbs: vec![1, u32::MAX],
        },
        Record::Ratio {
            numerator: immediate,
            denominator: object,
        },
        Record::DoubleFloat {
            bits: 1.5_f64.to_bits(),
        },
        Record::Complex {
            real: immediate,
            imag: object,
        },
    ];
    for record in records {
        round_trip(record);
    }
}

#[test]
fn references_and_code_blobs_round_trip() {
    let references = [Ref::Immediate(4), Ref::Object(8)];
    let mut bytes = Vec::new();
    put_refs(&mut bytes, &references).unwrap();
    assert_eq!(get_refs(&mut Reader::new(&bytes)).unwrap(), references);

    let code = CodeImage::from_raw(vec![1, 2], 1, 3, "fn".into()).unwrap();
    let mut bytes = Vec::new();
    super::put_code(&mut bytes, &code).unwrap();
    assert_eq!(super::get_code(&mut Reader::new(&bytes)).unwrap(), code);

    let mut bytes = Vec::new();
    put_ref(&mut bytes, Ref::Immediate(99));
    put_ref(&mut bytes, Ref::Object(7));
    assert_eq!(
        get_ref(&mut Reader::new(&bytes)).unwrap(),
        Ref::Immediate(99)
    );
    assert_eq!(
        get_ref(&mut Reader::new(&bytes[9..])).unwrap(),
        Ref::Object(7)
    );
}

#[test]
fn unknown_record_and_reference_tags_are_rejected() {
    assert_eq!(
        get_record(&mut Reader::new(&[255])).unwrap_err(),
        ImageError::UnknownTag {
            space: "object kind",
            tag: 255
        }
    );
    assert_eq!(
        get_ref(&mut Reader::new(&[255])).unwrap_err(),
        ImageError::UnknownTag {
            space: "reference",
            tag: 255
        }
    );
}
