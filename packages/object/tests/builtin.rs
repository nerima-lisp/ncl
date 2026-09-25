#![allow(missing_docs)]

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, LambdaList, MultipleValues, Runtime, ThreadContext,
};
use ncl_sys::Word;

fn add_builtin(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ncl_object::ObjectError> {
    values.set(&[Word::fixnum(99), Word::fixnum(100)]);
    let left = args
        .required(0)?
        .as_fixnum()
        .ok_or(ncl_object::ObjectError::TypeError)?;
    let right = args
        .required(1)?
        .as_fixnum()
        .ok_or(ncl_object::ObjectError::TypeError)?;
    Ok(Word::fixnum(left + right))
}

fn keyword_adapter(args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ncl_object::ObjectError> {
    if args.len() == 2 {
        Ok(vec![args.required(1)?, args.required(0)?])
    } else {
        Err(ncl_object::ObjectError::TypeError)
    }
}

const TEST_ID: BuiltinIdentifier =
    BuiltinIdentifier::new(BuiltinPackage::new("NCL-TEST"), BuiltinName::new("ADD"));
const ADAPTED_ID: BuiltinIdentifier =
    BuiltinIdentifier::new(BuiltinPackage::new("NCL-TEST"), BuiltinName::new("ADAPTED"));

#[test]
fn registered_builtin_has_a_function_object_and_rust_call_boundary() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let descriptor = Builtin {
        lambda_list: LambdaList::new("left right"),
        convention: BuiltinConvention::Direct(Arity::exact(2)),
    };
    let implementation = BuiltinImplementation::direct(descriptor, add_builtin);
    let entry = implementation.entry;
    let function = runtime
        .register_builtin(&mut ctx, TEST_ID, implementation)
        .unwrap_or_else(|error| panic!("builtin: {error:?}"));
    assert_eq!(
        runtime.function(&mut ctx, "NCL-TEST", "ADD"),
        Some(function.as_word())
    );
    assert_eq!(
        ncl_object::function_entry(&ctx, function.as_word().into()),
        Ok(entry)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, function, &[Word::fixnum(2), Word::fixnum(3)]),
        Ok(Word::fixnum(5))
    );
    assert_eq!(ctx.values(), &[Word::fixnum(99), Word::fixnum(100)]);
}

#[test]
fn adapted_builtin_reorders_keyword_payload_before_rust_call() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let descriptor = Builtin {
        lambda_list: LambdaList::new("&key left right"),
        convention: BuiltinConvention::Adapted,
    };
    let function = runtime
        .register_builtin(
            &mut ctx,
            ADAPTED_ID,
            BuiltinImplementation::adapted(descriptor, add_builtin, keyword_adapter),
        )
        .unwrap_or_else(|error| panic!("builtin: {error:?}"));
    assert_eq!(
        runtime.call_builtin(&mut ctx, function, &[Word::fixnum(3), Word::fixnum(2)]),
        Ok(Word::fixnum(5))
    );
}
