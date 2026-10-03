#![allow(clippy::unnecessary_wraps)]
#![allow(
    clippy::unwrap_used,
    reason = "tests assert on condition-system behavior"
)]

//! Wave 14 tests for alternate dispatch, malformed registration inputs, and
//! conversion variants not exercised by the earlier waves.

use ncl_conditions::{
    condition_class, condition_class_of, condition_from_lisp_error, make_condition, pop_handler,
    push_handler, signal_matched,
};
use ncl_object::{
    ArithmeticError, CellError, ControlError, FileError, FunctionObject, LispError, ObjectError,
    Package, PackageError, ProgramError, Runtime, StreamError, ThreadContext, Word, make_cons,
    make_string, string_length, string_ref,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_conditions::register(&runtime).unwrap();
    (runtime, ctx)
}

fn builtin(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    package: &str,
    name: &str,
) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, package, name).unwrap()).unwrap()
}

fn symbol(ctx: &mut ThreadContext, runtime: &Runtime, package: &str, name: &str) -> Word {
    let package = runtime.ensure_package(ctx, package).unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn text(ctx: &ThreadContext, value: Word) -> String {
    (0..string_length(ctx, value).unwrap())
        .map(|index| string_ref(ctx, value, index).unwrap())
        .collect()
}

#[allow(clippy::unnecessary_wraps)]
fn return_first(
    _runtime: std::ptr::NonNull<()>,
    _ctx: &mut ThreadContext,
    _function: Word,
    arguments: &[Word],
) -> Result<Word, ObjectError> {
    Ok(arguments.first().copied().unwrap_or(Word::NIL))
}

fn class_name(ctx: &ThreadContext, condition: Word) -> String {
    let class = condition_class_of(ctx, condition).unwrap();
    text(
        ctx,
        ncl_conditions::condition_class_name(ctx, class).unwrap(),
    )
}

#[test]
fn registration_wrappers_reject_wrong_designator_kinds_and_preserve_errors() {
    let (runtime, mut ctx) = setup();
    let push_handler = builtin(&runtime, &mut ctx, "NCL-EXT", "PUSH-HANDLER");
    let push_restart = builtin(&runtime, &mut ctx, "NCL-EXT", "PUSH-RESTART");
    let warning_text =
        make_string(&mut ctx, &runtime, &"WARNING".chars().collect::<Vec<_>>()).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, push_handler, &[warning_text, Word::NIL]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, push_restart, &[Word::NIL, Word::NIL, Word::NIL]),
        Err(ObjectError::TypeError)
    );

    let make = builtin(&runtime, &mut ctx, "COMMON-LISP", "MAKE-CONDITION");
    let unknown = symbol(&mut ctx, &runtime, "COMMON-LISP", "W14-UNKNOWN");
    assert_eq!(
        runtime.call_builtin(&mut ctx, make, &[unknown, Word::fixnum(1)]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn nested_handler_search_skips_unrelated_top_record_before_matching() {
    let (runtime, mut ctx) = setup();
    let error = condition_class(&mut ctx, &runtime, "ERROR").unwrap();
    let warning = condition_class(&mut ctx, &runtime, "WARNING").unwrap();
    let condition = make_condition(&mut ctx, &runtime, error, &[]).unwrap();
    let callback = builtin(&runtime, &mut ctx, "COMMON-LISP", "CONTINUE");
    let lower = push_handler(&mut ctx, &runtime, error, callback.as_word()).unwrap();
    let upper = push_handler(&mut ctx, &runtime, warning, callback.as_word()).unwrap();
    ctx.set_evaluator_runtime(std::ptr::NonNull::<()>::dangling().as_ptr());
    assert_eq!(signal_matched(&mut ctx, condition), Ok(true));
    pop_handler(&mut ctx, &runtime, upper);
    pop_handler(&mut ctx, &runtime, lower);
    assert_eq!(signal_matched(&mut ctx, condition), Ok(false));
}

#[test]
fn restart_builtins_accept_symbol_and_record_designators_with_arguments() {
    let (runtime, mut ctx) = setup();
    let push = builtin(&runtime, &mut ctx, "NCL-EXT", "PUSH-RESTART");
    let pop = builtin(&runtime, &mut ctx, "NCL-EXT", "POP-RESTART");
    let invoke = builtin(&runtime, &mut ctx, "COMMON-LISP", "INVOKE-RESTART");
    let find = builtin(&runtime, &mut ctx, "COMMON-LISP", "FIND-RESTART");
    let callback = builtin(&runtime, &mut ctx, "COMMON-LISP", "CONTINUE");
    ctx.set_condition_handler_invoker(return_first);
    ctx.set_evaluator_runtime(std::ptr::NonNull::<()>::dangling().as_ptr());
    let name = make_string(&mut ctx, &runtime, &"W14-CALL".chars().collect::<Vec<_>>()).unwrap();
    let token = runtime
        .call_builtin(
            &mut ctx,
            push,
            &[name, callback.as_word(), Word::NIL, Word::NIL, Word::NIL],
        )
        .unwrap();
    let designator = symbol(&mut ctx, &runtime, "COMMON-LISP", "W14-CALL");
    assert_eq!(
        runtime.call_builtin(&mut ctx, invoke, &[designator, Word::fixnum(41)]),
        Ok(Word::fixnum(41))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, invoke, &[token, Word::fixnum(42)]),
        Ok(Word::fixnum(42))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, find, &[designator, Word::fixnum(99)]),
        Ok(token)
    );
    assert_eq!(runtime.call_builtin(&mut ctx, pop, &[token]), Ok(Word::NIL));
    assert_eq!(
        runtime.call_builtin(&mut ctx, find, &[designator]),
        Ok(Word::NIL)
    );
}

