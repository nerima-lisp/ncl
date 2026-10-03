use super::{HashTable, HashTest, INDEX, TEST, WEAKNESS, Weakness};
use crate::{ObjectError, Runtime, ThreadContext, make_simple_vector};
use ncl_sys::Word;

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    (runtime, ctx)
}

#[test]
fn invalid_metadata_and_index_entries_report_layout_errors() {
    let (runtime, mut ctx) = setup();
    let table = HashTable::new(&mut ctx, &runtime, HashTest::Eq, Weakness::None)
        .unwrap_or_else(|error| panic!("table: {error:?}"));
    crate::object_access::put(&mut ctx, table.as_word(), TEST, Word::TRUE)
        .unwrap_or_else(|error| panic!("put test: {error:?}"));
    assert_eq!(table.test(&ctx), Err(ObjectError::Layout));
    crate::object_access::put(&mut ctx, table.as_word(), TEST, Word::fixnum(0))
        .unwrap_or_else(|error| panic!("put test: {error:?}"));
    crate::object_access::put(&mut ctx, table.as_word(), WEAKNESS, Word::fixnum(5))
        .unwrap_or_else(|error| panic!("put weakness: {error:?}"));
    assert_eq!(table.weakness(&ctx), Err(ObjectError::Layout));
    let bad_index = make_simple_vector(&mut ctx, &runtime, &[Word::TRUE])
        .unwrap_or_else(|error| panic!("bad index: {error:?}"));
    crate::object_access::put(&mut ctx, table.as_word(), INDEX, bad_index)
        .unwrap_or_else(|error| panic!("put index: {error:?}"));
    assert_eq!(
        table.get(&mut ctx, Word::fixnum(1)),
        Err(ObjectError::Layout)
    );
}
