#![allow(
    clippy::unwrap_used,
    reason = "save tests assert on valid fixture construction"
)]

use super::{Capture, save};
use crate::format::{HEADER_SIZE, ImageFile};
use crate::record::{Record, Ref};
use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, ArrayOptions, BuiltinImplementation, Package, Runtime, ThreadContext, Word,
    car, cdr, make_array, make_bignum_from_limbs, make_complex, make_cons, make_double, make_ratio,
    make_readtable, make_simple_vector, make_specialized_array, make_stream, make_string,
    simple_vector_ref,
};

#[path = "../tests/support/image_builtin.rs"]
mod image_builtin_support;
use image_builtin_support::{BUILTIN, BUILTIN_ID, image_builtin};

fn object_root(image: &ImageFile, index: usize) -> &Record {
    let Ref::Object(id) = image.roots[index] else {
        panic!("root {index} must refer to an object")
    };
    &image.objects[id as usize]
}

#[test]
fn save_records_shared_graph_values_and_loads_ordered_roots() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let shared = make_cons(&mut ctx, &runtime, Word::fixnum(3), Word::NIL).unwrap();
    let first = make_cons(&mut ctx, &runtime, shared, Word::fixnum(10)).unwrap();
    let second = make_cons(&mut ctx, &runtime, shared, Word::fixnum(20)).unwrap();
    let bytes = save(&runtime, &mut ctx, &[first, second, first], &[]).unwrap();

    let image = ImageFile::from_bytes(&bytes).unwrap();
    assert_eq!(
        image.roots,
        vec![Ref::Object(0), Ref::Object(1), Ref::Object(0)]
    );
    assert_eq!(
        image.objects[0],
        Record::Cons {
            car: Ref::Object(2),
            cdr: Ref::Immediate(Word::fixnum(10).bits()),
        }
    );
    assert_eq!(
        image.objects[1],
        Record::Cons {
            car: Ref::Object(2),
            cdr: Ref::Immediate(Word::fixnum(20).bits()),
        }
    );
    assert_eq!(
        image.objects[2],
        Record::Cons {
            car: Ref::Immediate(Word::fixnum(3).bits()),
            cdr: Ref::Immediate(Word::NIL.bits()),
        }
    );
    assert_eq!(
        bytes.len(),
        HEADER_SIZE + u32::from_le_bytes(bytes[44..48].try_into().unwrap()) as usize
    );

    let loaded_runtime = Runtime::new().unwrap();
    let mut loaded_ctx = ThreadContext::new();
    loaded_ctx.register(&loaded_runtime).unwrap();
    let loaded = crate::load(&bytes, &loaded_runtime, &mut loaded_ctx).unwrap();
    assert_eq!(loaded.roots.len(), 3);
    assert_eq!(loaded.roots[0], loaded.roots[2]);
    assert_ne!(loaded.roots[0], loaded.roots[1]);
    let first_child = car(&loaded_ctx, loaded.roots[0]).unwrap();
    let second_child = car(&loaded_ctx, loaded.roots[1]).unwrap();
    assert_eq!(first_child, second_child, "the shared child stays shared");
    assert_eq!(car(&loaded_ctx, first_child).unwrap(), Word::fixnum(3));
    assert_eq!(cdr(&loaded_ctx, loaded.roots[0]).unwrap(), Word::fixnum(10));
    assert_eq!(cdr(&loaded_ctx, loaded.roots[1]).unwrap(), Word::fixnum(20));
}

#[test]
fn save_records_package_hash_specialized_array_and_number_payloads() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let package = Package::new(&mut ctx, &runtime, "SAVE-PACKAGE").unwrap();
    let nickname = make_string(&mut ctx, &runtime, &['S', 'P']).unwrap();
    assert!(package.add_nickname(&mut ctx, &runtime, nickname).unwrap());
    let (symbol, status) = package.intern(&mut ctx, &runtime, "VALUE").unwrap();
    assert_eq!(status, ncl_object::package::FindStatus::Internal);
    let key = make_string(&mut ctx, &runtime, &['K']).unwrap();
    let table = HashTable::new(&mut ctx, &runtime, HashTest::Equal, Weakness::None).unwrap();
    table.insert(&mut ctx, &runtime, key, symbol).unwrap();
    let specialized = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Fixnum,
        &[Word::fixnum(3), Word::fixnum(5)],
    )
    .unwrap();
    let bignum = make_bignum_from_limbs(&mut ctx, &runtime, true, &[7, 11]).unwrap();
    let ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(2), Word::fixnum(3)).unwrap();
    let double = make_double(&mut ctx, &runtime, 2.5).unwrap();
    let complex = make_complex(&mut ctx, &runtime, Word::fixnum(1), double.into()).unwrap();
    let bytes = save(
        &runtime,
        &mut ctx,
        &[
            package.as_word(),
            symbol,
            table.as_word(),
            specialized,
            bignum.into(),
            ratio.into(),
            complex.into(),
        ],
        &[],
    )
    .unwrap();
    let image = ImageFile::from_bytes(&bytes).unwrap();

    assert_package_and_hash_records(&image);
    assert_specialized_array_and_number_records(&image);
}

