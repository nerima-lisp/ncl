use super::RuntimeFunctionCaller;
use crate::Runtime;
use crate::support::NativeInvocation;
use ncl_object::typed::FunctionDesignator;
use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, Function, FunctionArguments, FunctionCaller, FunctionObject,
    LambdaList, MultipleValues, Package, Parameter, ParameterType, Word, function_code,
    make_closure, make_code_object, symbol_function,
};

const TEST_ID: BuiltinIdentifier =
    BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("RUNTIME-CALL"));
const EMPTY_VALUES_ID: BuiltinIdentifier = BuiltinIdentifier::new(
    BuiltinPackage::NclTest,
    BuiltinName::new("RUNTIME-EMPTY-VALUES"),
);
const NATIVE_ALLOCATING_ID: BuiltinIdentifier = BuiltinIdentifier::new(
    BuiltinPackage::NclTest,
    BuiltinName::new("RUNTIME-NATIVE-ALLOCATING"),
);
const PARAMETERS: &[Parameter] = &[
    Parameter {
        name: BuiltinName::new("LEFT"),
        ty: ParameterType::Fixnum,
    },
    Parameter {
        name: BuiltinName::new("RIGHT"),
        ty: ParameterType::Fixnum,
    },
];
const EMPTY_VALUES_PARAMETER: &[Parameter] = &[Parameter {
    name: BuiltinName::new("EMPTY"),
    ty: ParameterType::Any,
}];

