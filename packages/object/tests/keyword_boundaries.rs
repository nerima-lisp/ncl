#![allow(missing_docs)]

use ncl_object::package::Package;
use ncl_object::{car, cdr, make_cons, ObjectError, Runtime, ThreadContext};
use ncl_sys::Word;

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    (runtime, ctx)
}

fn function(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> ncl_object::FunctionObject {
    runtime
        .function(ctx, "NCL-EXT", name)
        .and_then(|word| ncl_object::FunctionObject::try_from(word).ok())
        .unwrap_or_else(|| panic!("{name} not registered"))
}

fn list(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Word {
    values.iter().rev().fold(Word::NIL, |tail, value| {
        make_cons(ctx, runtime, *value, tail)
            .unwrap_or_else(|error| panic!("list allocation: {error:?}"))
    })
}

fn keyword(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
    let package = runtime
        .find_package(ctx, "KEYWORD")
        .unwrap_or_else(|| panic!("KEYWORD package missing"));
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap_or_else(|error| panic!("intern {name}: {error:?}"))
        .0
}

fn keyword_arguments(ctx: &mut ThreadContext, runtime: &Runtime, entries: &[(Word, Word)]) -> Word {
    let mut values = Vec::with_capacity(entries.len() * 2);
    for (key, value) in entries {
        values.extend([*key, *value]);
    }
    list(ctx, runtime, &values)
}

#[test]
fn make_rest_list_checks_ranges_types_and_arity() {
    let (runtime, mut ctx) = context();
    let make_rest = function(&mut ctx, &runtime, "MAKE-REST-LIST");
    let result = runtime
        .call_builtin(
            &mut ctx,
            make_rest,
            &[
                Word::fixnum(3),
                Word::fixnum(1),
                Word::fixnum(10),
                Word::fixnum(20),
                Word::fixnum(30),
            ],
        )
        .unwrap_or_else(|error| panic!("make rest list: {error:?}"));
    assert_eq!(car(&ctx, result), Ok(Word::fixnum(20)));
    let tail = cdr(&ctx, result).unwrap_or_else(|error| panic!("rest tail: {error:?}"));
    assert_eq!(car(&ctx, tail), Ok(Word::fixnum(30)));
    assert_eq!(cdr(&ctx, tail), Ok(Word::NIL));

    for args in [
        vec![
            Word::fixnum(3),
            Word::fixnum(4),
            Word::fixnum(1),
            Word::fixnum(2),
            Word::fixnum(3),
        ],
        vec![
            Word::fixnum(4),
            Word::fixnum(0),
            Word::fixnum(1),
            Word::fixnum(2),
        ],
        vec![Word::fixnum(-1), Word::fixnum(0)],
    ] {
        assert_eq!(
            runtime.call_builtin(&mut ctx, make_rest, &args),
            Err(ObjectError::TypeError)
        );
    }
    assert_eq!(
        runtime.call_builtin(&mut ctx, make_rest, &[Word::fixnum(1)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(ctx.take_pending_lisp_error(), None);
}

#[test]
fn keyword_value_and_supplied_p_define_duplicate_and_malformed_boundaries() {
    let (runtime, mut ctx) = context();
    let value = function(&mut ctx, &runtime, "KEYWORD-VALUE");
    let supplied = function(&mut ctx, &runtime, "KEYWORD-SUPPLIED-P");
    let key = keyword(&mut ctx, &runtime, "DUPLICATE");
    let arguments = keyword_arguments(
        &mut ctx,
        &runtime,
        &[(key, Word::fixnum(1)), (key, Word::fixnum(2))],
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, value, &[arguments, key]),
        Ok(Word::fixnum(1))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, supplied, &[arguments, key]),
        Ok(Word::TRUE)
    );

    let odd = make_cons(&mut ctx, &runtime, key, Word::NIL)
        .unwrap_or_else(|error| panic!("odd list: {error:?}"));
    assert_eq!(
        runtime.call_builtin(&mut ctx, value, &[odd, key]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, supplied, &[Word::fixnum(7), key]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn keyword_checker_reports_unknown_keys_and_honors_both_allow_other_keys_forms() {
    let (runtime, mut ctx) = context();
    let checker = function(&mut ctx, &runtime, "CHECK-KEYWORDS");
    let known = keyword(&mut ctx, &runtime, "KNOWN");
    let unknown = keyword(&mut ctx, &runtime, "UNKNOWN");
    let allow = keyword(&mut ctx, &runtime, "ALLOW-OTHER-KEYS");
    let unknown_arguments = keyword_arguments(&mut ctx, &runtime, &[(unknown, Word::fixnum(1))]);

    assert_eq!(
        runtime.call_builtin(&mut ctx, checker, &[unknown_arguments, Word::NIL, known]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, checker, &[unknown_arguments, Word::TRUE, known]),
        Ok(Word::NIL)
    );
    let allowed_list = list(&mut ctx, &runtime, &[known, unknown]);
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            checker,
            &[unknown_arguments, Word::NIL, allowed_list],
        ),
        Ok(Word::NIL)
    );

    let allowed_by_call = keyword_arguments(
        &mut ctx,
        &runtime,
        &[(allow, Word::TRUE), (unknown, Word::fixnum(2))],
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, checker, &[allowed_by_call, Word::NIL, known]),
        Ok(Word::NIL)
    );
    let disabled = keyword_arguments(
        &mut ctx,
        &runtime,
        &[(allow, Word::NIL), (unknown, Word::fixnum(2))],
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, checker, &[disabled, Word::NIL, known]),
        Err(ObjectError::TypeError)
    );

    let other_package = Package::new(&mut ctx, &runtime, "NOT-KEYWORD")
        .unwrap_or_else(|error| panic!("package: {error:?}"));
    let (lookalike, _) = other_package
        .intern(&mut ctx, &runtime, "ALLOW-OTHER-KEYS")
        .unwrap_or_else(|error| panic!("intern lookalike: {error:?}"));
    let lookalike_args = keyword_arguments(
        &mut ctx,
        &runtime,
        &[(lookalike, Word::TRUE), (unknown, Word::fixnum(3))],
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, checker, &[lookalike_args, Word::NIL, known]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn keyword_builtins_reject_wrong_arity_and_non_symbol_keyword_entries() {
    let (runtime, mut ctx) = context();
    let checker = function(&mut ctx, &runtime, "CHECK-KEYWORDS");
    let value = function(&mut ctx, &runtime, "KEYWORD-VALUE");
    let supplied = function(&mut ctx, &runtime, "KEYWORD-SUPPLIED-P");
    let key = keyword(&mut ctx, &runtime, "KEY");
    for (builtin, args) in [
        (checker, vec![Word::NIL]),
        (value, vec![Word::NIL]),
        (supplied, vec![Word::NIL]),
    ] {
        assert_eq!(
            runtime.call_builtin(&mut ctx, builtin, &args),
            Err(ObjectError::TypeError)
        );
        assert_eq!(ctx.take_pending_lisp_error(), None);
    }

    let malformed = list(&mut ctx, &runtime, &[Word::fixnum(9), Word::fixnum(10)]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, value, &[malformed, key]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(ctx.take_pending_lisp_error(), None);
    assert_eq!(
        runtime.call_builtin(&mut ctx, checker, &[Word::fixnum(9), Word::NIL, key]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(ctx.take_pending_lisp_error(), None);
}
