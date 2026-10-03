#![allow(
    clippy::expect_used,
    clippy::float_cmp,
    clippy::needless_pass_by_value,
    clippy::redundant_closure,
    clippy::too_many_lines,
    clippy::unwrap_used,
    reason = "tests assert on restore failures"
)]

use super::{check_architecture, decode_test, decode_weakness, fix, fix_u64, resolve};
use crate::error::ImageError;
use crate::format::ImageFile;
use crate::record::{Record, Ref};
use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, Bignum, CodeObject, Complex, DoubleFloat, Function, Instance, Ratio, Runtime,
    ThreadContext, Word, bignum_limbs, code_constants, code_debug, code_entry, code_size,
    code_stack_map, complex_imag, complex_real, double_value, function_code, function_entry,
    function_lambda_list, function_name, instance_class, ratio_denominator, ratio_numerator,
    simple_vector_ref, specialized_array_element_type, specialized_array_ref, string_length,
    string_ref, structure_ref, symbol_flags, symbol_name, symbol_value,
};

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

#[test]
fn load_rebuilds_every_record_kind_with_preserved_values() {
    let image = ImageFile {
        architecture: if cfg!(target_arch = "x86_64") {
            ncl_objfile::Architecture::X86_64
        } else {
            ncl_objfile::Architecture::Aarch64
        },
        gc_epoch: 27,
        objects: vec![
            Record::Package {
                name: "RESTORE-TEST".to_owned(),
                nicknames: vec!["RT".to_owned()],
            },
            Record::String("payload".to_owned()),
            Record::Symbol {
                package: "RESTORE-TEST".to_owned(),
                name: "VALUE".to_owned(),
                flags: 9,
                value: Ref::Immediate(Word::fixnum(42).bits()),
                function: Ref::Immediate(Word::NIL.bits()),
                plist: Ref::Immediate(Word::NIL.bits()),
            },
            Record::Cons {
                car: Ref::Object(1),
                cdr: Ref::Immediate(Word::fixnum(6).bits()),
            },
            Record::Vector(vec![Ref::Object(1), Ref::Object(2)]),
            Record::SpecializedArray {
                element_type: ArrayElementType::Fixnum as u8,
                elements: vec![Word::fixnum(8).bits(), Word::fixnum(13).bits()],
            },
            Record::HashTable {
                test: HashTest::Equalp as u8,
                weakness: Weakness::KeyOrValue as u8,
                entries: vec![(Ref::Immediate(Word::fixnum(3).bits()), Ref::Object(1))],
            },
            Record::Structure {
                slots: vec![Ref::Immediate(Word::fixnum(11).bits()), Ref::Object(1)],
            },
            Record::Instance {
                class: Ref::Immediate(Word::fixnum(17).bits()),
                slots: vec![Ref::Object(1)],
            },
            Record::CodeObject {
                entry: 12,
                size: 4,
                constants: Ref::Object(4),
                stack_map: Ref::Object(5),
                debug: Ref::Object(1),
            },
            Record::Function {
                closure: true,
                entry: 13,
                name: Ref::Object(2),
                lambda_list: Ref::Object(1),
                code: Ref::Object(9),
                captures: vec![Ref::Object(1), Ref::Object(4)],
            },
            Record::Bignum {
                negative: true,
                limbs: vec![1, u32::MAX, 2],
            },
            Record::Ratio {
                numerator: Ref::Immediate(Word::fixnum(5).bits()),
                denominator: Ref::Immediate(Word::fixnum(7).bits()),
            },
            Record::DoubleFloat {
                bits: 1.25_f64.to_bits(),
            },
            Record::Complex {
                real: Ref::Object(11),
                imag: Ref::Object(13),
            },
        ],
        roots: (0..15).map(|id| Ref::Object(id)).collect(),
        code: Vec::new(),
        features: vec!["restore-feature".to_owned()],
    }
    .to_bytes()
    .unwrap();

    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let loaded = super::load(&image, &runtime, &mut ctx).unwrap();
    assert_eq!(loaded.roots.len(), 15);

    assert_eq!(string_length(&ctx, loaded.roots[1]).unwrap(), 7);
    assert_eq!(string_ref(&ctx, loaded.roots[1], 3).unwrap(), 'l');
    assert_eq!(symbol_flags(&ctx, loaded.roots[2]).unwrap(), 9);
    assert_eq!(
        symbol_value(&ctx, loaded.roots[2]).unwrap(),
        Word::fixnum(42)
    );
    assert_eq!(
        simple_vector_ref(&ctx, loaded.roots[4], 1).unwrap(),
        loaded.roots[2]
    );
    assert_eq!(
        specialized_array_element_type(&ctx, loaded.roots[5]).unwrap(),
        ArrayElementType::Fixnum
    );
    assert_eq!(
        specialized_array_ref(&ctx, loaded.roots[5], 1).unwrap(),
        Word::fixnum(13)
    );

    let table = HashTable::from_word(loaded.roots[6]);
    assert_eq!(table.test(&ctx).unwrap(), HashTest::Equalp);
    assert_eq!(table.weakness(&ctx).unwrap(), Weakness::KeyOrValue);
    assert_eq!(
        table.get(&mut ctx, Word::fixnum(3)).unwrap(),
        Some(loaded.roots[1])
    );
    assert_eq!(
        structure_ref(&ctx, loaded.roots[7], 0).unwrap(),
        Word::fixnum(11)
    );
    assert_eq!(
        instance_class(&ctx, Instance::from_word(loaded.roots[8])).unwrap(),
        Word::fixnum(17)
    );
    assert_eq!(
        code_entry(&ctx, CodeObject::from_word(loaded.roots[9])).unwrap(),
        Word::fixnum(12)
    );
    assert_eq!(
        code_size(&ctx, CodeObject::from_word(loaded.roots[9])).unwrap(),
        Word::fixnum(4)
    );
    assert_eq!(
        code_constants(&ctx, CodeObject::from_word(loaded.roots[9])).unwrap(),
        loaded.roots[4]
    );
    assert_eq!(
        code_stack_map(&ctx, CodeObject::from_word(loaded.roots[9])).unwrap(),
        loaded.roots[5]
    );
    assert_eq!(
        code_debug(&ctx, CodeObject::from_word(loaded.roots[9])).unwrap(),
        loaded.roots[1]
    );

    let function = Function::from_word(loaded.roots[10]);
    assert_eq!(function_entry(&ctx, function).unwrap(), 13);
    assert_eq!(function_name(&ctx, function).unwrap(), loaded.roots[2]);
    assert_eq!(
        function_lambda_list(&ctx, function).unwrap(),
        loaded.roots[1]
    );
    assert_eq!(
        function_code(&ctx, function).unwrap().as_word(),
        loaded.roots[9]
    );
    assert_eq!(
        ncl_object::closure_ref(&ctx, function, 1).unwrap(),
        loaded.roots[4]
    );
    assert_eq!(
        bignum_limbs(&ctx, Bignum::from_word(loaded.roots[11])).unwrap(),
        vec![1, u32::MAX, 2]
    );
    assert_eq!(
        ratio_numerator(&ctx, Ratio::from_word(loaded.roots[12])).unwrap(),
        Word::fixnum(5)
    );
    assert_eq!(
        ratio_denominator(&ctx, Ratio::from_word(loaded.roots[12])).unwrap(),
        Word::fixnum(7)
    );
    assert_eq!(
        double_value(&ctx, DoubleFloat::from_word(loaded.roots[13])).unwrap(),
        1.25
    );
    assert_eq!(
        complex_real(&ctx, Complex::from_word(loaded.roots[14])).unwrap(),
        loaded.roots[11]
    );
    assert_eq!(
        complex_imag(&ctx, Complex::from_word(loaded.roots[14])).unwrap(),
        loaded.roots[13]
    );
    let restored_name = symbol_name(&ctx, loaded.roots[2]).unwrap();
    assert_eq!(string_length(&ctx, restored_name).unwrap(), 5);
    assert_eq!(string_ref(&ctx, restored_name, 0).unwrap(), 'V');
}

#[test]
fn load_rejects_bad_object_reference_without_leaking_roots() {
    let image = ImageFile {
        architecture: if cfg!(target_arch = "x86_64") {
            ncl_objfile::Architecture::X86_64
        } else {
            ncl_objfile::Architecture::Aarch64
        },
        gc_epoch: 0,
        objects: vec![Record::Vector(vec![Ref::Object(99)])],
        roots: vec![Ref::Object(0)],
        code: Vec::new(),
        features: Vec::new(),
    }
    .to_bytes()
    .unwrap();
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    assert_eq!(
        super::load(&image, &runtime, &mut ctx).unwrap_err(),
        ImageError::InvalidLayout {
            field: "object reference"
        }
    );
}
