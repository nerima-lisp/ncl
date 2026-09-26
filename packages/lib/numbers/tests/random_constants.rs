#![allow(clippy::unwrap_used, missing_docs)]

use ncl_object::{
    Bignum, DoubleFloat, FunctionObject, ObjectError, ObjectRef, Package, Runtime, ThreadContext,
    Word, bignum_limbs, bignum_sign, classify_object, double_value, symbol_is_constant,
    symbol_is_special, symbol_value,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_numbers::register(&runtime).unwrap();
    (runtime, ctx)
}

fn function(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap()
}

fn call(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let function = function(runtime, ctx, name);
    runtime.call_builtin(ctx, function, args)
}

fn common_lisp_symbol(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Word {
    let package = runtime.find_package(ctx, "COMMON-LISP").unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn float(ctx: &ThreadContext, value: Word) -> f64 {
    let ObjectRef::DoubleFloat(value) = classify_object(ctx, value) else {
        panic!("expected double-float")
    };
    double_value(ctx, DoubleFloat::from_word(value)).unwrap()
}

fn integer(ctx: &ThreadContext, value: Word) -> i128 {
    match classify_object(ctx, value) {
        ObjectRef::Fixnum(value) => i128::from(value),
        ObjectRef::Bignum(value) => {
            let magnitude = bignum_limbs(ctx, Bignum::from_word(value))
                .unwrap()
                .into_iter()
                .enumerate()
                .fold(0_i128, |value, (index, limb)| {
                    value | (i128::from(limb) << (index * 32))
                });
            if bignum_sign(ctx, Bignum::from_word(value)).unwrap() {
                -magnitude
            } else {
                magnitude
            }
        }
        other => panic!("expected integer, got {other:?}"),
    }
}

#[test]
fn random_state_is_deterministic_and_cloneable() {
    let (runtime, mut ctx) = setup();
    let first = call(&runtime, &mut ctx, "MAKE-RANDOM-STATE", &[Word::NIL]).unwrap();
    let second = call(&runtime, &mut ctx, "MAKE-RANDOM-STATE", &[Word::NIL]).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "RANDOM-STATE-P", &[first]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "RANDOM-STATE-P", &[Word::NIL]),
        Ok(Word::NIL)
    );

    let first_value = call(&runtime, &mut ctx, "RANDOM", &[Word::fixnum(100), first]).unwrap();
    let second_value = call(&runtime, &mut ctx, "RANDOM", &[Word::fixnum(100), second]).unwrap();
    assert_eq!(first_value, second_value);

    let clone = call(&runtime, &mut ctx, "MAKE-RANDOM-STATE", &[first]).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "RANDOM", &[Word::fixnum(100), first]).unwrap(),
        call(&runtime, &mut ctx, "RANDOM", &[Word::fixnum(100), clone]).unwrap()
    );
}

#[test]
fn random_respects_integer_and_float_ranges() {
    let (runtime, mut ctx) = setup();
    let state = call(&runtime, &mut ctx, "MAKE-RANDOM-STATE", &[Word::NIL]).unwrap();
    for _ in 0..128 {
        let integer = call(&runtime, &mut ctx, "RANDOM", &[Word::fixnum(7), state]).unwrap();
        let ObjectRef::Fixnum(value) = classify_object(&ctx, integer) else {
            panic!("expected fixnum")
        };
        assert!((0..7).contains(&value));

        let limit = ncl_object::make_double(&mut ctx, &runtime, 3.5)
            .unwrap()
            .into();
        let value = call(&runtime, &mut ctx, "RANDOM", &[limit, state]).unwrap();
        assert!(float(&ctx, value) >= 0.0 && float(&ctx, value) < 3.5);
    }
}

#[test]
fn random_supports_bignum_limits_above_fixnum_range() {
    let (runtime, mut ctx) = setup();
    let state = call(&runtime, &mut ctx, "MAKE-RANDOM-STATE", &[Word::NIL]).unwrap();
    let limit = ncl_object::make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 63)
        .unwrap()
        .into();
    for _ in 0..16 {
        let value = call(&runtime, &mut ctx, "RANDOM", &[limit, state]).unwrap();
        assert!((0..(1_i128 << 63)).contains(&integer(&ctx, value)));
    }
}

#[test]
fn random_rejects_invalid_limits() {
    let (runtime, mut ctx) = setup();
    let state = call(&runtime, &mut ctx, "MAKE-RANDOM-STATE", &[Word::NIL]).unwrap();
    for limit in [Word::fixnum(0), Word::fixnum(-1), Word::NIL] {
        assert!(call(&runtime, &mut ctx, "RANDOM", &[limit, state]).is_err());
    }
}

#[test]
fn random_and_constants_survive_gc_stress_and_strict_forwarding() {
    let (runtime, mut ctx) = setup();
    let mut state = call(&runtime, &mut ctx, "MAKE-RANDOM-STATE", &[Word::NIL]).unwrap();
    let token = ncl_object::push_root(&mut ctx, &mut state);
    let state_p = function(&runtime, &mut ctx, "RANDOM-STATE-P");
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    assert_eq!(
        runtime.call_builtin(&mut ctx, state_p, &[state]),
        Ok(Word::TRUE)
    );
    assert!(ncl_object::pop_root(&mut ctx, token));

    let pi = common_lisp_symbol(&runtime, &mut ctx, "PI");
    let pi_value = symbol_value(&ctx, pi).unwrap();
    assert!(float(&ctx, pi_value).is_finite());
}

#[test]
fn numeric_constants_are_constant_and_bound() {
    let (runtime, mut ctx) = setup();
    for name in ["PI", "DOUBLE-FLOAT-EPSILON", "MOST-POSITIVE-FIXNUM"] {
        let symbol = common_lisp_symbol(&runtime, &mut ctx, name);
        assert!(symbol_is_constant(&ctx, symbol).unwrap(), "{name}");
        assert_ne!(symbol_value(&ctx, symbol).unwrap(), Word::UNBOUND, "{name}");
    }
    let random_state = common_lisp_symbol(&runtime, &mut ctx, "*RANDOM-STATE*");
    assert!(symbol_is_special(&ctx, random_state).unwrap());
    let state_value = symbol_value(&ctx, random_state).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "RANDOM-STATE-P", &[state_value]),
        Ok(Word::TRUE)
    );
    let pi = common_lisp_symbol(&runtime, &mut ctx, "PI");
    let pi_value = symbol_value(&ctx, pi).unwrap();
    assert!((float(&ctx, pi_value) - std::f64::consts::PI).abs() < f64::EPSILON);
}

#[test]
fn fixnum_constants_match_word_encoding_range() {
    let (runtime, mut ctx) = setup();
    for (name, expected) in [
        ("MOST-POSITIVE-FIXNUM", i64::MAX >> ncl_sys::FIXNUM_TAG_BITS),
        ("MOST-NEGATIVE-FIXNUM", i64::MIN >> ncl_sys::FIXNUM_TAG_BITS),
    ] {
        let symbol = common_lisp_symbol(&runtime, &mut ctx, name);
        assert_eq!(
            symbol_value(&ctx, symbol).unwrap().as_fixnum(),
            Some(expected)
        );
    }
}
