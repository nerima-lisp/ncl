#![allow(missing_docs)]

use ncl_object::{BuiltinImplementation, MultipleValues, Runtime, ThreadContext};
use ncl_sys::Word;

fn add_builtin(
    _runtime: &Runtime,
    _ctx: &mut ThreadContext,
    args: &[Word],
    values: &mut MultipleValues,
) -> Result<Word, ncl_object::ObjectError> {
    if args.len() != 2 {
        return Err(ncl_object::ObjectError::TypeError);
    }
    values.set(&[Word::fixnum(99), Word::fixnum(100)]);
    Ok(Word::fixnum(
        args[0].as_fixnum().unwrap_or(0) + args[1].as_fixnum().unwrap_or(0),
    ))
}

fn keyword_adapter(args: &[Word]) -> Result<Vec<Word>, ncl_object::ObjectError> {
    if args.len() == 2 {
        Ok(vec![args[1], args[0]])
    } else {
        Err(ncl_object::ObjectError::TypeError)
    }
}

#[test]
fn registered_builtin_has_a_function_object_and_rust_call_boundary() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let descriptor = ncl_object::Builtin {
        arity: 2,
        direct: true,
        lambda_list: "left right",
    };
    let implementation = BuiltinImplementation::direct(descriptor, add_builtin);
    let entry = implementation.entry;
    let function = runtime
        .register_builtin(&mut ctx, "NCL-TEST", "ADD", implementation)
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
    let descriptor = ncl_object::Builtin {
        arity: 0,
        direct: false,
        lambda_list: "&key left right",
    };
    let function = runtime
        .register_builtin(
            &mut ctx,
            "NCL-TEST",
            "ADAPTED",
            BuiltinImplementation::adapted(descriptor, add_builtin, keyword_adapter),
        )
        .unwrap_or_else(|error| panic!("builtin: {error:?}"));
    assert_eq!(
        runtime.call_builtin(&mut ctx, function, &[Word::fixnum(3), Word::fixnum(2)]),
        Ok(Word::fixnum(5))
    );
}
