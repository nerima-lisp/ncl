#![allow(clippy::unwrap_used, reason = "record tests assert on codec values")]
use super::{Record, Ref, get_record, get_ref, get_refs, put_record, put_ref, put_refs};
use crate::error::ImageError;
use crate::format::Reader;

fn round_trip(record: &Record) {
    let mut bytes = Vec::new();
    put_record(&mut bytes, record).unwrap();
    let decoded = get_record(&mut Reader::new(&bytes)).unwrap();
    assert_eq!(&decoded, record);
}

#[test]
fn record_codec_round_trips_every_record_kind_and_reference_form() {
    let immediate = Ref::Immediate(0x0123_4567_89ab_cdef);
    let object = Ref::Object(42);
    round_trip(&Record::Cons {
        car: immediate,
        cdr: object,
    });
    round_trip(&Record::Symbol {
        package: "NCL-IMAGE".to_owned(),
        name: "CODEC".to_owned(),
        flags: 0xa5a5,
        value: immediate,
        function: object,
        plist: immediate,
    });
    round_trip(&Record::Package {
        name: "NCL-IMAGE".to_owned(),
        nicknames: vec!["IMAGE".to_owned(), "IMG".to_owned()],
    });
    round_trip(&Record::String("record codec".to_owned()));
    round_trip(&Record::Vector(vec![immediate, object]));
    round_trip(&Record::SpecializedArray {
        element_type: 7,
        elements: vec![1, 0xfeed_beef],
    });
    round_trip(&Record::HashTable {
        test: 2,
        weakness: 1,
        entries: vec![(immediate, object)],
    });
    round_trip(&Record::Structure {
        slots: vec![object, immediate],
    });
    round_trip(&Record::Instance {
        class: object,
        slots: vec![immediate],
    });
    round_trip(&Record::Function {
        closure: true,
        entry: 0x1234,
        name: immediate,
        lambda_list: object,
        code: immediate,
        captures: vec![object, immediate],
    });
    round_trip(&Record::CodeObject {
        entry: 0x1000,
        size: 19,
        constants: immediate,
        stack_map: object,
        debug: immediate,
    });
    round_trip(&Record::Bignum {
        negative: true,
        limbs: vec![0, u32::MAX],
    });
    round_trip(&Record::Ratio {
        numerator: immediate,
        denominator: object,
    });
    round_trip(&Record::DoubleFloat {
        bits: 0x3ff8_0000_0000_0000,
    });
    round_trip(&Record::Complex {
        real: immediate,
        imag: object,
    });

    let mut refs = Vec::new();
    put_refs(&mut refs, &[immediate, object]).unwrap();
    assert_eq!(
        get_refs(&mut Reader::new(&refs)).unwrap(),
        vec![immediate, object]
    );
}

#[test]
fn reference_and_record_unknown_tags_report_exact_errors() {
    let reference_error = get_ref(&mut Reader::new(&[9])).unwrap_err();
    assert_eq!(
        reference_error,
        ImageError::UnknownTag {
            space: "reference",
            tag: 9
        }
    );
    assert_eq!(reference_error.to_string(), "unknown reference tag 9");

    let record_error = get_record(&mut Reader::new(&[99])).unwrap_err();
    assert_eq!(
        record_error,
        ImageError::UnknownTag {
            space: "object kind",
            tag: 99
        }
    );
    assert_eq!(record_error.to_string(), "unknown object kind tag 99");
}

#[test]
fn reference_list_reader_preserves_truncation_details() {
    let mut bytes = 2_u32.to_le_bytes().to_vec();
    put_ref(&mut bytes, Ref::Immediate(7));
    let error = get_refs(&mut Reader::new(&bytes)).unwrap_err();
    assert_eq!(
        error,
        ImageError::Truncated {
            offset: 13,
            needed: 1
        }
    );
    assert_eq!(error.to_string(), "image truncated at 13, needed 1 bytes");
}
