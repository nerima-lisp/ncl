#![allow(missing_docs)]

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, LambdaList, MultipleValues, Parameter, ParameterType, Runtime,
    ThreadContext,
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
    BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("ADD"));
const ADAPTED_ID: BuiltinIdentifier =
    BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("ADAPTED"));

const REQUIRED_PARAMETERS: &[Parameter] = &[
    Parameter {
        name: BuiltinName::new("LEFT"),
        ty: ParameterType::Fixnum,
    },
    Parameter {
        name: BuiltinName::new("RIGHT"),
        ty: ParameterType::Fixnum,
    },
];
const KEY_PARAMETERS: &[Parameter] = &[
    Parameter {
        name: BuiltinName::new("LEFT"),
        ty: ParameterType::Fixnum,
    },
    Parameter {
        name: BuiltinName::new("RIGHT"),
        ty: ParameterType::Fixnum,
    },
];

#[test]
fn registered_builtin_has_a_function_object_and_rust_call_boundary() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let descriptor = Builtin {
        lambda_list: LambdaList::new(REQUIRED_PARAMETERS, &[], None, &[], false),
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
        ncl_object::FunctionObject::try_from(function.as_word()),
        Ok(function)
    );
    assert_eq!(
        ncl_object::FunctionObject::try_from(Word::NIL),
        Err(ncl_object::ObjectError::TypeError)
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
        lambda_list: LambdaList::new(&[], &[], None, KEY_PARAMETERS, false),
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

fn callback(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    _args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ncl_object::ObjectError> {
    Ok(Word::NIL)
}

extern "C" fn native_entry(_ctx: *mut ThreadContext, _left: Word, _right: Word) -> Word {
    Word::NIL
}

#[test]
fn registered_builtin_address_uses_native_entry() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    let descriptor = Builtin {
        lambda_list: LambdaList::fixed(&[]),
        convention: BuiltinConvention::Direct(Arity::exact(2)),
    };
    let native_address = native_entry as *const () as usize;
    let implementation =
        BuiltinImplementation::direct(descriptor, callback).with_entry(native_address);
    runtime
        .register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("ADD")),
            implementation,
        )
        .unwrap_or_else(|error| panic!("builtin: {error:?}"));

    assert_eq!(
        runtime.builtin_address("NCL-TEST::ADD"),
        Some(native_address as u64)
    );
}
