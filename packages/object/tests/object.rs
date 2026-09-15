#![allow(missing_docs)]

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::package::Package;
use ncl_object::{
    ArrayElementType, ArrayOptions, ObjectRef, Runtime, ThreadContext, array_dimensions,
    array_row_major_ref, array_row_major_set, builtin, car, classify, classify_object, make_array,
    make_cons, make_simple_vector, make_specialized_array, make_string, make_symbol, register,
    set_symbol_value, simple_vector_ref, string_ref, symbol_name, symbol_value,
};
use ncl_sys::Word;

#[test]
fn classify_and_allocate() {
    assert_eq!(classify(Word::fixnum(-2)), ObjectRef::Fixnum(-2));
    assert_eq!(classify(Word::NIL), ObjectRef::Symbol(Word::NIL));
    let runtime = Runtime::new();
    let mut ctx = ThreadContext::new();
    assert!(make_cons(&mut ctx, &runtime, Word::NIL, Word::NIL).is_err());
    assert!(ctx.register(&runtime).is_ok());
    let cons = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL).unwrap_or(Word::NIL);
    assert_eq!(car(&mut ctx, cons), Ok(Word::fixnum(1)));
}

#[test]
fn symbols_and_bindings() {
    let runtime = Runtime::new();
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    let symbol = make_symbol(&mut ctx, &runtime, Word::NIL).unwrap_or(Word::NIL);
    assert_eq!(symbol_name(&ctx, symbol), Ok(Word::NIL));
    assert_eq!(symbol_value(&ctx, symbol), Ok(Word::UNBOUND));
    assert!(set_symbol_value(&mut ctx, symbol, Word::fixnum(9)).is_ok());
    assert_eq!(symbol_value(&ctx, symbol), Ok(Word::fixnum(9)));
    ctx.bind(1, Word::fixnum(1));
    ctx.bind(1, Word::fixnum(2));
    assert_eq!(ctx.unbind(1), Ok(Word::fixnum(2)));
}

#[test]
fn builtin_metadata_expands() {
    builtin!(TEST_BUILTIN, 2);
    assert_eq!(TEST_BUILTIN.arity, 2);
}

#[test]
fn builtin_abi_expands_for_fixed_and_variadic_forms() {
    builtin!(TEST_ABI, 2, "a b", test_direct, test_variadic);
    let direct: extern "C" fn(*mut ThreadContext, Word, Word) -> Word = test_direct;
    let variadic: extern "C" fn(
        *mut ThreadContext,
        usize,
        *const Word,
        *mut ncl_object::MultipleValues,
    ) -> ncl_object::NclStatus = test_variadic;
    assert_eq!(TEST_ABI.arity, 2);
    const { assert!(TEST_ABI.direct) };
    assert_eq!(TEST_ABI.lambda_list, "a b");
    let _ = (direct, variadic);
}

#[test]
fn gc_extensions_register_the_owned_symbols() {
    let runtime = Runtime::new();
    register(&runtime);
    for name in [
        "*AFTER-GC-HOOKS*",
        "*GC-REAL-TIME*",
        "*GC-RUN-TIME*",
        "CANCEL-FINALIZATION",
        "FINALIZE",
        "GC",
        "GENERATION-BYTES-ALLOCATED",
        "HASH-TABLE-WEAKNESS",
        "MAKE-WEAK-POINTER",
        "MAKE-WEAK-VECTOR",
        "WEAK-POINTER",
        "WEAK-POINTER-P",
        "WEAK-POINTER-VALUE",
        "WEAK-VECTOR-P",
    ] {
        assert_eq!(runtime.function("SB-EXT", name), Some(Word::UNBOUND));
    }
}

