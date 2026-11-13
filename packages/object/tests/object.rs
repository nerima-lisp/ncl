//! Integration tests for object classification, allocation, and accessors.

use ncl_object::{
    ObjectRef, Package, Runtime, ThreadContext, builtin, car, classify, classify_object, make_cons,
    make_simple_vector, make_string, make_symbol, register, set_symbol_value, simple_vector_ref,
    string_ref, symbol_name, symbol_value,
};
use ncl_sys::Word;
#[test]
fn classify_and_allocate() {
    assert_eq!(classify(Word::fixnum(-2)), ObjectRef::Fixnum(-2));
    assert_eq!(classify(Word::NIL), ObjectRef::Symbol(Word::NIL));
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    assert!(make_cons(&mut ctx, &runtime, Word::NIL, Word::NIL).is_err());
    assert!(ctx.register(&runtime).is_ok());
    let cons = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL).unwrap_or(Word::NIL);
    assert_eq!(car(&ctx, cons), Ok(Word::fixnum(1)));
}
#[test]
fn symbols_and_bindings() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
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
    assert_eq!(
        TEST_BUILTIN.convention.arity(),
        Some(ncl_object::Arity::exact(2))
    );
}
#[test]
fn builtin_metadata_expands_with_lambda_list() {
    builtin!(
        TEST_ABI,
        1,
        ncl_object::LambdaList::new(
            &[ncl_object::Parameter {
                name: ncl_object::BuiltinName::new("A"),
                ty: ncl_object::ParameterType::Any,
            }],
            &[],
            None,
            &[],
            false,
        )
    );
    assert_eq!(
        TEST_ABI.convention.arity(),
        Some(ncl_object::Arity::exact(1))
    );
    const { assert!(TEST_ABI.convention.direct()) };
    assert!(TEST_ABI.lambda_list.is_direct());
    assert_eq!(TEST_ABI.lambda_list.min_arity(), 1);
}

#[test]
fn gc_extensions_register_the_owned_symbols() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    assert!(register(&mut ctx, &runtime).is_ok());
    for name in [
        "CANCEL-FINALIZATION",
        "FINALIZE",
        "GENERATION-BYTES-ALLOCATED",
        "HASH-TABLE-WEAKNESS",
        "MAKE-WEAK-POINTER",
        "MAKE-WEAK-VECTOR",
        "WEAK-POINTER-P",
        "WEAK-POINTER-VALUE",
        "WEAK-VECTOR-P",
    ] {
        assert_eq!(
            runtime.function(&mut ctx, "NCL-GC", name),
            Some(Word::UNBOUND)
        );
    }
    assert_ne!(
        runtime.function(&mut ctx, "NCL-GC", "GC"),
        Some(Word::UNBOUND)
    );
    assert_eq!(runtime.class(&mut ctx, "WEAK-POINTER"), Some(Word::UNBOUND));
}

