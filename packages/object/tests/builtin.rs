#![allow(missing_docs)]

use ncl_object::package::Package;
use ncl_object::typed::FunctionDesignator;
use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinFunctionCaller, BuiltinIdentifier,
    BuiltinImplementation, BuiltinName, BuiltinPackage, FunctionArguments, FunctionCaller,
    LambdaList, LispError, MultipleValues, ObjectError, Parameter, ParameterType, ProgramError,
    Runtime, ThreadContext, make_cons,
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

#[allow(clippy::unnecessary_wraps)]
fn arity_error_converter(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    error: LispError,
) -> Result<Word, ObjectError> {
    match error {
        LispError::ProgramError(ProgramError::WrongNumberOfArguments { minimum, maximum }) => {
            let encoded = minimum
                .checked_mul(10)
                .and_then(|value| value.checked_add(maximum.unwrap_or(0)))
                .and_then(|value| i64::try_from(value).ok())
                .ok_or(ObjectError::Layout)?;
            Ok(Word::fixnum(encoded))
        }
        _ => panic!("unexpected Lisp error: {error:?}"),
    }
}

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
        ncl_object::function_entry(&ctx, ncl_object::Function::from_word(function.as_word())),
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
#[test]
fn builtin_function_caller_preserves_multiple_values() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let descriptor = Builtin {
        lambda_list: LambdaList::new(REQUIRED_PARAMETERS, &[], None, &[], false),
        convention: BuiltinConvention::Direct(Arity::exact(2)),
    };
    let function = runtime
        .register_builtin(
            &mut ctx,
            TEST_ID,
            BuiltinImplementation::direct(descriptor, add_builtin),
        )
        .unwrap_or_else(|error| panic!("builtin: {error:?}"));
    let mut caller = BuiltinFunctionCaller;
    let mut values = MultipleValues::new();
    let result = caller.call_function(
        &mut ctx,
        &runtime,
        FunctionDesignator::Function(function),
        FunctionArguments::new(&[Word::fixnum(2), Word::fixnum(3)]),
        &mut values,
    );
    assert_eq!(result, Ok(Word::fixnum(5)));
    assert_eq!(values.as_slice(), &[Word::fixnum(99), Word::fixnum(100)]);
}

#[test]
fn runtime_registers_and_executes_keyword_builtins() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let keyword_package = runtime
        .find_package(&ctx, "KEYWORD")
        .unwrap_or_else(|| panic!("KEYWORD package missing"));
    let (keyword, _) = Package::from_word(keyword_package)
        .intern(&mut ctx, &runtime, "VALUE")
        .unwrap_or_else(|error| panic!("intern keyword: {error:?}"));
    let value = make_cons(&mut ctx, &runtime, Word::fixnum(42), Word::NIL)
        .unwrap_or_else(|error| panic!("make value tail: {error:?}"));
    let arguments = make_cons(&mut ctx, &runtime, keyword, value)
        .unwrap_or_else(|error| panic!("make argument list: {error:?}"));

    let value = runtime
        .function(&mut ctx, "NCL-EXT", "KEYWORD-VALUE")
        .and_then(|word| ncl_object::FunctionObject::try_from(word).ok())
        .unwrap_or_else(|| panic!("KEYWORD-VALUE not registered"));
    let supplied = runtime
        .function(&mut ctx, "NCL-EXT", "KEYWORD-SUPPLIED-P")
        .and_then(|word| ncl_object::FunctionObject::try_from(word).ok())
        .unwrap_or_else(|| panic!("KEYWORD-SUPPLIED-P not registered"));
    assert_eq!(
        runtime.call_builtin(&mut ctx, value, &[arguments, keyword]),
        Ok(Word::fixnum(42))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, supplied, &[arguments, keyword]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, value, &[Word::NIL, keyword]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, supplied, &[Word::NIL, keyword]),
        Ok(Word::NIL)
    );
}

#[test]
fn keyword_checker_applies_lambda_and_call_allow_other_keys_rules() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let keyword_package = runtime
        .find_package(&ctx, "KEYWORD")
        .unwrap_or_else(|| panic!("KEYWORD package missing"));
    let (known, _) = Package::from_word(keyword_package)
        .intern(&mut ctx, &runtime, "KNOWN")
        .unwrap_or_else(|error| panic!("intern known keyword: {error:?}"));
    let (unknown, _) = Package::from_word(keyword_package)
        .intern(&mut ctx, &runtime, "UNKNOWN")
        .unwrap_or_else(|error| panic!("intern unknown keyword: {error:?}"));
    let (allow, _) = Package::from_word(keyword_package)
        .intern(&mut ctx, &runtime, "ALLOW-OTHER-KEYS")
        .unwrap_or_else(|error| panic!("intern allow keyword: {error:?}"));
    let checker = runtime
        .function(&mut ctx, "NCL-EXT", "CHECK-KEYWORDS")
        .and_then(|word| ncl_object::FunctionObject::try_from(word).ok())
        .unwrap_or_else(|| panic!("CHECK-KEYWORDS not registered"));
    let known_value = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL)
        .unwrap_or_else(|error| panic!("make known value: {error:?}"));
    let known_arguments = make_cons(&mut ctx, &runtime, known, known_value)
        .unwrap_or_else(|error| panic!("make known arguments: {error:?}"));
    let unknown_value = make_cons(&mut ctx, &runtime, Word::fixnum(2), Word::NIL)
        .unwrap_or_else(|error| panic!("make unknown value: {error:?}"));
    let unknown_arguments = make_cons(&mut ctx, &runtime, unknown, unknown_value)
        .unwrap_or_else(|error| panic!("make unknown arguments: {error:?}"));
    let allow_value = make_cons(&mut ctx, &runtime, Word::TRUE, unknown_arguments)
        .unwrap_or_else(|error| panic!("make allow value: {error:?}"));
    let allow_arguments = make_cons(&mut ctx, &runtime, allow, allow_value)
        .unwrap_or_else(|error| panic!("make allow arguments: {error:?}"));
    let allow_and_unknown = allow_arguments;
    let odd_arguments = make_cons(&mut ctx, &runtime, known, Word::NIL)
        .unwrap_or_else(|error| panic!("make odd arguments: {error:?}"));

    assert_eq!(
        runtime.call_builtin(&mut ctx, checker, &[known_arguments, Word::NIL, known]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, checker, &[unknown_arguments, Word::NIL, known]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, checker, &[unknown_arguments, Word::TRUE, known]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, checker, &[allow_and_unknown, Word::NIL, known]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, checker, &[odd_arguments, Word::NIL, known]),
        Err(ncl_object::ObjectError::TypeError)
    );
}
#[test]
fn builtin_arity_mismatch_records_a_program_error() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    runtime.register_lisp_error_converter(arity_error_converter);
    let descriptor = Builtin {
        lambda_list: LambdaList::new(REQUIRED_PARAMETERS, &[], None, &[], false),
        convention: BuiltinConvention::Direct(Arity::exact(2)),
    };
    let function = runtime
        .register_builtin(
            &mut ctx,
            TEST_ID,
            BuiltinImplementation::direct(descriptor, add_builtin),
        )
        .unwrap_or_else(|error| panic!("builtin: {error:?}"));

    assert_eq!(
        runtime.call_builtin(&mut ctx, function, &[Word::fixnum(2)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(ctx.take_pending_condition(), Some(Word::fixnum(22)));
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            function,
            &[Word::fixnum(1), Word::fixnum(2), Word::fixnum(3)],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(ctx.take_pending_condition(), Some(Word::fixnum(22)));
}
