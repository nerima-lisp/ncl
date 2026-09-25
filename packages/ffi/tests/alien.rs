#![allow(
    clippy::panic,
    clippy::unwrap_used,
    reason = "tests assert on concrete values"
)]

//! Alien type parsing, layout, and marshalling.

use ncl_ffi::{
    AlienRoutine, AlienType, FfiError, align_of, marshal_argument, offset_of, parse_type_name,
    parse_type_specifier, record_size, size_of, union_size, unmarshal_result,
};
use ncl_object::{DoubleFloat, Runtime, ThreadContext, Word, double_value, make_double};

/// Declare a runtime and a registered context as test locals, in that order so
/// the context drops before the runtime that owns its heap.
macro_rules! fixture {
    ($runtime:ident, $ctx:ident) => {
        let $runtime = Runtime::new().unwrap();
        let mut $ctx = ThreadContext::new();
        $ctx.register(&$runtime).unwrap();
    };
}

#[test]
fn atomic_type_names_parse_regardless_of_separator_or_case() {
    assert_eq!(parse_type_name("int").unwrap(), AlienType::Int);
    assert_eq!(
        parse_type_name("unsigned-long").unwrap(),
        AlienType::UnsignedLong
    );
    assert_eq!(
        parse_type_name("unsigned char").unwrap(),
        AlienType::UnsignedChar
    );
    assert_eq!(parse_type_name("SIZE_T").unwrap(), AlienType::SizeT);
    assert_eq!(parse_type_name("c-string").unwrap(), AlienType::CString);
    assert_eq!(
        parse_type_name("SAP").unwrap(),
        AlienType::SystemAreaPointer
    );
    assert_eq!(parse_type_name("double").unwrap(), AlienType::DoubleFloat);
}

#[test]
fn unknown_type_name_is_rejected() {
    let error = parse_type_name("quadruple").unwrap_err();
    assert_eq!(error, FfiError::UnknownAlienType("quadruple".to_owned()));
}

#[test]
fn compound_specifiers_parse_into_nested_types() {
    assert_eq!(
        parse_type_specifier("(array int 4)").unwrap(),
        AlienType::array(AlienType::Int, 4)
    );
    assert_eq!(
        parse_type_specifier("(pointer double)").unwrap(),
        AlienType::pointer(AlienType::DoubleFloat)
    );
    assert_eq!(
        parse_type_specifier("(struct point (x int) (y int))").unwrap(),
        AlienType::structure(
            "point",
            vec![
                ("x".to_owned(), AlienType::Int),
                ("y".to_owned(), AlienType::Int),
            ]
        )
    );
    assert_eq!(
        parse_type_specifier("(enum color (:red 0) (:green 1))").unwrap(),
        AlienType::enumeration(
            "color",
            vec![(":red".to_owned(), 0), (":green".to_owned(), 1)]
        )
    );
    assert_eq!(
        parse_type_specifier("(function int (int c-string))").unwrap(),
        AlienType::function(
            "",
            vec![AlienType::Int, AlienType::CString],
            AlienType::Int,
            false
        )
    );
}

#[test]
fn malformed_specifier_is_rejected() {
    assert!(parse_type_specifier("(array int)").is_err());
    assert!(parse_type_specifier("(bogus int)").is_err());
    assert!(parse_type_specifier("(pointer int) extra").is_err());
}

#[test]
fn scalar_sizes_and_alignments_follow_lp64() {
    assert_eq!(
        (size_of(&AlienType::Char), align_of(&AlienType::Char)),
        (1, 1)
    );
    assert_eq!(
        (size_of(&AlienType::Short), align_of(&AlienType::Short)),
        (2, 2)
    );
    assert_eq!(
        (size_of(&AlienType::Int), align_of(&AlienType::Int)),
        (4, 4)
    );
    assert_eq!(
        (size_of(&AlienType::Long), align_of(&AlienType::Long)),
        (8, 8)
    );
    assert_eq!(
        (
            size_of(&AlienType::DoubleFloat),
            align_of(&AlienType::DoubleFloat)
        ),
        (8, 8)
    );
    assert_eq!(
        (
            size_of(&AlienType::pointer(AlienType::Int)),
            align_of(&AlienType::pointer(AlienType::Int))
        ),
        (8, 8)
    );
}

#[test]
fn structure_layout_inserts_padding() {
    let mixed = AlienType::structure(
        "mixed",
        vec![
            ("c".to_owned(), AlienType::Char),
            ("i".to_owned(), AlienType::Int),
        ],
    );
    let AlienType::Structure(record) = &mixed else {
        panic!("expected a structure");
    };
    assert_eq!(size_of(&mixed), 8);
    assert_eq!(align_of(&mixed), 4);
    assert_eq!(record_size(record), 8);
    assert_eq!(offset_of(record, 0), Some(0));
    assert_eq!(offset_of(record, 1), Some(4));
    assert_eq!(offset_of(record, 2), None);
}

#[test]
fn union_size_is_the_largest_member_padded_to_alignment() {
    let wide = AlienType::union(
        "wide",
        vec![
            ("i".to_owned(), AlienType::Int),
            ("d".to_owned(), AlienType::DoubleFloat),
        ],
    );
    assert_eq!(size_of(&wide), 8);
    assert_eq!(align_of(&wide), 8);
    let AlienType::Union(record) = &wide else {
        panic!("expected a union");
    };
    assert_eq!(union_size(record), 8);

    let narrow = AlienType::union(
        "narrow",
        vec![
            ("i".to_owned(), AlienType::Int),
            ("c".to_owned(), AlienType::Char),
        ],
    );
    assert_eq!(size_of(&narrow), 4);
    assert_eq!(align_of(&narrow), 4);
}