fn add(
    _ctx: &mut ncl_object::ThreadContext,
    _runtime: &ncl_object::Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ncl_object::ObjectError> {
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

fn optional_empty_values(
    _ctx: &mut ncl_object::ThreadContext,
    _runtime: &ncl_object::Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ncl_object::ObjectError> {
    if args.required(0)? == Word::TRUE {
        values.set(&[]);
    }
    Ok(Word::fixnum(7))
}

fn allocating_builtin_observes_native_state(
    context: &mut ncl_object::ThreadContext,
    runtime: &ncl_object::Runtime,
    _args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ncl_object::ObjectError> {
    let entered_native = context.native_state() == ncl_sys::NativeState::Native;
    ncl_object::make_cons(context, runtime, Word::TRUE, Word::NIL)?;
    Ok(if entered_native {
        Word::TRUE
    } else {
        Word::NIL
    })
}

fn register_builtin(runtime: &mut Runtime) -> ncl_object::FunctionObject {
    let descriptor = Builtin {
        lambda_list: LambdaList::new(PARAMETERS, &[], None, &[], false),
        convention: BuiltinConvention::Direct(Arity::exact(2)),
    };
    runtime
        .object
        .register_builtin(
            &mut runtime.context,
            TEST_ID,
            BuiltinImplementation::direct(descriptor, add),
        )
        .unwrap_or_else(|error| panic!("register builtin: {error:?}"))
}

fn register_empty_values_builtin(runtime: &mut Runtime) -> ncl_object::FunctionObject {
    let descriptor = Builtin {
        lambda_list: LambdaList::new(EMPTY_VALUES_PARAMETER, &[], None, &[], false),
        convention: BuiltinConvention::Direct(Arity::exact(1)),
    };
    runtime
        .object
        .register_builtin(
            &mut runtime.context,
            EMPTY_VALUES_ID,
            BuiltinImplementation::direct(descriptor, optional_empty_values),
        )
        .unwrap_or_else(|error| panic!("register empty-values builtin: {error:?}"))
}

fn register_native_allocating_builtin(runtime: &mut Runtime) -> FunctionObject {
    let descriptor = Builtin {
        lambda_list: LambdaList::new(&[], &[], None, &[], false),
        convention: BuiltinConvention::Direct(Arity::exact(0)),
    };
    runtime
        .object
        .register_builtin(
            &mut runtime.context,
            NATIVE_ALLOCATING_ID,
            BuiltinImplementation::direct(descriptor, allocating_builtin_observes_native_state),
        )
        .unwrap_or_else(|error| panic!("register native allocating builtin: {error:?}"))
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
fn call_builtin_through_native_trampoline(
    runtime: &mut Runtime,
    function: FunctionObject,
    code: ncl_object::CodeObject,
    args: &mut [Word],
    values: &mut MultipleValues,
) -> Result<Word, ncl_object::ObjectError> {
    let object = &runtime.object;
    let entry_codes = &runtime.entry_codes;
    let context = &mut runtime.context;
    let thread = context.thread_access().as_non_null();
    let mut invocation = NativeInvocation {
        object,
        context,
        code,
        entry_codes,
    };
    let previous = ncl_sys::replace_native_context(
        thread,
        Some(std::ptr::NonNull::from(&mut invocation).cast()),
    );
    let result = super::call_native(
        invocation.context,
        invocation.object,
        function,
        args,
        values,
    );
    let _ = ncl_sys::replace_native_context(thread, previous);
    result
}

#[test]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
fn native_builtin_trampoline_publishes_values_within_the_abi_limit() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let callback = register_empty_values_builtin(&mut runtime);
    let callback_code = function_code(&runtime.context, Function::from_word(callback.as_word()))
        .unwrap_or_else(|error| panic!("empty-values code object: {error:?}"));
    let mut callback_args = [Word::NIL];
    let mut values = MultipleValues::new();
    assert_eq!(
        call_builtin_through_native_trampoline(
            &mut runtime,
            callback,
            callback_code,
            &mut callback_args,
            &mut values,
        ),
        Ok(Word::fixnum(7))
    );
    assert!(
        !runtime.context.frame_snapshot_failed(),
        "direct Rust ENTRY call must not capture its unmapped caller PC"
    );
    assert_eq!(values.as_slice(), &[Word::fixnum(7)]);
    assert!(values.is_explicit());

    callback_args[0] = Word::TRUE;
    assert_eq!(
        call_builtin_through_native_trampoline(
            &mut runtime,
            callback,
            callback_code,
            &mut callback_args,
            &mut values,
        ),
        Ok(Word::NIL)
    );
    assert!(values.is_empty());
    assert!(values.is_explicit());

    let package = runtime
        .object
        .find_package(&runtime.context, "COMMON-LISP")
        .unwrap_or_else(|| panic!("COMMON-LISP package missing"));
    let (symbol, _) = Package::from_word(package)
        .intern(&mut runtime.context, &runtime.object, "VALUES")
        .unwrap_or_else(|error| panic!("intern VALUES: {error:?}"));
    let function_word = symbol_function(&runtime.context, symbol)
        .unwrap_or_else(|error| panic!("VALUES function cell: {error:?}"));
    let function = FunctionObject::try_from(function_word)
        .unwrap_or_else(|error| panic!("VALUES function object: {error:?}"));
    let code = function_code(&runtime.context, Function::from_word(function_word))
        .unwrap_or_else(|error| panic!("VALUES code object: {error:?}"));

    let mut twenty = (1..=20).map(Word::fixnum).collect::<Vec<_>>();
    values.clear();
    assert_eq!(
        call_builtin_through_native_trampoline(
            &mut runtime,
            function,
            code,
            &mut twenty,
            &mut values,
        ),
        Ok(Word::fixnum(1))
    );
    assert_eq!(values.as_slice(), twenty);
    assert!(values.is_explicit());

    let mut twenty_one = (1..=21).map(Word::fixnum).collect::<Vec<_>>();
    assert_eq!(
        call_builtin_through_native_trampoline(
            &mut runtime,
            function,
            code,
            &mut twenty_one,
            &mut values,
        ),
        Err(ncl_object::ObjectError::Layout)
    );
    assert!(values.is_empty());
    assert!(!values.is_explicit());
}

#[test]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
fn direct_rust_builtin_entry_enters_native_while_allocating() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let function = register_native_allocating_builtin(&mut runtime);
    let code = function_code(&runtime.context, Function::from_word(function.as_word()))
        .unwrap_or_else(|error| panic!("native allocating code object: {error:?}"));
    runtime.context.set_gc_stress(true);
    let mut args = [];
    let mut values = MultipleValues::new();

    assert_eq!(
        call_builtin_through_native_trampoline(
            &mut runtime,
            function,
            code,
            &mut args,
            &mut values,
        ),
        Ok(Word::TRUE)
    );
    assert_eq!(runtime.context.native_state(), ncl_sys::NativeState::Lisp);
    assert!(!runtime.context.frame_snapshot_failed());
}

#[test]
fn closure_entries_survive_gc_stress_and_strict_forwarding() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    runtime.context.set_gc_stress(true);
    runtime.context.set_strict_forwarding(true);
    let value = runtime
        .eval("(funcall (let ((y 5)) (lambda (x) (+ x y))) 10)")
        .unwrap_or_else(|error| panic!("closure evaluation: {error:?}"));
    assert_eq!(runtime.format_result(value), "15");
}

#[test]
fn generic_builtin_trampoline_survives_gc_stress_and_strict_forwarding_in_a_loop() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    // Compile the recursive helper before enabling stress mode: `DEFUN`'s
    // own macro expansion is unrelated to the generic builtin trampoline
    // this test exercises, and is not part of this task's scope.
    runtime
        .eval(
            "(defun count-down (n acc) \
               (if (< n 1) acc (count-down (- n 1) (length (list n n)))))",
        )
        .unwrap_or_else(|error| panic!("defining count-down: {error:?}"));
    runtime.context.set_gc_stress(true);
    runtime.context.set_strict_forwarding(true);
    // `LIST` and `LENGTH` have no ISA-specific fast path, so every
    // recursive call below dispatches through the generic native
    // trampoline (`crate::builtin_trampoline`) and allocates, forcing a
    // collection on nearly every call under `gc_stress`.
    let value = runtime
        .eval("(count-down 20 0)")
        .unwrap_or_else(|error| panic!("gc-stress builtin loop: {error:?}"));
    assert_eq!(runtime.format_result(value), "2");
}

#[test]
fn generic_builtin_trampoline_forwards_all_list_arguments() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    for (count, expected) in [
        (7, "(1 2 3 4 5 6 7)"),
        (8, "(1 2 3 4 5 6 7 8)"),
        (10, "(1 2 3 4 5 6 7 8 9 10)"),
    ] {
        let form = format!(
            "(list {})",
            (1..=count)
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        );
        let value = runtime
            .eval(&form)
            .unwrap_or_else(|error| panic!("evaluating {form}: {error:?}"));
        assert_eq!(runtime.format_result(value), expected);
    }
}

