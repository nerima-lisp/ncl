#![allow(missing_docs, clippy::expect_used, clippy::unwrap_used)]

use ncl_lib_format::{execute, parse};
use ncl_object::{Runtime, ThreadContext, Word, make_cons, make_string};
use ncl_printer::StringSink;

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("register context");
    (runtime, ctx)
}

fn string(runtime: &Runtime, ctx: &mut ThreadContext, value: &str) -> Word {
    make_string(ctx, runtime, &value.chars().collect::<Vec<_>>()).expect("string")
}

fn list(runtime: &Runtime, ctx: &mut ThreadContext, values: &[Word]) -> Word {
    values.iter().rev().copied().fold(Word::NIL, |tail, value| {
        make_cons(ctx, runtime, value, tail).expect("cons")
    })
}

#[test] // CLHS 22.3.9.2
fn colon_escape_stops_the_current_justification_segment() {
    let (runtime, mut ctx) = context();
    let a = string(&runtime, &mut ctx, "a");
    let b = string(&runtime, &mut ctx, "b");
    let left = list(&runtime, &mut ctx, &[a]);
    let right = list(&runtime, &mut ctx, &[b]);
    let rows = list(&runtime, &mut ctx, &[left, right]);
    let control = parse("~:{~<~A~:^x~;~A~>!~}Q").expect("control");
    let mut sink = StringSink::new();

    assert_eq!(
        execute(&control, &[rows], &mut ctx, &runtime, &mut sink),
        Ok(1)
    );
    assert_eq!(sink.into_string(), "a!b!Q");
}