fn assert_package_and_hash_records(image: &ImageFile) {
    assert_eq!(
        object_root(image, 0),
        &Record::Package {
            name: "SAVE-PACKAGE".to_owned(),
            nicknames: vec!["SP".to_owned()],
        }
    );
    let Ref::Object(symbol_id) = image.roots[1] else {
        panic!("symbol root")
    };
    let Record::HashTable {
        test,
        weakness,
        entries,
    } = object_root(image, 2)
    else {
        panic!("hash-table root")
    };
    assert_eq!(*test, HashTest::Equal as u8);
    assert_eq!(*weakness, Weakness::None as u8);
    assert_eq!(entries.len(), 1);
    let (key_ref, value_ref) = entries[0];
    assert_eq!(value_ref, Ref::Object(symbol_id));
    let Ref::Object(key_id) = key_ref else {
        panic!("hash key record")
    };
    assert_eq!(
        image.objects[key_id as usize],
        Record::String("K".to_owned())
    );
    assert!(matches!(
        &image.objects[symbol_id as usize],
        Record::Symbol { package, name, .. }
            if package == "SAVE-PACKAGE" && name == "VALUE"
    ));
}

fn assert_specialized_array_and_number_records(image: &ImageFile) {
    assert!(matches!(
        object_root(image, 3),
        Record::SpecializedArray { element_type, elements }
            if *element_type == ArrayElementType::Fixnum as u8
                && elements == &[Word::fixnum(3).bits(), Word::fixnum(5).bits()]
    ));
    assert_eq!(
        object_root(image, 4),
        &Record::Bignum {
            negative: true,
            limbs: vec![7, 11],
        }
    );
    assert_eq!(
        object_root(image, 5),
        &Record::Ratio {
            numerator: Ref::Immediate(Word::fixnum(2).bits()),
            denominator: Ref::Immediate(Word::fixnum(3).bits()),
        }
    );
    let Record::Complex { real, imag } = object_root(image, 6) else {
        panic!("complex root")
    };
    assert_eq!(*real, Ref::Immediate(Word::fixnum(1).bits()));
    let Ref::Object(double_id) = imag else {
        panic!("imaginary double-float record")
    };
    assert_eq!(
        image.objects[*double_id as usize],
        Record::DoubleFloat {
            bits: 2.5_f64.to_bits(),
        }
    );
}

#[test]
fn save_reports_exact_unsupported_kind_payloads() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let array = make_array(
        &mut ctx,
        &runtime,
        &[1],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap();
    let readtable = make_readtable(&mut ctx, &runtime, Word::NIL, Word::NIL, Word::NIL).unwrap();
    let stream = make_stream(
        &mut ctx,
        &runtime,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap();
    for (object, kind) in [
        (array, "non-simple array"),
        (readtable.into(), "readtable"),
        (stream.into(), "stream"),
    ] {
        assert_eq!(
            save(&runtime, &mut ctx, &[object], &[]),
            Err(crate::ImageError::UnsupportedKind { kind })
        );
    }
}

#[test]
fn registered_builtin_capture_zeros_the_entry() -> Result<(), crate::ImageError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let function = runtime.register_builtin(
        &mut ctx,
        BUILTIN_ID,
        BuiltinImplementation::direct(BUILTIN, image_builtin).with_entry(0x1234_5678),
    )?;

    let mut capture = Capture::new(&runtime);
    capture.run(&mut ctx, &[function.as_word()])?;
    let records = capture.finish();

    assert!(records.iter().any(|record| matches!(
        record,
        Record::Function {
            entry: 0,
            builtin: Some(identity),
            ..
        } if identity.package == "NCL-TEST" && identity.name == image_builtin_support::BUILTIN_NAME
    )));
    Ok(())
}

#[test]
fn save_capture_helpers_preserve_values_after_load() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let string = make_string(&mut ctx, &runtime, &['v', 'a', 'l', 'u', 'e']).unwrap();
    let vector = make_simple_vector(&mut ctx, &runtime, &[string, Word::fixnum(-7)]).unwrap();
    let bytes = save(&runtime, &mut ctx, &[vector], &[]).unwrap();
    let image = ImageFile::from_bytes(&bytes).unwrap();
    let Record::Vector(elements) = object_root(&image, 0) else {
        panic!("vector root")
    };
    assert_eq!(elements.len(), 2);
    let Ref::Object(string_id) = elements[0] else {
        panic!("string reference")
    };
    assert_eq!(
        image.objects[string_id as usize],
        Record::String("value".to_owned())
    );
    assert_eq!(elements[1], Ref::Immediate(Word::fixnum(-7).bits()));

    let loaded_runtime = Runtime::new().unwrap();
    let mut loaded_ctx = ThreadContext::new();
    loaded_ctx.register(&loaded_runtime).unwrap();
    let loaded = crate::load(&bytes, &loaded_runtime, &mut loaded_ctx).unwrap();
    let loaded_vector = loaded.roots[0];
    let loaded_string = simple_vector_ref(&loaded_ctx, loaded_vector, 0).unwrap();
    assert_eq!(
        ncl_object::string_length(&loaded_ctx, loaded_string).unwrap(),
        5
    );
    for (index, expected) in "value".chars().enumerate() {
        assert_eq!(
            ncl_object::string_ref(&loaded_ctx, loaded_string, index).unwrap(),
            expected
        );
    }
    assert_eq!(
        simple_vector_ref(&loaded_ctx, loaded_vector, 1).unwrap(),
        Word::fixnum(-7)
    );
}