#[test]
fn gc_preserves_object_accessors_and_weak_entries() {
    let runtime = Runtime::new();
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    assert!(runtime.register_layouts().is_ok());
    assert!(
        ncl_sys::register_layout(
            runtime.heap(),
            99,
            ncl_sys::ReferenceLayout {
                reference_words: vec![1],
                boxed_from: None,
            },
        )
        .is_ok()
    );

    let mut list = Word::NIL;
    for value in 0..10_000 {
        list = make_cons(&mut ctx, &runtime, Word::fixnum(value), list).unwrap_or(Word::NIL);
    }
    let mut symbols = Vec::new();
    for _ in 0..1_000 {
        symbols.push(make_symbol(&mut ctx, &runtime, Word::NIL).unwrap_or(Word::NIL));
    }
    let mut package = Package::new("GC-TEST");
    let interned = package
        .intern(&mut ctx, &runtime, "SAME")
        .map_or(Word::NIL, |pair| pair.0);
    let mut weak = ncl_object::allocate(&mut ctx, &runtime, 99, 1).unwrap_or(Word::NIL);
    let referent = make_cons(&mut ctx, &runtime, Word::fixnum(7), Word::NIL).unwrap_or(Word::NIL);
    assert!(ctx.write_object_slot(weak, 0, referent).is_ok());
    weak = ctx.make_weak(weak, ncl_sys::Weakness::Value);
    let mut roots = symbols;
    roots.push(list);
    roots.push(interned);
    roots.push(weak);
    let tokens = roots
        .iter_mut()
        .map(|value| ncl_object::push_root(&mut ctx, value))
        .collect::<Vec<_>>();
    assert_eq!(ncl_sys::widetag(runtime.heap(), roots[1_001]), Some(1));
    let table_key = Word::fixnum(42);
    let Ok(table) = HashTable::new(&mut ctx, &runtime, HashTest::Eq, Weakness::None) else {
        return;
    };
    let mut table_word = table.as_word();
    let table_token = ncl_object::push_root(&mut ctx, &mut table_word);
    let table = HashTable::from(table_word);
    assert!(table
        .insert(&mut ctx, &runtime, table_key, Word::fixnum(99))
        .is_ok());

    ctx.collect(false);
    assert_eq!(car(&mut ctx, roots[1_000]), Ok(Word::fixnum(9_999)));
    assert_eq!(ncl_sys::widetag(runtime.heap(), roots[1_001]), Some(1));
    assert_eq!(symbol_value(&ctx, roots[1_001]), Ok(Word::UNBOUND));
    assert_eq!(table.get(&mut ctx, table_key), Ok(Some(Word::fixnum(99))));
    assert_eq!(
        package
            .intern(&mut ctx, &runtime, "SAME")
            .map(|pair| pair.0),
        Ok(interned)
    );

    ctx.collect(true);
    assert_eq!(car(&mut ctx, roots[1_000]), Ok(Word::fixnum(9_999)));
    let table = HashTable::from(table_word);
    assert_eq!(table.get(&mut ctx, table_key), Ok(Some(Word::fixnum(99))));
    assert_eq!(ctx.weak_value(weak), Word::NIL);
    assert!(ncl_object::pop_root(&mut ctx, table_token));
    for token in tokens.into_iter().rev() {
        assert!(ncl_object::pop_root(&mut ctx, token));
    }
}

#[test]
fn strings_and_simple_vectors_have_typed_accessors() {
    let runtime = Runtime::new();
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    let string = make_string(&mut ctx, &runtime, &['a', 'λ']).unwrap_or(Word::NIL);
    assert_eq!(string_ref(&ctx, string, 1), Ok('λ'));
    assert_eq!(classify_object(&ctx, string), ObjectRef::String(string));
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(4)]).unwrap_or(Word::NIL);
    assert_eq!(simple_vector_ref(&ctx, vector, 0), Ok(Word::fixnum(4)));
    assert_eq!(
        classify_object(&ctx, vector),
        ObjectRef::SimpleVector(vector)
    );
}

#[test]
fn specialized_arrays_validate_element_type() {
    let runtime = Runtime::new();
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    let array = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(1)],
    )
    .unwrap_or(Word::NIL);
    assert_eq!(
        ncl_object::specialized_array_ref(&ctx, array, 0),
        Ok(Word::fixnum(1))
    );
    assert!(ncl_object::specialized_array_set(&mut ctx, array, 0, Word::fixnum(2)).is_err());
    assert_eq!(
        classify_object(&ctx, array),
        ObjectRef::SpecializedArray(array)
    );
}

