#![allow(missing_docs)]

use ncl_lib_hash_arrays::hash::{
    HashTableOptions, LispValue, clrhash, gethash, hash_table_count, make_hash_table, remhash,
    set_hash_value,
};
use ncl_object::{Runtime, ThreadContext, Word};

fn runtime_and_context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("create runtime");
    let mut context = ThreadContext::new();
    context.register(&runtime).expect("register runtime");
    (runtime, context)
}

#[test]
fn typed_hash_domain_round_trip_and_clear() {
    let (runtime, mut context) = runtime_and_context();
    let table = make_hash_table(&mut context, &runtime, HashTableOptions::default())
        .expect("make hash table");
    let key = LispValue::from_word(Word::fixnum(7));
    let value = LispValue::from_word(Word::fixnum(42));
    let default = LispValue::from_word(Word::NIL);

    set_hash_value(&mut context, &runtime, table, key, value).expect("insert");
    let found = gethash(&mut context, table, key, default).expect("gethash");
    assert_eq!(found.value, value);
    assert!(found.present);
    assert_eq!(hash_table_count(&context, table).expect("count"), 1);
    assert!(remhash(&mut context, &runtime, table, key).expect("remhash"));
    assert_eq!(clrhash(&mut context, &runtime, table).expect("clrhash"), 0);
}