#[test]
fn extended_builtin_arguments_survive_gc_stress_and_strict_forwarding() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    runtime
        .eval(
            "(defun list-cons-pairs () \
               (list (cons 1 2) (cons 3 4) (cons 5 6) (cons 7 8) \
                     (cons 9 10) (cons 11 12) (cons 13 14)))",
        )
        .unwrap_or_else(|error| panic!("defining list-cons-pairs: {error:?}"));
    runtime.context.set_gc_stress(true);
    runtime.context.set_strict_forwarding(true);

    let value = runtime
        .eval("(list-cons-pairs)")
        .unwrap_or_else(|error| panic!("evaluating list-cons-pairs: {error:?}"));
    assert_eq!(
        runtime.format_result(value),
        "((1 . 2) (3 . 4) (5 . 6) (7 . 8) (9 . 10) (11 . 12) (13 . 14))"
    );
}

#[test]
#[cfg(target_arch = "aarch64")]
fn make_rest_list_survives_gc_stress_and_strict_forwarding() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    runtime.context.set_gc_stress(true);
    runtime.context.set_strict_forwarding(true);
    let value = runtime
        .eval("(funcall (lambda (&rest values) values) 1 2 3 4 5 6)")
        .unwrap_or_else(|error| panic!("make-rest-list evaluation: {error:?}"));
    assert_eq!(runtime.format_result(value), "(1 2 3 4 5 6)");
}

#[test]
fn calls_registered_builtin_through_function_and_symbol_designators_under_gc_stress_and_forwarding()
{
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let function = register_builtin(&mut runtime);
    let package = runtime
        .object
        .find_package(&runtime.context, "NCL-TEST")
        .and_then(|word| {
            Package::from_word(word)
                .intern(&mut runtime.context, &runtime.object, "RUNTIME-CALL")
                .ok()
        })
        .map_or_else(
            || panic!("test symbol was not interned"),
            |(symbol, _)| ncl_object::typed::Symbol::from_word(symbol),
        );
    runtime.context.set_gc_stress(true);
    runtime.context.set_strict_forwarding(true);
    let argument_words = [Word::fixnum(2), Word::fixnum(3)];
    let args = FunctionArguments::new(&argument_words);
    let mut caller = RuntimeFunctionCaller;

    let mut values = MultipleValues::new();
    assert_eq!(
        caller.call_function(
            &mut runtime.context,
            &runtime.object,
            FunctionDesignator::Function(function),
            args,
            &mut values,
        ),
        Ok(Word::fixnum(5))
    );
    assert!(values.is_empty());

    let mut values = MultipleValues::new();
    assert_eq!(
        caller.call_function(
            &mut runtime.context,
            &runtime.object,
            FunctionDesignator::Symbol(package),
            args,
            &mut values,
        ),
        Ok(Word::fixnum(5))
    );
}

#[path = "tests_function_call_remaining.rs"]
mod remaining;
