#![allow(missing_docs)]

use ncl_object::typed::FunctionDesignator;
use ncl_object::{
    make_code_object, make_simple_fun, Arity, Builtin, BuiltinArgs, BuiltinConvention,
    BuiltinFunctionCaller, BuiltinIdentifier, BuiltinImplementation, BuiltinName, BuiltinPackage,
    FunctionArguments, FunctionCaller, LambdaList, MultipleValues, ObjectError, Runtime,
    ThreadContext,
};
use ncl_sys::Word;

const TEST_ID: BuiltinIdentifier =
    BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("BOUNDARY"));

fn descriptor() -> Builtin {
    Builtin {
        lambda_list: LambdaList::new(&[], &[], None, &[], false),
        convention: BuiltinConvention::Direct(Arity::exact(0)),
    }
}

fn returns_error(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    _args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Err(ObjectError::Unsupported)
}

fn sets_pending_error(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    _args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    ctx.set_pending(ObjectError::Unsupported);
    Ok(Word::fixnum(17))
}

fn registered_caller() -> (Runtime, ThreadContext, BuiltinFunctionCaller) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register failed: {error:?}"));
    (runtime, ctx, BuiltinFunctionCaller)
}

#[test]
fn builtin_function_caller_propagates_callback_and_pending_errors() {
    let (runtime, mut ctx, mut caller) = registered_caller();
    let failing = runtime
        .register_builtin(
            &mut ctx,
            TEST_ID,
            BuiltinImplementation::direct(descriptor(), returns_error),
        )
        .unwrap_or_else(|error| panic!("register builtin failed: {error:?}"));
    let mut values = MultipleValues::new();
    assert_eq!(
        caller.call_function(
            &mut ctx,
            &runtime,
            FunctionDesignator::Function(failing),
            FunctionArguments::new(&[]),
            &mut values,
        ),
        Err(ObjectError::Unsupported)
    );

    let pending = runtime
        .register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("PENDING")),
            BuiltinImplementation::direct(descriptor(), sets_pending_error),
        )
        .unwrap_or_else(|error| panic!("register builtin failed: {error:?}"));
    assert_eq!(
        caller.call_function(
            &mut ctx,
            &runtime,
            FunctionDesignator::Function(pending),
            FunctionArguments::new(&[]),
            &mut values,
        ),
        Err(ObjectError::Unsupported)
    );
    assert_eq!(ctx.take_pending(), None);
}

#[test]
fn builtin_function_caller_turns_an_existing_pending_error_into_control_error() {
    let (runtime, mut ctx, mut caller) = registered_caller();
    let function = runtime
        .register_builtin(
            &mut ctx,
            TEST_ID,
            BuiltinImplementation::direct(descriptor(), returns_error),
        )
        .unwrap_or_else(|error| panic!("register builtin failed: {error:?}"));
    ctx.set_pending(ObjectError::Unsupported);
    let mut values = MultipleValues::new();
    assert_eq!(
        caller.call_function(
            &mut ctx,
            &runtime,
            FunctionDesignator::Function(function),
            FunctionArguments::new(&[]),
            &mut values,
        ),
        Err(ObjectError::ControlError)
    );
    assert_eq!(ctx.take_pending(), Some(ObjectError::ControlError));
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
fn native_return_code(value: u64, count: u32) -> ncl_sys::CodePtr {
    #[cfg(target_arch = "x86_64")]
    let bytes = {
        let mut bytes = vec![0x48, 0xb8];
        bytes.extend_from_slice(&value.to_le_bytes());
        bytes.push(0xba);
        bytes.extend_from_slice(&count.to_le_bytes());
        bytes.push(0xc3);
        bytes
    };
    #[cfg(target_arch = "aarch64")]
    let bytes = {
        assert!(value <= u64::from(u16::MAX));
        [
            0xd2800000_u32 | ((value as u32) << 5),
            0xd2800000_u32 | (count << 5) | 1,
            0xd65f03c0,
        ]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>()
    };
    let mut code = ncl_sys::alloc_code(bytes.len())
        .unwrap_or_else(|error| panic!("alloc native code failed: {error:?}"));
    code.write_code(0, &bytes)
        .unwrap_or_else(|error| panic!("write native code failed: {error:?}"));
    ncl_sys::publish_code(&mut code)
        .unwrap_or_else(|error| panic!("publish native code failed: {error:?}"));
    code
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
fn native_function(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    code: &ncl_sys::CodePtr,
) -> ncl_object::FunctionObject {
    let descriptor = make_code_object(
        ctx,
        runtime,
        code.address(),
        code.len(),
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap_or_else(|error| panic!("code object failed: {error:?}"));
    let function = make_simple_fun(
        ctx,
        runtime,
        code.address(),
        Word::NIL,
        Word::NIL,
        descriptor,
    )
    .unwrap_or_else(|error| panic!("function failed: {error:?}"));
    ncl_object::FunctionObject::try_from(function.as_word())
        .unwrap_or_else(|error| panic!("function object failed: {error:?}"))
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[test]
fn native_function_call_checks_multiple_value_area_and_non_local_exit() {
    let (runtime, mut ctx, mut caller) = registered_caller();
    let code = native_return_code(Word::fixnum(7).bits(), 21);
    let function = native_function(&mut ctx, &runtime, &code);
    let mut values = MultipleValues::new();
    assert_eq!(
        caller.call_function(
            &mut ctx,
            &runtime,
            FunctionDesignator::Function(function),
            FunctionArguments::new(&[]),
            &mut values,
        ),
        Err(ObjectError::Layout)
    );

    ctx.thread_mut()
        .set_multiple_value_area(&[Word::fixnum(21), Word::fixnum(22)]);
    ctx.set_non_local_exit(true);
    assert_eq!(
        caller.call_function(
            &mut ctx,
            &runtime,
            FunctionDesignator::Function(function),
            FunctionArguments::new(&[]),
            &mut values,
        ),
        Err(ObjectError::NonLocalExit)
    );
    assert_eq!(values.len(), 20);
    assert_eq!(
        &values.as_slice()[..2],
        &[Word::fixnum(21), Word::fixnum(22)]
    );
    assert!(ctx.is_unwinding());
    assert!(ctx.take_non_local_exit());
    assert!(!ctx.is_unwinding());
}

#[test]
fn malformed_function_entry_is_reported_through_function_caller() {
    let (runtime, mut ctx, mut caller) = registered_caller();
    let code = make_code_object(&mut ctx, &runtime, 0, 0, Word::NIL, Word::NIL, Word::NIL)
        .unwrap_or_else(|error| panic!("code object failed: {error:?}"));
    let function = make_simple_fun(&mut ctx, &runtime, 0, Word::NIL, Word::NIL, code)
        .unwrap_or_else(|error| panic!("function failed: {error:?}"));
    let mut values = MultipleValues::new();
    assert_eq!(
        caller.call_function(
            &mut ctx,
            &runtime,
            FunctionDesignator::Function(
                ncl_object::FunctionObject::try_from(function.as_word()).unwrap(),
            ),
            FunctionArguments::new(&[]),
            &mut values,
        ),
        Err(ObjectError::Layout)
    );
}