#[test]
fn non_simple_arrays_store_dimensions_and_row_major_values() {
    let runtime = Runtime::new();
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    let array = make_array(
        &mut ctx,
        &runtime,
        &[2, 2],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: true,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap_or(Word::NIL);
    assert_eq!(array_dimensions(&ctx, array), Ok(vec![2, 2]));
    assert!(array_row_major_set(&mut ctx, array, 3, Word::fixnum(8)).is_ok());
    assert_eq!(array_row_major_ref(&ctx, array, 3), Ok(Word::fixnum(8)));
    assert_eq!(classify_object(&ctx, array), ObjectRef::Array(array));
}

#[test]
fn non_simple_array_references_survive_minor_and_full_gc() {
    let runtime = Runtime::new();
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    assert!(runtime.register_layouts().is_ok());
    let target = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(11)]).unwrap_or(Word::NIL);
    let rank_one = make_array(
        &mut ctx,
        &runtime,
        &[1],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: false,
            fill_pointer: Some(1),
            displaced_to: Some(target),
            displaced_index_offset: 0,
        },
    )
    .unwrap_or(Word::NIL);
    let payload = make_string(&mut ctx, &runtime, &['x']).unwrap_or(Word::NIL);
    let rank_three = make_array(
        &mut ctx,
        &runtime,
        &[1, 1, 1],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: payload,
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap_or(Word::NIL);
    let mut rank_one_root = rank_one;
    let _rank_one_token = ncl_object::push_root(&mut ctx, &mut rank_one_root);
    let mut rank_three_root = rank_three;
    let _rank_three_token = ncl_object::push_root(&mut ctx, &mut rank_three_root);
    ctx.collect(false);
    ctx.collect(true);
    assert_eq!(
        array_row_major_ref(&ctx, rank_one_root, 0),
        Ok(Word::fixnum(11))
    );
    let value = array_row_major_ref(&ctx, rank_three_root, 0).unwrap_or(Word::NIL);
    assert_eq!(string_ref(&ctx, value, 0), Ok('x'));
}

