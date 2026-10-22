#![allow(
    clippy::unwrap_used,
    reason = "test constructs a valid builtin fixture"
)]

use super::fill_objects;
use crate::image_builtin_support::{BUILTIN, BUILTIN_ID, BUILTIN_NAME, image_builtin};
use crate::record::{BuiltinIdentity, Record, Ref};
use ncl_object::{
    BuiltinImplementation, Function, Runtime, ThreadContext, Word, function_entry, function_name,
    function_offset,
};

#[test]
fn builtin_fill_skips_registered_object_even_without_descriptor_lookup() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let original_entry = 0x1234_5678;
    let function = runtime
        .register_builtin(
            &mut ctx,
            BUILTIN_ID,
            BuiltinImplementation::direct(BUILTIN, image_builtin).with_entry(original_entry),
        )
        .unwrap();
    let function_word = function.as_word();
    let name = function_name(&ctx, Function::from_word(function_word)).unwrap();
    assert_eq!(
        runtime.registered_builtin_for_image(&ctx, name, "NCL-TEST", BUILTIN_NAME),
        Some(function)
    );

    assert!(ctx.thread_access().write_object_word(
        function_word,
        function_offset::BUILTIN_INDEX,
        Word::fixnum(-1),
    ));
    assert_eq!(runtime.builtin_descriptor(&ctx, function), None);

    let records = [Record::Function {
        closure: false,
        entry: 0,
        name: Ref::Immediate(Word::NIL.bits()),
        lambda_list: Ref::Immediate(Word::NIL.bits()),
        code: Ref::Immediate(Word::NIL.bits()),
        captures: Vec::new(),
        builtin: Some(BuiltinIdentity {
            package: "NCL-TEST".to_owned(),
            name: BUILTIN_NAME.to_owned(),
        }),
    }];
    fill_objects(&runtime, &mut ctx, &records, &[function_word]).unwrap();

    assert_eq!(
        function_entry(&ctx, Function::from_word(function_word)).unwrap(),
        original_entry
    );
}