#[test]
fn custom_slot_plist_uses_last_supplied_pair_and_odd_tail_nil() {
    let (runtime, mut ctx) = setup();
    let define = builtin(&runtime, &mut ctx, "NCL-EXT", "DEFINE-CONDITION-CLASS");
    let make = builtin(&runtime, &mut ctx, "COMMON-LISP", "MAKE-CONDITION");
    let access = builtin(&runtime, &mut ctx, "NCL-EXT", "CONDITION-SLOT-REF");
    let name = symbol(&mut ctx, &runtime, "NCL-W14", "PLIST-MATRIX");
    let key = symbol(&mut ctx, &runtime, "KEYWORD", "VALUE");
    let pair = make_cons(&mut ctx, &runtime, key, Word::NIL).unwrap();
    let specs = make_cons(&mut ctx, &runtime, pair, Word::NIL).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, define, &[name, Word::NIL, specs, Word::NIL]),
        Ok(name)
    );
    let instance = runtime
        .call_builtin(
            &mut ctx,
            make,
            &[name, key, Word::fixnum(1), key, Word::fixnum(2)],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, access, &[instance, Word::fixnum(0)]),
        Ok(Word::fixnum(1))
    );
    let odd = runtime.call_builtin(&mut ctx, make, &[name, key]).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, access, &[odd, Word::fixnum(0)]),
        Ok(Word::NIL)
    );
}

#[test]
fn conversion_variant_matrix_maps_specific_condition_classes() {
    let (runtime, mut ctx) = setup();
    let cases = [
        (
            LispError::ProgramError(ProgramError::UnknownKeyword),
            "PROGRAM-ERROR",
        ),
        (
            LispError::ProgramError(ProgramError::OddKeywordArguments),
            "PROGRAM-ERROR",
        ),
        (
            LispError::ArithmeticError(ArithmeticError::InvalidOperation),
            "ARITHMETIC-ERROR",
        ),
        (
            LispError::ControlError(ControlError::ReturnFrom),
            "CONTROL-ERROR",
        ),
        (LispError::CellError(CellError::UnboundSlot), "UNBOUND-SLOT"),
        (
            LispError::PackageError(PackageError::Locked),
            "PACKAGE-ERROR",
        ),
        (LispError::StreamError(StreamError::Closed), "STREAM-ERROR"),
        (LispError::FileError(FileError::NotFound), "FILE-ERROR"),
    ];
    for (error, expected) in cases {
        let condition = condition_from_lisp_error(&mut ctx, &runtime, error).unwrap();
        assert_eq!(class_name(&ctx, condition), expected);
    }
    let generic = condition_from_lisp_error(
        &mut ctx,
        &runtime,
        LispError::Object(ObjectError::ControlError),
    )
    .unwrap();
    assert_eq!(class_name(&ctx, generic), "ERROR");
}
