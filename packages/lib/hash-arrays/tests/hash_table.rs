#![allow(missing_docs)]

use ncl_lib_hash_arrays::{gethash, make_hash_table, register, remhash};
use ncl_object::hash_table::{HashTable, HashTest};
use ncl_object::{FunctionObject, MultipleValues, Runtime, ThreadContext, Word};

#[test]
fn registered_hash_builtins_use_the_runtime_aware_callback_path() {
    let runtime = Runtime::new().unwrap();
    register(&runtime).unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();

    let make = FunctionObject::from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "MAKE-HASH-TABLE")
            .unwrap(),
    );
    let table = runtime.call_builtin(&mut ctx, make, &[]).unwrap();
    let table_ref = HashTable::from(table);
    table_ref
        .insert(&mut ctx, &runtime, Word::fixnum(7), Word::fixnum(9))
        .unwrap();

    let get = FunctionObject::from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "GETHASH")
            .unwrap(),
    );
    let value = runtime
        .call_builtin(&mut ctx, get, &[Word::fixnum(7), table, Word::NIL])
        .unwrap();
    assert_eq!(value, Word::fixnum(9));
    assert_eq!(ctx.values(), &[Word::fixnum(9), Word::TRUE]);

    let mut values = MultipleValues::new();
    assert_eq!(
        gethash(&mut ctx, Word::fixnum(7), table, Word::NIL, &mut values).unwrap(),
        Word::fixnum(9)
    );
    assert_eq!(values.as_slice(), &[Word::fixnum(9), Word::TRUE]);
    assert_eq!(
        remhash(&mut ctx, &runtime, Word::fixnum(7), table).unwrap(),
        Word::TRUE
    );
    assert_eq!(table_ref.count(&ctx).unwrap(), 0);
}

#[test]
fn helper_allocates_requested_hash_test() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let table = make_hash_table(&mut ctx, &runtime, HashTest::Eq).unwrap();
    assert_eq!(HashTable::from(table).test(&ctx).unwrap(), HashTest::Eq);
}
