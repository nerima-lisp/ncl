#![allow(clippy::unwrap_used, reason = "tests assert on printer output")]

//! Coverage for opaque object kinds not constructed by the printer's other tests.

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{Runtime, ThreadContext, Word, make_closure, make_code_object, make_readtable};
use ncl_printer::{PrintOptions, StringSink, write};

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_printer::register(&mut ctx, &runtime).unwrap();
    (runtime, ctx)
}

fn render(runtime: &Runtime, ctx: &mut ThreadContext, object: Word) -> String {
    let mut sink = StringSink::new();
    write(ctx, runtime, object, &mut sink, &PrintOptions::new()).unwrap();
    sink.into_string()
}

#[test]
fn opaque_object_kinds_render_their_type_names() {
    let (runtime, mut ctx) = context();
    let table = HashTable::new(&mut ctx, &runtime, HashTest::Equal, Weakness::None)
        .unwrap()
        .as_word();
    let readtable = make_readtable(&mut ctx, &runtime, Word::NIL, Word::NIL, Word::NIL)
        .unwrap()
        .as_word();
    let code = make_code_object(&mut ctx, &runtime, 0, 0, Word::NIL, Word::NIL, Word::NIL).unwrap();
    let closure = make_closure(&mut ctx, &runtime, 0, Word::NIL, Word::NIL, code, &[])
        .unwrap()
        .as_word();
    for (object, prefix) in [
        (table, "#<HASH-TABLE "),
        (closure, "#<CLOSURE "),
        (readtable, "#<READTABLE "),
        (code.as_word(), "#<CODE "),
    ] {
        assert!(render(&runtime, &mut ctx, object).starts_with(prefix));
    }
}