#[test]
fn gc_preserves_object_accessors_and_weak_entries() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    assert!(runtime.register_layouts().is_ok());
    assert!(
        runtime
            .register_layout(
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
    let package = Package::new(&mut ctx, &runtime, "GC-TEST")
        .unwrap_or_else(|error| panic!("Package allocation failed: {error:?}"));
    let mut package_word = package.as_word();
    let package_token = ncl_object::push_root(&mut ctx, &mut package_word);
    let package = Package::from_word(package_word);
    let interned = package
        .intern(&mut ctx, &runtime, "SAME")
        .map_or(Word::NIL, |pair| pair.0);
    let weak = make_weak_cons(&mut ctx, &runtime);
    let mut roots = symbols;
    roots.push(list);
    roots.push(interned);
    let weak_root_index = roots.len();
    roots.push(weak);
    let tokens = roots
        .iter_mut()
        .map(|value| ncl_object::push_root(&mut ctx, value))
        .collect::<Vec<_>>();
    assert_eq!(runtime.widetag(roots[1_001]), Some(1));
    assert!(ctx.collect(false).is_ok());
    assert_eq!(car(&ctx, roots[1_000]), Ok(Word::fixnum(9_999)));
    assert_eq!(runtime.widetag(roots[1_001]), Some(1));
    assert_eq!(symbol_value(&ctx, roots[1_001]), Ok(Word::UNBOUND));
    let package = Package::from_word(package_word);
    assert_eq!(
        package
            .intern(&mut ctx, &runtime, "SAME")
            .map(|pair| pair.0),
        Ok(roots[1_001])
    );

    assert!(ctx.collect(true).is_ok());
    assert_eq!(car(&ctx, roots[1_000]), Ok(Word::fixnum(9_999)));
    assert_eq!(ctx.weak_value(roots[weak_root_index]), Word::NIL);
    for token in tokens.into_iter().rev() {
        assert!(ncl_object::pop_root(&mut ctx, token));
    }
    assert!(ncl_object::pop_root(&mut ctx, package_token));
}

fn make_weak_cons(ctx: &mut ThreadContext, runtime: &Runtime) -> Word {
    let mut weak = ncl_object::allocate(ctx, runtime, 99, 1).unwrap_or(Word::NIL);
    let weak_token = ncl_object::push_root(ctx, &mut weak);
    let referent = make_cons(ctx, runtime, Word::fixnum(7), Word::NIL).unwrap_or(Word::NIL);
    assert!(ctx.write_object_slot(weak, 0, referent).is_ok());
    weak = ctx.make_weak(weak, ncl_sys::Weakness::Value);
    assert!(ncl_object::pop_root(ctx, weak_token));
    weak
}

#[test]
fn strings_and_simple_vectors_have_typed_accessors() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
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
#[expect(
    clippy::too_many_lines,
    reason = "This round-trip test checks the object kinds and their rooted values together."
)]
fn remaining_object_kinds_round_trip() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    assert!(runtime.register_layouts().is_ok());
    let name = make_string(&mut ctx, &runtime, &['N', 'A', 'M', 'E']).unwrap_or(Word::NIL);
    let symbol = make_symbol(&mut ctx, &runtime, name).unwrap_or(Word::NIL);
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(21), Word::fixnum(22)])
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
        .unwrap_or_else(|_| ncl_object::Instance::from_word(Word::NIL));
    assert_eq!(ncl_object::slot_ref(&ctx, instance, 0), Ok(Word::fixnum(3)));
    assert!(ncl_object::slot_set(&mut ctx, instance, 0, Word::fixnum(4)).is_ok());
    let bignum = ncl_object::make_bignum_from_i128(&mut ctx, &runtime, -0x1_0000_0001)
        .unwrap_or_else(|_| ncl_object::Bignum::from_word(Word::NIL));
    assert_eq!(ncl_object::bignum_limbs(&ctx, bignum), Ok(vec![1, 1]));
    let double = ncl_object::make_double(&mut ctx, &runtime, 1.25)
        .unwrap_or_else(|_| ncl_object::DoubleFloat::from_word(Word::NIL));
    assert_eq!(ncl_object::double_value(&ctx, double), Ok(1.25));
    let code = ncl_object::make_code_object(&mut ctx, &runtime, 7, 4, bignum.into(), name, vector)
        .unwrap_or_else(|_| ncl_object::CodeObject::from_word(Word::NIL));
    let simple_fun = ncl_object::make_simple_fun(&mut ctx, &runtime, 8, name, vector, code)
        .unwrap_or_else(|_| ncl_object::Function::from_word(Word::NIL));
    let function = ncl_object::make_closure(
        &mut ctx,
        &runtime,
        7,
        Word::NIL,
        Word::NIL,
        code,
        &[instance.into(), bignum.into(), double.into()],
    )
    .unwrap_or_else(|_| ncl_object::Function::from_word(Word::NIL));
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
    let stream = ncl_streams::make_stream(
        &mut ctx,
        &runtime,
        Word::fixnum(1),
        Word::NIL,
        Word::NIL,
        Word::fixnum(9),
        Word::NIL,
    )
    .unwrap_or_else(|_| ncl_streams::Stream::from_word(Word::NIL));
    assert_eq!(ncl_streams::stream_state(&ctx, stream), Ok(Word::fixnum(9)));
    let ratio = ncl_object::make_ratio(&mut ctx, &runtime, bignum.into(), double.into())
        .unwrap_or_else(|_| ncl_object::Ratio::from_word(Word::NIL));
    let complex = ncl_object::make_complex(&mut ctx, &runtime, ratio.into(), bignum.into())
        .unwrap_or_else(|_| ncl_object::Complex::from_word(Word::NIL));
    let readtable = ncl_readtables::make_readtable(
        &mut ctx,
        &runtime,
        complex.into(),
        ratio.into(),
        Word::fixnum(2),
    )
    .unwrap_or_else(|_| ncl_readtables::Readtable::from_word(Word::NIL));
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
    assert!(ctx.collect(false).is_ok());
    assert_eq!(classify_object(&ctx, roots[2]), ObjectRef::Bignum(roots[2]));
    assert!(ctx.collect(true).is_ok());
    assert_eq!(
        ncl_object::bignum_limbs(&ctx, ncl_object::Bignum::from_word(roots[2])),
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
        ncl_object::slot_ref(&ctx, ncl_object::Instance::from_word(roots[1]), 0),
        Ok(Word::fixnum(4))
    );
    assert_eq!(
        ncl_object::closure_ref(&ctx, ncl_object::Function::from_word(roots[5]), 0),
        Ok(roots[1])
    );
    assert_eq!(
        ncl_object::closure_ref(&ctx, ncl_object::Function::from_word(roots[5]), 1),
        Ok(roots[2])
    );
    assert_eq!(
        ncl_object::closure_ref(&ctx, ncl_object::Function::from_word(roots[5]), 2),
        Ok(roots[3])
    );
    assert_eq!(
        ncl_object::code_entry(&ctx, ncl_object::CodeObject::from_word(roots[4])),
        Ok(Word::fixnum(7))
    );
    assert_eq!(
        ncl_object::code_size(&ctx, ncl_object::CodeObject::from_word(roots[4])),
        Ok(Word::fixnum(4))
    );
    assert_eq!(
        ncl_object::code_constants(&ctx, ncl_object::CodeObject::from_word(roots[4])),
        Ok(roots[2])
    );
    assert_eq!(
        ncl_object::code_stack_map(&ctx, ncl_object::CodeObject::from_word(roots[4])),
        Ok(roots[10])
    );
    let code4 = ncl_object::CodeObject::from_word(roots[4]);
    assert_eq!(ncl_object::code_debug(&ctx, code4), Ok(roots[12]));
    assert_eq!(string_ref(&ctx, roots[10], 3), Ok('E'));
    assert_eq!(symbol_name(&ctx, roots[11]), Ok(roots[10]));
    assert_eq!(simple_vector_ref(&ctx, roots[12], 1), Ok(Word::fixnum(22)));
    let function13 = ncl_object::Function::from_word(roots[13]);
    assert_eq!(ncl_object::function_entry(&ctx, function13), Ok(8));
    let double3 = ncl_object::DoubleFloat::from_word(roots[3]);
    assert_eq!(ncl_object::double_value(&ctx, double3), Ok(1.25));
    assert_eq!(
        ncl_object::ratio_numerator(&ctx, ncl_object::Ratio::from_word(roots[7])),
        Ok(roots[2])
    );
    assert_eq!(
        ncl_object::complex_real(&ctx, ncl_object::Complex::from_word(roots[8])),
        Ok(roots[7])
    );
    assert_eq!(
        ncl_readtables::readtable_syntax(&ctx, ncl_readtables::Readtable::from_word(roots[9])),
        Ok(roots[8])
    );
    assert_eq!(
        ncl_streams::stream_state(&ctx, ncl_streams::Stream::from_word(roots[6])),
        Ok(Word::fixnum(9))
    );
    for token in tokens.into_iter().rev() {
        assert!(ncl_object::pop_root(&mut ctx, token));
    }
}