#[test]
#[allow(clippy::too_many_lines)]
fn remaining_object_kinds_round_trip() {
    let runtime = Runtime::new();
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    assert!(runtime.register_layouts().is_ok());
    let name = make_string(&mut ctx, &runtime, &['N', 'A', 'M', 'E']).unwrap_or(Word::NIL);
    let symbol = make_symbol(&mut ctx, &runtime, name).unwrap_or(Word::NIL);
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(21), Word::fixnum(22)])
        .unwrap_or(Word::NIL);
    let specialized = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Fixnum,
        &[Word::fixnum(31), Word::fixnum(32)],
    )
    .unwrap_or(Word::NIL);
    let layout = runtime
        .register_structure_layout(2)
        .unwrap_or_else(|_| 0.into());
    let structure = ncl_object::make_structure(
        &mut ctx,
        &runtime,
        layout,
        &[Word::fixnum(1), Word::fixnum(2)],
    )
    .unwrap_or(Word::NIL);
    assert_eq!(ncl_object::structure_layout(&ctx, structure), Ok(layout));
    assert_eq!(
        ncl_object::structure_ref(&ctx, structure, 1),
        Ok(Word::fixnum(2))
    );
    let instance = ncl_object::make_instance(&mut ctx, &runtime, Word::NIL, &[Word::fixnum(3)])
        .unwrap_or_else(|_| Word::NIL.into());
    assert_eq!(ncl_object::slot_ref(&ctx, instance, 0), Ok(Word::fixnum(3)));
    assert!(ncl_object::slot_set(&mut ctx, instance, 0, Word::fixnum(4)).is_ok());
    let bignum = ncl_object::make_bignum_from_i128(&mut ctx, &runtime, -0x1_0000_0001)
        .unwrap_or_else(|_| Word::NIL.into());
    assert_eq!(ncl_object::bignum_limbs(&ctx, bignum), Ok(vec![1, 1]));
    let double =
        ncl_object::make_double(&mut ctx, &runtime, 1.25).unwrap_or_else(|_| Word::NIL.into());
    assert_eq!(ncl_object::double_value(&ctx, double), Ok(1.25));
    let code = ncl_object::make_code_object(&mut ctx, &runtime, 7, 4, bignum.into(), name, vector)
        .unwrap_or_else(|_| Word::NIL.into());
    let simple_fun = ncl_object::make_simple_fun(&mut ctx, &runtime, 8, name, specialized, code)
        .unwrap_or_else(|_| Word::NIL.into());
    let function = ncl_object::make_closure(
        &mut ctx,
        &runtime,
        7,
        Word::NIL,
        Word::NIL,
        code,
        &[instance.into(), bignum.into(), double.into()],
    )
    .unwrap_or_else(|_| Word::NIL.into());
    assert_eq!(ncl_object::function_entry(&ctx, function), Ok(7));
    assert_eq!(
        ncl_object::closure_ref(&ctx, function, 0),
        Ok(instance.into())
    );
    assert_eq!(
        ncl_object::closure_ref(&ctx, function, 1),
        Ok(bignum.into())
    );
    assert_eq!(
        ncl_object::closure_ref(&ctx, function, 2),
        Ok(double.into())
    );
    let stream = ncl_object::make_stream(
        &mut ctx,
        &runtime,
        Word::fixnum(1),
        Word::NIL,
        Word::NIL,
        Word::fixnum(9),
        Word::NIL,
    )
    .unwrap_or_else(|_| Word::NIL.into());
    assert_eq!(ncl_object::stream_state(&ctx, stream), Ok(Word::fixnum(9)));
    let ratio = ncl_object::make_ratio(&mut ctx, &runtime, bignum.into(), double.into())
        .unwrap_or_else(|_| Word::NIL.into());
    let complex = ncl_object::make_complex(&mut ctx, &runtime, ratio.into(), bignum.into())
        .unwrap_or_else(|_| Word::NIL.into());
    let readtable = ncl_object::make_readtable(
        &mut ctx,
        &runtime,
        complex.into(),
        ratio.into(),
        Word::fixnum(2),
    )
    .unwrap_or_else(|_| Word::NIL.into());
    let mut roots = [
        structure,
        instance.into(),
        bignum.into(),
        double.into(),
        code.into(),
        function.into(),
        stream.into(),
        ratio.into(),
        complex.into(),
        readtable.into(),
        name,
        symbol,
        vector,
        specialized,
        simple_fun.into(),
    ];
    let tokens: Vec<_> = roots
        .iter_mut()
        .map(|value| ncl_object::push_root(&mut ctx, value))
        .collect();
    for index in 0..32 {
        let suffix = u8::try_from(index % 26).unwrap_or(0);
        let _ = make_string(&mut ctx, &runtime, &[char::from(b'a' + suffix)]);
    }
    ctx.collect(false);
    assert_eq!(classify_object(&ctx, roots[2]), ObjectRef::Bignum(roots[2]));
    ctx.collect(true);
    assert_eq!(
        ncl_object::bignum_limbs(&ctx, roots[2].into()),
        Ok(vec![1, 1])
    );
    assert_eq!(
        ncl_object::structure_ref(&ctx, roots[0], 0),
        Ok(Word::fixnum(1))
    );
    assert_eq!(
        ncl_object::structure_ref(&ctx, roots[0], 1),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        ncl_object::slot_ref(&ctx, roots[1].into(), 0),
        Ok(Word::fixnum(4))
    );
    assert_eq!(
        ncl_object::closure_ref(&ctx, roots[5].into(), 0),
        Ok(roots[1])
    );
    assert_eq!(
        ncl_object::closure_ref(&ctx, roots[5].into(), 1),
        Ok(roots[2])
    );
    assert_eq!(
        ncl_object::closure_ref(&ctx, roots[5].into(), 2),
        Ok(roots[3])
    );
    assert_eq!(
        ncl_object::code_entry(&ctx, roots[4].into()),
        Ok(Word::fixnum(7))
    );
    assert_eq!(
        ncl_object::code_size(&ctx, roots[4].into()),
        Ok(Word::fixnum(4))
    );
    assert_eq!(
        ncl_object::code_constants(&ctx, roots[4].into()),
        Ok(roots[2])
    );
    assert_eq!(
        ncl_object::code_stack_map(&ctx, roots[4].into()),
        Ok(roots[10])
    );
    assert_eq!(ncl_object::code_debug(&ctx, roots[4].into()), Ok(roots[12]));
    assert_eq!(string_ref(&ctx, roots[10], 3), Ok('E'));
    assert_eq!(symbol_name(&ctx, roots[11]), Ok(roots[10]));
    assert_eq!(simple_vector_ref(&ctx, roots[12], 1), Ok(Word::fixnum(22)));
    assert_eq!(
        ncl_object::specialized_array_ref(&ctx, roots[13], 1),
        Ok(Word::fixnum(32))
    );
    assert_eq!(ncl_object::function_entry(&ctx, roots[14].into()), Ok(8));
    assert_eq!(ncl_object::double_value(&ctx, roots[3].into()), Ok(1.25));
    assert_eq!(
        ncl_object::ratio_numerator(&ctx, roots[7].into()),
        Ok(roots[2])
    );
    assert_eq!(
        ncl_object::complex_real(&ctx, roots[8].into()),
        Ok(roots[7])
    );
    assert_eq!(
        ncl_object::readtable_syntax(&ctx, roots[9].into()),
        Ok(roots[8])
    );
    assert_eq!(
        ncl_object::stream_state(&ctx, roots[6].into()),
        Ok(Word::fixnum(9))
    );
    for token in tokens.into_iter().rev() {
        assert!(ncl_object::pop_root(&mut ctx, token));
    }
}
