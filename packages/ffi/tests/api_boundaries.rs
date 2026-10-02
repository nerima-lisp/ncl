#![allow(
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "coverage tests assert on concrete public API behavior"
)]

//! Behavior-focused tests for the public FFI data model and scalar boundary.

use std::error::Error;

use ncl_conditions::ConditionError;
use ncl_ffi::{
    AlienEnum, AlienRecord, AlienRoutine, AlienType, ArrayLength, CallableMode, FfiError,
    FieldOffset, SysPrimitive, SystemAreaPointer, alien_sap, align_of, allocate_system_memory,
    cast, deallocate_system_memory, field_offset, marshal_argument, memmove, offset_of,
    parse_type_name, parse_type_specifier, record_size, sap_ref, sap_set, signal_ffi_error,
    size_of, union_size, unmarshal_result,
};
use ncl_object::{
    DoubleFloat, ObjectError, Package, Runtime, ThreadContext, Word, double_value,
    make_bignum_from_i128, make_bignum_from_limbs, make_double, symbol_function, symbol_is_macro,
    symbol_is_special,
};

macro_rules! fixture {
    ($runtime:ident, $ctx:ident) => {
        let $runtime = Runtime::new().unwrap();
        let mut $ctx = ThreadContext::new();
        $ctx.register(&$runtime).unwrap();
    };
}

fn missing<T>(result: Result<T, FfiError>, expected: SysPrimitive) {
    match result {
        Err(error) => assert_eq!(error, FfiError::MissingSysPrimitive(expected)),
        Ok(_) => panic!("expected missing primitive error"),
    }
}

#[test]
fn alien_model_accessors_and_conversions_preserve_declarations() {
    let record = AlienRecord::new(
        "point",
        vec![
            ("x".to_owned(), AlienType::Int),
            ("name".to_owned(), AlienType::CString),
        ],
    );
    assert_eq!(record.name(), "point");
    assert_eq!(record.fields().len(), 2);
    assert_eq!(record.field(0), Some(("x", &AlienType::Int)));
    assert_eq!(record.field(2), None);
    assert_eq!(record.field_offset(1), Some(FieldOffset::new(8)));

    let enumeration = AlienEnum::new(
        "color",
        vec![(":red".to_owned(), -1), (":green".to_owned(), 2)],
    );
    assert_eq!(enumeration.name(), "color");
    assert_eq!(enumeration.variants().len(), 2);
    assert_eq!(enumeration.variant_value(":green"), Some(2));
    assert_eq!(enumeration.variant_value(":blue"), None);

    let array = AlienType::array(AlienType::Int, ArrayLength::new(3));
    assert_eq!(array.array_length(), Some(ArrayLength::new(3)));
    assert_eq!(AlienType::Int.array_length(), None);
    assert_eq!(usize::from(ArrayLength::new(3)), 3);
    assert_eq!(ArrayLength::from(4).get(), 4);
    assert_eq!(usize::from(FieldOffset::new(7)), 7);

    let function = AlienType::function(
        "sum",
        vec![AlienType::Int, AlienType::Int],
        AlienType::Int,
        CallableMode::Callable,
    );
    let AlienType::Function(routine) = &function else {
        panic!("expected function type");
    };
    assert_eq!(routine.name(), "sum");
    assert_eq!(routine.arguments(), &[AlienType::Int, AlienType::Int]);
    assert_eq!(routine.result(), &AlienType::Int);
    assert_eq!(routine.callable_mode(), CallableMode::Callable);
    assert!(routine.is_callable());
    assert!(bool::from(CallableMode::Callable));
    assert!(!bool::from(CallableMode::NotCallable));
    assert_eq!(CallableMode::from(true), CallableMode::Callable);
    assert_eq!(CallableMode::from(false), CallableMode::NotCallable);

    let declaration = AlienRoutine::new("decl", Vec::new(), AlienType::Void, false);
    assert_eq!(declaration.callable_mode(), CallableMode::NotCallable);
    assert!(!declaration.is_callable());
    let explicit = AlienType::function_with_mode(
        "decl",
        Vec::new(),
        AlienType::Void,
        CallableMode::NotCallable,
    );
    assert!(!matches!(explicit, AlienType::Function(r) if r.is_callable()));
}

