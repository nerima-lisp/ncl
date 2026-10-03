#![allow(clippy::unwrap_used, reason = "tests assert on remaining coverage")]
#![allow(missing_docs)]

use std::error::Error;

use ncl_object::{
    FunctionObject, Package, Runtime, ThreadContext, Word, make_bignum_from_i128, make_double,
    make_ratio, make_string,
};
use ncl_types::{TypeError, TypeSpecifier, Value, serialize_value, typep};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_types::register(&runtime).unwrap();
    ncl_types::builtins::register(&runtime).unwrap();
    (runtime, ctx)
}

fn intern(ctx: &mut ThreadContext, runtime: &Runtime, package: &str, name: &str) -> Word {
    let package = runtime.find_package(ctx, package).unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

#[test]
fn serialize_value_handles_each_supported_variant_and_rejects_opaque() {
    let (runtime, mut ctx) = setup();
    let symbol =
        serialize_value(&mut ctx, &runtime, &Value::Symbol("mixedCase".to_owned())).unwrap();
    assert_eq!(
        symbol,
        intern(&mut ctx, &runtime, "COMMON-LISP", "mixedCase")
    );

    let string = serialize_value(&mut ctx, &runtime, &Value::String("hello".to_owned())).unwrap();
    assert_eq!(ncl_object::string_length(&ctx, string).unwrap(), 5);
    assert_eq!(
        serialize_value(&mut ctx, &runtime, &Value::Nil).unwrap(),
        Word::NIL
    );
    assert_eq!(
        serialize_value(&mut ctx, &runtime, &Value::True).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        serialize_value(&mut ctx, &runtime, &Value::Integer(-9)).unwrap(),
        Word::fixnum(-9)
    );
    assert_eq!(
        serialize_value(&mut ctx, &runtime, &Value::Character(955)).unwrap(),
        Word::character(955)
    );
    assert_eq!(
        serialize_value(&mut ctx, &runtime, &Value::Opaque(0x1234)),
        Err(TypeError::CannotSerialize)
    );
}

#[test]
fn adapter_normalizes_symbol_and_string_values_for_type_queries() {
    let (runtime, mut ctx) = setup();
    let symbol = intern(&mut ctx, &runtime, "COMMON-LISP", "MiXeD");
    let string = make_string(&mut ctx, &runtime, &['h', 'i']).unwrap();
    let symbol_member = TypeSpecifier::Member(vec![Value::Symbol("MIXED".to_owned())]);
    let string_member = TypeSpecifier::Member(vec![Value::String("hi".to_owned())]);

    assert!(typep(&mut ctx, symbol, &symbol_member).unwrap());
    assert!(typep(&mut ctx, string, &string_member).unwrap());
    assert!(
        !typep(
            &mut ctx,
            string,
            &TypeSpecifier::Member(vec![Value::String("HI".to_owned())])
        )
        .unwrap()
    );
}

#[test]
fn type_errors_display_source_and_convert_between_layers() {
    let invalid = TypeError::InvalidSpecifier(Word::fixnum(7));
    assert_eq!(invalid.to_string(), "invalid type specifier: Word(0xe)");
    assert_eq!(
        TypeError::InvalidForm.to_string(),
        "invalid type specifier form"
    );
    assert_eq!(
        TypeError::CannotInvoke("PREDICATE".to_owned()).to_string(),
        "cannot invoke predicate: PREDICATE"
    );
    assert_eq!(
        TypeError::UnexpandedDeftype("CUSTOM".to_owned()).to_string(),
        "unexpanded deftype: CUSTOM"
    );
    assert_eq!(
        TypeError::CannotSerialize.to_string(),
        "cannot serialize type-specifier value"
    );
    let object_error = TypeError::from(ncl_object::ObjectError::TypeError);
    assert!(object_error.source().is_some());
    assert_eq!(
        ncl_object::ObjectError::from(TypeError::InvalidForm),
        ncl_object::ObjectError::TypeError
    );
}

#[test]
fn coerce_handles_bignum_ratio_and_function_designators() {
    let (runtime, mut ctx) = setup();
    let coerce =
        FunctionObject::try_from(runtime.function(&mut ctx, "COMMON-LISP", "COERCE").unwrap())
            .unwrap();
    let float = intern(&mut ctx, &runtime, "COMMON-LISP", "DOUBLE-FLOAT");
    let function = intern(&mut ctx, &runtime, "COMMON-LISP", "FUNCTION");
    let type_of = intern(&mut ctx, &runtime, "COMMON-LISP", "TYPE-OF");
    let bignum: Word = make_bignum_from_i128(&mut ctx, &runtime, -((1_i128 << 40) + 3))
        .unwrap()
        .into();
    let ratio: Word = make_ratio(&mut ctx, &runtime, Word::fixnum(3), Word::fixnum(2))
        .unwrap()
        .into();

    let bignum_float = runtime
        .call_builtin(&mut ctx, coerce, &[bignum, float])
        .unwrap();
    let ratio_float = runtime
        .call_builtin(&mut ctx, coerce, &[ratio, float])
        .unwrap();
    assert_eq!(
        ncl_object::double_value(&ctx, ncl_object::DoubleFloat::from_word(bignum_float)).unwrap(),
        -(1099511627776.0 + 3.0)
    );
    assert_eq!(
        ncl_object::double_value(&ctx, ncl_object::DoubleFloat::from_word(ratio_float)).unwrap(),
        1.5
    );

    let function_word = runtime
        .call_builtin(&mut ctx, coerce, &[type_of, function])
        .unwrap();
    assert!(matches!(
        ncl_object::classify_object(&ctx, function_word),
        ncl_object::ObjectRef::Function(_)
    ));
    let direct_function = runtime
        .function(&mut ctx, "COMMON-LISP", "TYPE-OF")
        .unwrap();
    assert_eq!(
        runtime
            .call_builtin(&mut ctx, coerce, &[direct_function, function])
            .unwrap(),
        direct_function
    );
    let double = make_double(&mut ctx, &runtime, 4.25).unwrap();
    assert_eq!(
        runtime
            .call_builtin(&mut ctx, coerce, &[double.into(), float])
            .unwrap(),
        double.into()
    );
}
