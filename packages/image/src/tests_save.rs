#![allow(
    clippy::unwrap_used,
    reason = "save tests assert on valid fixture construction"
)]

use super::save;
use crate::format::{HEADER_SIZE, ImageFile};
use crate::record::{Record, Ref};
use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, ArrayOptions, Package, Runtime, ThreadContext, Word, make_array,
    make_bignum_from_limbs, make_complex, make_cons, make_double, make_ratio, make_readtable,
    make_simple_vector, make_specialized_array, make_stream, make_string,
};

#[test]
fn save_records_shared_graph_values_and_round_trips_bytes() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let string = make_string(&mut ctx, &runtime, &['a', 'l', 'i', 'g', 'n']).unwrap();
    let vector = make_simple_vector(&mut ctx, &runtime, &[string, Word::fixnum(42)]).unwrap();
    let cons = make_cons(&mut ctx, &runtime, vector, vector).unwrap();
    let bytes = save(&runtime, &mut ctx, &[cons, vector, Word::NIL], &[]).unwrap();
    let image = ImageFile::from_bytes(&bytes).unwrap();
    assert_eq!(image.roots.len(), 3);
    assert_eq!(image.roots[2], Ref::Immediate(Word::NIL.bits()));
    let Ref::Object(cons_id) = image.roots[0] else {
        panic!("cons root")
    };
    let Ref::Object(vector_id) = image.roots[1] else {
        panic!("vector root")
    };
    assert_eq!(
        image.objects[cons_id as usize],
        Record::Cons {
            car: Ref::Object(vector_id),
            cdr: Ref::Object(vector_id)
        }
    );
    assert_eq!(
        image.objects[vector_id as usize],
        Record::Vector(vec![
            Ref::Object(vector_id + 1),
            Ref::Immediate(Word::fixnum(42).bits()),
        ])
    );
    assert_eq!(
        image.objects[vector_id as usize + 1],
        Record::String("align".to_owned())
    );
    let Record::Vector(elements) = &image.objects[vector_id as usize] else {
        panic!("expected vector")
    };
    assert_eq!(elements.len(), 2);
    assert_eq!(
        bytes.len(),
        HEADER_SIZE + u32::from_le_bytes(bytes[44..48].try_into().unwrap()) as usize
    );
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
    let Record::Vector(elements) = &image.objects[0] else {
        panic!("vector record")
    };
    let Ref::Object(string_id) = elements[0] else {
        panic!("string reference")
    };
    assert!(matches!(
        image.objects[string_id as usize],
        Record::String(_)
    ));
    assert_eq!(elements[1], Ref::Immediate(Word::fixnum(-7).bits()));
}

#[test]
fn save_captures_packages_hash_tables_specialized_arrays_and_numbers() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let package = Package::new(&mut ctx, &runtime, "SAVE-PACKAGE").unwrap();
    let nickname = make_string(&mut ctx, &runtime, &['S', 'P']).unwrap();
    package.add_nickname(&mut ctx, &runtime, nickname).unwrap();
    let (symbol, _) = package.intern(&mut ctx, &runtime, "VALUE").unwrap();
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
    assert!(image.objects.iter().any(|record| matches!(record, Record::Package { name, nicknames } if name == "SAVE-PACKAGE" && nicknames == &["SP"])));
    assert!(
        image.objects.iter().any(
            |record| matches!(record, Record::HashTable { entries, .. } if entries.len() == 1)
        )
    );
    assert!(image.objects.iter().any(
        |record| matches!(record, Record::SpecializedArray { elements, .. } if elements.len() == 2)
    ));
    assert!(image.objects.iter().any(
        |record| matches!(record, Record::Bignum { negative: true, limbs } if limbs == &[7, 11])
    ));
    assert!(
        image
            .objects
            .iter()
            .any(|record| matches!(record, Record::Ratio { .. }))
    );
    assert!(image.objects.iter().any(
        |record| matches!(record, Record::DoubleFloat { bits } if *bits == 2.5_f64.to_bits())
    ));
    assert!(
        image
            .objects
            .iter()
            .any(|record| matches!(record, Record::Complex { .. }))
    );
}

#[test]
fn save_reports_unsupported_array_readtable_and_stream_kinds() {
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
    for object in [array, readtable.into(), stream.into()] {
        assert!(matches!(
            save(&runtime, &mut ctx, &[object], &[]),
            Err(crate::ImageError::UnsupportedKind { .. })
        ));
    }
}