#[test]
fn alien_labels_and_scalar_classification_cover_each_shape() {
    let scalar_types = [
        AlienType::Boolean,
        AlienType::Char,
        AlienType::UnsignedChar,
        AlienType::Short,
        AlienType::UnsignedShort,
        AlienType::Int,
        AlienType::UnsignedInt,
        AlienType::Long,
        AlienType::UnsignedLong,
        AlienType::LongLong,
        AlienType::UnsignedLongLong,
        AlienType::SizeT,
        AlienType::SSizeT,
        AlienType::SingleFloat,
        AlienType::DoubleFloat,
        AlienType::pointer(AlienType::Int),
        AlienType::CString,
        AlienType::Utf8String,
        AlienType::SystemAreaPointer,
        AlienType::enumeration("e", vec![]),
    ];
    assert!(scalar_types.iter().all(AlienType::is_scalar));
    assert_eq!(scalar_types[0].label(), "boolean");
    let expected_labels = [
        "boolean",
        "char",
        "unsigned-char",
        "short",
        "unsigned-short",
        "int",
        "unsigned-int",
        "long",
        "unsigned-long",
        "long-long",
        "unsigned-long-long",
        "size-t",
        "ssize-t",
        "single-float",
        "double-float",
        "pointer",
        "c-string",
        "utf8-string",
        "system-area-pointer",
        "enum",
    ];
    assert_eq!(
        scalar_types
            .iter()
            .map(AlienType::label)
            .collect::<Vec<_>>(),
        expected_labels
    );
    assert_eq!(AlienType::Void.label(), "void");
    assert_eq!(AlienType::LongFloat.label(), "long-float");
    assert_eq!(AlienType::array(AlienType::Int, 1).label(), "array");
    assert_eq!(AlienType::structure("s", Vec::new()).label(), "struct");
    assert_eq!(AlienType::union("u", Vec::new()).label(), "union");
    assert!(!AlienType::Void.is_scalar());
    assert!(!AlienType::LongFloat.is_scalar());
    assert!(!AlienType::array(AlienType::Int, 1).is_scalar());
    assert!(!AlienType::structure("s", Vec::new()).is_scalar());
    assert!(!AlienType::union("u", Vec::new()).is_scalar());
    assert!(!AlienType::function("f", Vec::new(), AlienType::Void, false).is_scalar());
    assert_eq!(
        AlienType::function("f", Vec::new(), AlienType::Void, false).label(),
        "function"
    );
}

#[test]
fn parser_accepts_all_atomic_aliases_and_reports_structural_errors() {
    let aliases = [
        ("void", AlienType::Void),
        ("boolean", AlienType::Boolean),
        ("bool", AlienType::Boolean),
        ("char", AlienType::Char),
        ("unsigned_char", AlienType::UnsignedChar),
        ("signed-short", AlienType::Short),
        ("short int", AlienType::Short),
        ("unsigned-short-int", AlienType::UnsignedShort),
        ("signed", AlienType::Int),
        ("signed-int", AlienType::Int),
        ("unsigned", AlienType::UnsignedInt),
        ("long-int", AlienType::Long),
        ("signed-long", AlienType::Long),
        ("unsigned-long-int", AlienType::UnsignedLong),
        ("long-long", AlienType::LongLong),
        ("signed-long-long", AlienType::LongLong),
        ("unsigned-long-long-int", AlienType::UnsignedLongLong),
        ("size_t", AlienType::SizeT),
        ("ssize-t", AlienType::SSizeT),
        ("float", AlienType::SingleFloat),
        ("single-float", AlienType::SingleFloat),
        ("double", AlienType::DoubleFloat),
        ("double-float", AlienType::DoubleFloat),
        ("long-float", AlienType::LongFloat),
        ("cstring", AlienType::CString),
        ("utf8-string", AlienType::Utf8String),
        ("sap", AlienType::SystemAreaPointer),
        ("system-area-pointer", AlienType::SystemAreaPointer),
    ];
    for (source, expected) in aliases {
        assert_eq!(parse_type_name(source), Ok(expected));
    }
    assert_eq!(parse_type_name(" __ INT-- "), Ok(AlienType::Int));
    assert_eq!(
        parse_type_name(" unknown "),
        Err(FfiError::UnknownAlienType("unknown".to_owned()))
    );

    assert_eq!(
        parse_type_specifier("(ARRAY (POINTER INT) 0)"),
        Err(FfiError::UnknownAlienType("ARRAY".to_owned()))
    );
    assert_eq!(
        parse_type_specifier(""),
        Err(FfiError::UnknownAlienType(String::new()))
    );
    assert!(parse_type_specifier("(array int nope)").is_err());
    assert!(parse_type_specifier("(array int 1").is_err());
    assert!(parse_type_specifier("(struct point (x int)").is_err());
    assert!(parse_type_specifier("(struct point (x)").is_err());
    assert!(parse_type_specifier("(enum color (:red nope))").is_err());
    assert!(parse_type_specifier("(enum color (:red 1)").is_err());
    assert!(parse_type_specifier("(function int ((x int))").is_err());
    assert!(parse_type_specifier("(function int (int (named c-string)))").is_ok());
    assert!(parse_type_specifier("(function int int)").is_err());

    assert_eq!(
        parse_type_specifier("(pointer unsigned-char)"),
        Ok(AlienType::pointer(AlienType::UnsignedChar))
    );
    assert_eq!(
        parse_type_specifier("(array short 2)"),
        Ok(AlienType::array(AlienType::Short, 2))
    );
    assert_eq!(
        parse_type_specifier("(union pair (first int))"),
        Ok(AlienType::union(
            "pair",
            vec![("first".to_owned(), AlienType::Int)]
        ))
    );
}