#[test]
fn array_size_is_element_size_times_length() {
    let array = AlienType::array(AlienType::Int, 4);
    assert_eq!(size_of(&array), 16);
    assert_eq!(align_of(&array), 4);
}

#[test]
fn integers_marshal_little_endian() {
    fixture!(runtime, ctx);
    assert_eq!(
        marshal_argument(&ctx, &AlienType::Int, Word::fixnum(42)).unwrap(),
        vec![42, 0, 0, 0]
    );
    assert_eq!(
        marshal_argument(&ctx, &AlienType::Short, Word::fixnum(-1)).unwrap(),
        vec![0xff, 0xff]
    );
    assert_eq!(
        marshal_argument(&ctx, &AlienType::UnsignedInt, Word::fixnum(4_294_967_295)).unwrap(),
        vec![0xff, 0xff, 0xff, 0xff]
    );
}

#[test]
fn out_of_range_integer_is_rejected() {
    fixture!(runtime, ctx);
    let error = marshal_argument(&ctx, &AlienType::Short, Word::fixnum(40_000)).unwrap_err();
    assert_eq!(error, FfiError::ValueOutOfRange { type_name: "short" });
}

#[test]
fn boolean_and_character_marshal() {
    fixture!(runtime, ctx);
    assert_eq!(
        marshal_argument(&ctx, &AlienType::Boolean, Word::NIL).unwrap(),
        vec![0]
    );
    assert_eq!(
        marshal_argument(&ctx, &AlienType::Boolean, Word::TRUE).unwrap(),
        vec![1]
    );
    assert_eq!(
        marshal_argument(&ctx, &AlienType::Char, Word::character(65)).unwrap(),
        vec![65]
    );
    let error = marshal_argument(&ctx, &AlienType::Char, Word::character(0x1_0000)).unwrap_err();
    assert_eq!(error, FfiError::ValueOutOfRange { type_name: "char" });
}

#[test]
fn a_fixnum_is_not_a_character() {
    fixture!(runtime, ctx);
    let error = marshal_argument(&ctx, &AlienType::Char, Word::fixnum(65)).unwrap_err();
    assert_eq!(error, FfiError::TypeMismatch { type_name: "char" });
}

#[test]
fn integer_round_trips_through_bytes() {
    fixture!(runtime, ctx);
    let bytes = marshal_argument(&ctx, &AlienType::Int, Word::fixnum(-7)).unwrap();
    let value = unmarshal_result(&mut ctx, &runtime, &AlienType::Int, &bytes).unwrap();
    assert_eq!(value.as_fixnum(), Some(-7));
}

#[test]
fn double_round_trips_through_bytes() {
    fixture!(runtime, ctx);
    let original = make_double(&mut ctx, &runtime, 1.5).unwrap().as_word();
    let bytes = marshal_argument(&ctx, &AlienType::DoubleFloat, original).unwrap();
    let value = unmarshal_result(&mut ctx, &runtime, &AlienType::DoubleFloat, &bytes).unwrap();
    assert_eq!(
        double_value(&ctx, DoubleFloat::from(value))
            .unwrap()
            .to_bits(),
        1.5_f64.to_bits()
    );
}

#[test]
fn pointer_marshals_from_the_null_value() {
    fixture!(runtime, ctx);
    assert_eq!(
        marshal_argument(&ctx, &AlienType::CString, Word::NIL).unwrap(),
        vec![0; 8]
    );
    assert_eq!(
        marshal_argument(&ctx, &AlienType::SystemAreaPointer, Word::fixnum(0x40)).unwrap(),
        vec![0x40, 0, 0, 0, 0, 0, 0, 0]
    );
}

#[test]
fn void_result_unmarshals_to_nil() {
    fixture!(runtime, ctx);
    let value = unmarshal_result(&mut ctx, &runtime, &AlienType::Void, &[]).unwrap();
    assert_eq!(value, Word::NIL);
}

#[test]
fn composite_marshalling_needs_a_sys_memory_primitive() {
    fixture!(runtime, ctx);
    let structure = AlienType::structure("point", vec![("x".to_owned(), AlienType::Int)]);
    let error = marshal_argument(&ctx, &structure, Word::NIL).unwrap_err();
    assert!(matches!(error, FfiError::MissingSysPrimitive(_)));
    let error = unmarshal_result(&mut ctx, &runtime, &structure, &[0, 0, 0, 0]).unwrap_err();
    assert!(matches!(error, FfiError::MissingSysPrimitive(_)));
}

#[test]
fn long_float_has_no_phase_one_representation() {
    fixture!(runtime, ctx);
    let error = marshal_argument(&ctx, &AlienType::LongFloat, Word::NIL).unwrap_err();
    assert_eq!(error, FfiError::UnsupportedType("long-float"));
    let error = unmarshal_result(&mut ctx, &runtime, &AlienType::LongFloat, &[0; 16]).unwrap_err();
    assert_eq!(error, FfiError::UnsupportedType("long-float"));
}

#[test]
fn result_width_must_match_the_type() {
    fixture!(runtime, ctx);
    let error = unmarshal_result(&mut ctx, &runtime, &AlienType::Int, &[0, 0]).unwrap_err();
    assert_eq!(error, FfiError::ValueOutOfRange { type_name: "int" });
}

#[test]
fn routine_declaration_carries_its_signature() {
    let routine = AlienRoutine::new("strlen", vec![AlienType::CString], AlienType::SizeT, false);
    assert_eq!(routine.arguments().len(), 1);
    assert_eq!(routine.result(), &AlienType::SizeT);
}
