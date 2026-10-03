#![allow(
    clippy::unwrap_used,
    reason = "coverage tests assert on printer output"
)]
#![allow(missing_docs)]

use ncl_object::{Runtime, ThreadContext, make_string};
use ncl_printer::{PrintOptions, StringSink, write};

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_printer::register(&mut ctx, &runtime).unwrap();
    (runtime, ctx)
}

#[test]
fn non_escaping_multiline_string_updates_the_print_column_per_line() {
    let (runtime, mut ctx) = context();
    let string = make_string(&mut ctx, &runtime, &['A', '\n', 'B']).unwrap();
    let mut sink = StringSink::new();

    write(
        &mut ctx,
        &runtime,
        string,
        &mut sink,
        &PrintOptions::new().with_escape(false),
    )
    .unwrap();

    assert_eq!(sink.into_string(), "A\nB");
}