#[test]
fn parser_handles_empty_forms_and_rejects_missing_structural_tokens() {
    assert_eq!(
        parse_type_specifier("(union empty)"),
        Ok(AlienType::union("empty", Vec::new()))
    );
    assert_eq!(
        parse_type_specifier("(function int ())"),
        Ok(AlienType::function("", Vec::new(), AlienType::Int, false))
    );
    assert_eq!(
        parse_type_specifier("(enum signs (negative -1))"),
        Ok(AlienType::enumeration(
            "signs",
            vec![("negative".to_owned(), -1)]
        ))
    );

    for source in [
        "(",
        "(pointer)",
        "(struct",
        "(struct point (",
        "(struct point (x",
        "(union point (x int)",
        "(enum",
        "(enum color (",
        "(enum color (negative",
        "(function int (int",
        "(function int (",
    ] {
        assert!(
            parse_type_specifier(source).is_err(),
            "malformed type specifier was accepted: {source}"
        );
    }
}

#[test]
fn empty_and_nested_layouts_have_stable_size_alignment_and_offsets() {
    let empty = AlienRecord::new("empty", Vec::new());
    assert_eq!(record_size(&empty), 0);
    assert_eq!(align_of(&AlienType::Structure(empty.clone())), 1);
    assert_eq!(field_offset(&empty, 0), None);
    assert_eq!(offset_of(&empty, usize::MAX), None);
    assert_eq!(union_size(&empty), 0);

    let nested = AlienType::array(AlienType::pointer(AlienType::Short), 2);
    assert_eq!(size_of(&nested), 16);
    assert_eq!(align_of(&nested), 8);
    assert_eq!(size_of(&AlienType::LongFloat), 16);
    assert_eq!(align_of(&AlienType::LongFloat), 16);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "coverage test groups one marshalling contract"
)]
fn marshal_covers_boolean_character_integer_float_pointer_and_composite_edges() {
    fixture!(runtime, ctx);
    assert_eq!(
        marshal_argument(&ctx, &AlienType::Void, Word::NIL),
        Err(FfiError::UnsupportedType("void"))
    );
    assert_eq!(
        marshal_argument(&ctx, &AlienType::Boolean, Word::fixnum(0)),
        Ok(vec![0])
    );
    assert_eq!(
        marshal_argument(&ctx, &AlienType::Boolean, Word::fixnum(1)),
        Ok(vec![1])
    );
    assert_eq!(
        marshal_argument(&ctx, &AlienType::Boolean, Word::fixnum(2)),
        Err(FfiError::TypeMismatch {
            type_name: "boolean"
        })
    );
    assert_eq!(
        marshal_argument(&ctx, &AlienType::Boolean, Word::character(1)),
        Err(FfiError::TypeMismatch {
            type_name: "boolean"
        })
    );
    assert_eq!(
        marshal_argument(&ctx, &AlienType::Char, Word::character(255)),
        Ok(vec![255])
    );
    assert_eq!(
        marshal_argument(&ctx, &AlienType::UnsignedChar, Word::character(255)),
        Ok(vec![255])
    );
    assert_eq!(
        marshal_argument(&ctx, &AlienType::Char, Word::character(256)),
        Err(FfiError::ValueOutOfRange { type_name: "char" })
    );

    assert_eq!(
        marshal_argument(&ctx, &AlienType::Short, Word::fixnum(-32768)),
        Ok(vec![0, 128])
    );
    assert_eq!(
        marshal_argument(&ctx, &AlienType::UnsignedShort, Word::fixnum(65535)),
        Ok(vec![255, 255])
    );
    assert_eq!(
        marshal_argument(&ctx, &AlienType::Int, Word::fixnum(i64::from(i32::MIN))),
        Ok(vec![0, 0, 0, 128])
    );
    assert_eq!(
        marshal_argument(
            &ctx,
            &AlienType::UnsignedInt,
            Word::fixnum(i64::from(u32::MAX))
        ),
        Ok(vec![255, 255, 255, 255])
    );
    assert_eq!(
        marshal_argument(&ctx, &AlienType::Long, Word::fixnum(-1)),
        Ok(vec![255; 8])
    );
    assert_eq!(
        marshal_argument(&ctx, &AlienType::UnsignedLongLong, Word::fixnum(0)),
        Ok(vec![0; 8])
    );
    assert_eq!(
        marshal_argument(
            &ctx,
            &AlienType::enumeration("e", Vec::new()),
            Word::fixnum(7)
        ),
        Ok(vec![7, 0, 0, 0])
    );
    assert_eq!(
        marshal_argument(&ctx, &AlienType::Int, Word::character(1)),
        Err(FfiError::TypeMismatch {
            type_name: "integer"
        })
    );
    assert_eq!(
        marshal_argument(&ctx, &AlienType::Short, Word::TRUE),
        Err(FfiError::TypeMismatch {
            type_name: "integer"
        })
    );
    assert_eq!(
        marshal_argument(&ctx, &AlienType::UnsignedInt, Word::fixnum(-1)),
        Err(FfiError::ValueOutOfRange {
            type_name: "integer"
        })
    );
    assert_eq!(
        marshal_argument(&ctx, &AlienType::UnsignedShort, Word::fixnum(-1)),
        Err(FfiError::ValueOutOfRange {
            type_name: "integer"
        })
    );

    let big = make_bignum_from_i128(&mut ctx, &runtime, (1_i128 << 62) + 1)
        .unwrap()
        .as_word();
    assert_eq!(
        marshal_argument(&ctx, &AlienType::LongLong, big),
        Ok(vec![1, 0, 0, 0, 0, 0, 0, 64])
    );
    let negative_big = make_bignum_from_i128(&mut ctx, &runtime, -((1_i128 << 62) + 1))
        .unwrap()
        .as_word();
    assert_eq!(
        marshal_argument(&ctx, &AlienType::LongLong, negative_big),
        Ok(vec![255, 255, 255, 255, 255, 255, 255, 191])
    );
    let huge = make_bignum_from_limbs(&mut ctx, &runtime, false, &[0, 0, 0, 0, 0])
        .unwrap()
        .as_word();
    assert_eq!(
        marshal_argument(&ctx, &AlienType::Int, huge),
        Err(FfiError::ValueOutOfRange {
            type_name: "integer"
        })
    );
    let overflowing = make_bignum_from_limbs(&mut ctx, &runtime, false, &[1, 0, 0, 0, 1])
        .unwrap()
        .as_word();
    assert_eq!(
        marshal_argument(&ctx, &AlienType::Int, overflowing),
        Err(FfiError::ValueOutOfRange {
            type_name: "integer"
        })
    );

    let single = Word::from_bits((u64::from(1.5_f32.to_bits()) << 4) | 2);
    let double = make_double(&mut ctx, &runtime, 2.5).unwrap().as_word();
    assert_eq!(
        marshal_argument(&ctx, &AlienType::SingleFloat, single)
            .unwrap()
            .len(),
        4
    );
    assert_eq!(
        marshal_argument(&ctx, &AlienType::SingleFloat, double).unwrap(),
        2.5_f32.to_le_bytes()
    );
    assert_eq!(
        marshal_argument(&ctx, &AlienType::SingleFloat, Word::TRUE),
        Err(FfiError::TypeMismatch {
            type_name: "single-float"
        })
    );
    assert_eq!(
        marshal_argument(&ctx, &AlienType::DoubleFloat, single).unwrap(),
        f64::from(1.5_f32).to_le_bytes()
    );
    assert_eq!(
        marshal_argument(&ctx, &AlienType::DoubleFloat, double).unwrap(),
        2.5_f64.to_le_bytes()
    );
    assert_eq!(
        marshal_argument(&ctx, &AlienType::DoubleFloat, Word::TRUE),
        Err(FfiError::TypeMismatch {
            type_name: "double-float"
        })
    );

    for ty in [
        AlienType::pointer(AlienType::Int),
        AlienType::CString,
        AlienType::Utf8String,
        AlienType::SystemAreaPointer,
        AlienType::function("f", Vec::new(), AlienType::Void, false),
    ] {
        assert_eq!(marshal_argument(&ctx, &ty, Word::NIL), Ok(vec![0; 8]));
        assert_eq!(
            marshal_argument(&ctx, &ty, Word::fixnum(0x40)),
            Ok(vec![0x40, 0, 0, 0, 0, 0, 0, 0])
        );
        assert_eq!(
            marshal_argument(&ctx, &ty, Word::fixnum(-1)),
            Err(FfiError::ValueOutOfRange {
                type_name: "pointer"
            })
        );
        assert_eq!(
            marshal_argument(&ctx, &ty, Word::TRUE),
            Err(FfiError::TypeMismatch {
                type_name: "pointer"
            })
        );
    }
    for ty in [
        AlienType::array(AlienType::Int, 1),
        AlienType::structure("s", Vec::new()),
        AlienType::union("u", Vec::new()),
    ] {
        missing(
            marshal_argument(&ctx, &ty, Word::NIL),
            SysPrimitive::ReadSystemMemory,
        );
    }
    assert_eq!(
        marshal_argument(&ctx, &AlienType::LongFloat, Word::NIL),
        Err(FfiError::UnsupportedType("long-float"))
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "coverage test groups one unmarshalling contract"
)]
fn unmarshal_covers_scalar_results_bignums_pointers_and_errors() {
    fixture!(runtime, ctx);
    assert_eq!(
        unmarshal_result(&mut ctx, &runtime, &AlienType::Void, &[1, 2, 3]),
        Ok(Word::NIL)
    );
    assert_eq!(
        unmarshal_result(&mut ctx, &runtime, &AlienType::Boolean, &[0]),
        Ok(Word::NIL)
    );
    assert_eq!(
        unmarshal_result(&mut ctx, &runtime, &AlienType::Boolean, &[2]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        unmarshal_result(&mut ctx, &runtime, &AlienType::Char, &[255]),
        Ok(Word::character(255))
    );
    assert_eq!(
        unmarshal_result(&mut ctx, &runtime, &AlienType::UnsignedChar, &[65]),
        Ok(Word::character(65))
    );
    assert_eq!(
        unmarshal_result(&mut ctx, &runtime, &AlienType::Short, &[255, 255])
            .unwrap()
            .as_fixnum(),
        Some(-1)
    );
    assert_eq!(
        unmarshal_result(&mut ctx, &runtime, &AlienType::Int, &[255, 255, 255, 255])
            .unwrap()
            .as_fixnum(),
        Some(-1)
    );
    assert_eq!(
        unmarshal_result(
            &mut ctx,
            &runtime,
            &AlienType::enumeration("e", Vec::new()),
            &[2, 0, 0, 0]
        )
        .unwrap()
        .as_fixnum(),
        Some(2)
    );
    assert_eq!(
        unmarshal_result(&mut ctx, &runtime, &AlienType::UnsignedShort, &[255, 255])
            .unwrap()
            .as_fixnum(),
        Some(65535)
    );
    assert_eq!(
        unmarshal_result(
            &mut ctx,
            &runtime,
            &AlienType::UnsignedInt,
            &[255, 255, 255, 255]
        )
        .unwrap()
        .as_fixnum(),
        Some(4_294_967_295)
    );
    assert_eq!(
        unmarshal_result(&mut ctx, &runtime, &AlienType::Long, &[255; 8])
            .unwrap()
            .as_fixnum(),
        Some(-1)
    );
    assert_eq!(
        unmarshal_result(
            &mut ctx,
            &runtime,
            &AlienType::SSizeT,
            &[0, 0, 0, 0, 0, 0, 0, 128]
        )
        .unwrap()
        .as_fixnum(),
        None
    );
    let unsigned_big =
        unmarshal_result(&mut ctx, &runtime, &AlienType::UnsignedLongLong, &[255; 8]).unwrap();
    assert_eq!(
        marshal_argument(&ctx, &AlienType::UnsignedLongLong, unsigned_big).unwrap(),
        vec![255; 8]
    );
    let signed_big = unmarshal_result(
        &mut ctx,
        &runtime,
        &AlienType::LongLong,
        &[0, 0, 0, 0, 0, 0, 0, 64],
    )
    .unwrap();
    assert_eq!(
        marshal_argument(&ctx, &AlienType::LongLong, signed_big).unwrap(),
        vec![0, 0, 0, 0, 0, 0, 0, 64]
    );

    let single_bytes = 1.25_f32.to_le_bytes();
    let single_word =
        unmarshal_result(&mut ctx, &runtime, &AlienType::SingleFloat, &single_bytes).unwrap();
    assert_eq!(single_word.bits(), (u64::from(1.25_f32.to_bits()) << 4) | 2);
    let double_word = unmarshal_result(
        &mut ctx,
        &runtime,
        &AlienType::DoubleFloat,
        &2.5_f64.to_le_bytes(),
    )
    .unwrap();
    assert_eq!(
        double_value(&ctx, DoubleFloat::from_word(double_word))
            .unwrap()
            .to_bits(),
        2.5_f64.to_bits()
    );

    for ty in [
        AlienType::pointer(AlienType::Int),
        AlienType::CString,
        AlienType::Utf8String,
        AlienType::SystemAreaPointer,
        AlienType::function("f", Vec::new(), AlienType::Void, false),
    ] {
        assert_eq!(
            unmarshal_result(&mut ctx, &runtime, &ty, &[0; 8]),
            Ok(Word::NIL)
        );
        assert_eq!(
            unmarshal_result(&mut ctx, &runtime, &ty, &[0x40, 0, 0, 0, 0, 0, 0, 0])
                .unwrap()
                .as_fixnum(),
            Some(0x40)
        );
    }
    assert_eq!(
        unmarshal_result(&mut ctx, &runtime, &AlienType::LongFloat, &[0; 16]),
        Err(FfiError::UnsupportedType("long-float"))
    );
    assert_eq!(
        unmarshal_result(&mut ctx, &runtime, &AlienType::Int, &[0, 0]),
        Err(FfiError::ValueOutOfRange { type_name: "int" })
    );
    for ty in [
        AlienType::array(AlienType::Int, 1),
        AlienType::structure("s", Vec::new()),
        AlienType::union("u", Vec::new()),
    ] {
        missing(
            unmarshal_result(&mut ctx, &runtime, &ty, &vec![0; size_of(&ty)]),
            SysPrimitive::ReadSystemMemory,
        );
    }
}

#[test]
fn marshal_and_unmarshal_enforce_integer_float_and_pointer_boundaries() {
    fixture!(runtime, ctx);

    let too_wide_unsigned = make_bignum_from_limbs(&mut ctx, &runtime, false, &[0, 0, 1])
        .unwrap()
        .as_word();
    assert_eq!(
        marshal_argument(&ctx, &AlienType::UnsignedInt, too_wide_unsigned),
        Err(FfiError::ValueOutOfRange {
            type_name: "unsigned-int"
        })
    );
    let larger_than_i128 =
        make_bignum_from_limbs(&mut ctx, &runtime, false, &[1, 0, 0, 0x8000_0000])
            .unwrap()
            .as_word();
    assert_eq!(
        marshal_argument(&ctx, &AlienType::LongLong, larger_than_i128),
        Err(FfiError::ValueOutOfRange {
            type_name: "integer"
        })
    );
    let negative_larger_than_i128 =
        make_bignum_from_limbs(&mut ctx, &runtime, true, &[1, 0, 0, 0x8000_0000])
            .unwrap()
            .as_word();
    assert_eq!(
        marshal_argument(&ctx, &AlienType::LongLong, negative_larger_than_i128),
        Err(FfiError::ValueOutOfRange {
            type_name: "integer"
        })
    );
    let negative_for_unsigned = make_bignum_from_i128(&mut ctx, &runtime, -1)
        .unwrap()
        .as_word();
    assert_eq!(
        marshal_argument(&ctx, &AlienType::UnsignedLong, negative_for_unsigned),
        Err(FfiError::ValueOutOfRange {
            type_name: "integer"
        })
    );

    let max_fixnum = (1_i64 << 62) - 1;
    assert_eq!(
        marshal_argument(
            &ctx,
            &AlienType::pointer(AlienType::Int),
            Word::fixnum(max_fixnum),
        ),
        Ok(u64::try_from(max_fixnum).unwrap().to_le_bytes().to_vec())
    );
    assert_eq!(
        unmarshal_result(
            &mut ctx,
            &runtime,
            &AlienType::pointer(AlienType::Int),
            &u64::try_from(max_fixnum).unwrap().to_le_bytes(),
        )
        .unwrap()
        .as_fixnum(),
        Some(max_fixnum)
    );

    assert_eq!(
        unmarshal_result(&mut ctx, &runtime, &AlienType::SingleFloat, &[0; 3]),
        Err(FfiError::ValueOutOfRange {
            type_name: "single-float"
        })
    );
    assert_eq!(
        unmarshal_result(&mut ctx, &runtime, &AlienType::DoubleFloat, &[0; 4]),
        Err(FfiError::ValueOutOfRange {
            type_name: "double-float"
        })
    );
    assert_eq!(
        unmarshal_result(
            &mut ctx,
            &runtime,
            &AlienType::pointer(AlienType::Int),
            &[0; 4],
        ),
        Err(FfiError::ValueOutOfRange {
            type_name: "pointer"
        })
    );
}

#[test]
fn sys_primitive_table_exposes_every_exact_signature() {
    let expected = [
        SysPrimitive::DlopenSharedObject,
        SysPrimitive::DlcloseSharedObject,
        SysPrimitive::DlsymForeignSymbol,
        SysPrimitive::DlerrorMessage,
        SysPrimitive::CallForeignFunction,
        SysPrimitive::ReadSystemMemory,
        SysPrimitive::WriteSystemMemory,
        SysPrimitive::AllocateSystemMemory,
        SysPrimitive::DeallocateSystemMemory,
        SysPrimitive::MemmoveSystemMemory,
        SysPrimitive::PinObject,
        SysPrimitive::UnpinObject,
        SysPrimitive::ObjectAddress,
    ];
    assert_eq!(SysPrimitive::ALL, expected.as_slice());
    let signatures: Vec<_> = SysPrimitive::ALL
        .iter()
        .map(|primitive| primitive.signature())
        .collect();
    assert_eq!(signatures.len(), 13);
    assert!(
        signatures
            .iter()
            .all(|signature| signature.starts_with("ncl_sys::"))
    );
    assert!(signatures.windows(2).all(|pair| pair[0] != pair[1]));
    assert!(SysPrimitive::ALL.contains(&SysPrimitive::ObjectAddress));
}

#[test]
fn ffi_error_formats_sources_and_converts_layers() {
    let object = FfiError::Object(ncl_object::ObjectError::TypeError);
    assert_eq!(object.to_string(), "object error: TypeError");
    assert!(object.source().is_some());
    assert_eq!(
        object.into_object_error(),
        ncl_object::ObjectError::TypeError
    );

    let condition = FfiError::Condition(ConditionError::NotACondition);
    assert_eq!(condition.to_string(), "condition error: not a condition");
    assert!(condition.source().is_some());
    assert_eq!(
        condition.into_object_error(),
        ncl_object::ObjectError::Unsupported
    );
    assert_eq!(
        FfiError::from(ncl_object::ObjectError::Layout),
        FfiError::Object(ncl_object::ObjectError::Layout)
    );
    assert_eq!(
        FfiError::from(ConditionError::Unhandled),
        FfiError::Condition(ConditionError::Unhandled)
    );

    let errors = [
        (
            FfiError::UnknownAlienType("x".to_owned()),
            "unknown alien type: x",
        ),
        (
            FfiError::ValueOutOfRange { type_name: "int" },
            "value out of range for alien type int",
        ),
        (
            FfiError::TypeMismatch {
                type_name: "pointer",
            },
            "value does not match alien type pointer",
        ),
        (
            FfiError::ArityMismatch {
                expected: 2,
                got: 1,
            },
            "expected 2 arguments, got 1",
        ),
        (FfiError::NullPointer, "null system area pointer"),
        (
            FfiError::UnsupportedType("long-float"),
            "unsupported alien type: long-float",
        ),
        (
            FfiError::MissingSysPrimitive(SysPrimitive::DlerrorMessage),
            "missing ncl-sys primitive: ncl_sys::dlerror_message() -> Option<String>",
        ),
        (
            FfiError::MissingConditionClass("SIMPLE-ERROR"),
            "condition class not registered: SIMPLE-ERROR",
        ),
        (
            FfiError::RootStackCorrupt,
            "precise-root token popped out of stack order",
        ),
    ];
    for (error, message) in errors {
        assert_eq!(error.to_string(), message);
        assert!(error.source().is_none());
        assert_eq!(
            error.into_object_error(),
            ncl_object::ObjectError::Unsupported
        );
    }
}

#[test]
fn memory_and_pointer_wrappers_keep_their_declared_contracts() {
    fixture!(runtime, ctx);
    let address = SystemAreaPointer::new(0x1000);
    assert_eq!(alien_sap(Word::fixnum(0x1000)), Ok(address));
    assert_eq!(
        alien_sap(Word::NIL),
        Err(FfiError::TypeMismatch {
            type_name: "system-area-pointer"
        })
    );
    assert_eq!(cast(&AlienType::Int, address), address);
    missing(
        deallocate_system_memory(address, 0),
        SysPrimitive::DeallocateSystemMemory,
    );
    missing(
        memmove(address, address, 0),
        SysPrimitive::MemmoveSystemMemory,
    );
    missing(
        sap_ref(&mut ctx, &runtime, &AlienType::Int, address, -1),
        SysPrimitive::ReadSystemMemory,
    );
    missing(
        sap_set(&ctx, &runtime, &AlienType::Int, address, 1, Word::fixnum(1)),
        SysPrimitive::WriteSystemMemory,
    );
}

#[test]
fn allocator_boundary_reports_exact_missing_primitive() {
    missing(
        allocate_system_memory(0),
        SysPrimitive::AllocateSystemMemory,
    );
}

#[test]
fn condition_boundary_reports_missing_hierarchy_and_unhandled_error() {
    fixture!(runtime, ctx);
    assert_eq!(
        signal_ffi_error(&mut ctx, &runtime, "without registration"),
        Err(FfiError::MissingConditionClass("SIMPLE-ERROR"))
    );

    ncl_conditions::register(&runtime).unwrap();
    assert_eq!(
        signal_ffi_error(&mut ctx, &runtime, "unhandled"),
        Err(FfiError::Condition(ConditionError::Unhandled))
    );
}

#[test]
fn rooted_object_helper_propagates_closure_errors() {
    fixture!(runtime, ctx);
    let result: Result<(), FfiError> =
        ncl_ffi::with_rooted_objects(&mut ctx, &[Word::NIL, Word::TRUE], |_ctx, slots| {
            assert_eq!(slots.len(), 2);
            Err(FfiError::RootStackCorrupt)
        });
    assert_eq!(result, Err(FfiError::RootStackCorrupt));
}

#[test]
fn registration_installs_function_flags_and_alien_classes() {
    fixture!(runtime, ctx);
    ncl_ffi::register(&runtime).unwrap();
    let package = Package::from_word(runtime.find_package(&ctx, "NCL-FFI").unwrap());

    let (call_foreign, _) = package.intern(&mut ctx, &runtime, "CALL-FOREIGN").unwrap();
    assert_eq!(symbol_function(&ctx, call_foreign), Ok(Word::UNBOUND));

    let (macro_symbol, _) = package.intern(&mut ctx, &runtime, "ADDR").unwrap();
    assert_eq!(symbol_is_macro(&ctx, macro_symbol), Ok(true));

    let (variable, _) = package.intern(&mut ctx, &runtime, "*").unwrap();
    assert_eq!(symbol_function(&ctx, variable), Ok(Word::UNBOUND));
    assert_eq!(symbol_is_special(&ctx, variable), Ok(true));

    assert!(runtime.class(&mut ctx, "ARRAY").is_some());
    assert!(runtime.class(&mut ctx, "CAST").is_some());
    assert!(runtime.class(&mut ctx, "FUNCTION").is_some());
}

#[test]
fn registration_reports_a_locked_package_error() {
    fixture!(runtime, ctx);
    ncl_ffi::register(&runtime).unwrap();
    let package = runtime.find_package(&ctx, "NCL-FFI").unwrap();
    Package::from_word(package)
        .set_locked(&mut ctx, true)
        .unwrap();

    assert_eq!(ncl_ffi::register(&runtime), Err(ObjectError::TypeError));
}
