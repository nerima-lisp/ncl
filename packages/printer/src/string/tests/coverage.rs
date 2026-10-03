#![allow(
    clippy::unwrap_used,
    reason = "coverage tests assert on setup and output"
)]

use super::super::{PrintOptions, StringSink, write};
use crate::print::Printer;
use ncl_object::{Runtime, ThreadContext, Word, make_string};

#[test]
fn escaped_strings_cover_backslash_and_raw_control_paths() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let text = make_string(&mut ctx, &runtime, &['\\', '"', 'x']).unwrap();
    let mut sink = StringSink::new();
    write(&mut ctx, &runtime, text, &mut sink, &PrintOptions::new()).unwrap();
    assert_eq!(sink.into_string(), "\"\\\\\\\"x\"");
    let mut sink = StringSink::new();
    write(
        &mut ctx,
        &runtime,
        text,
        &mut sink,
        &PrintOptions::new().with_escape(false),
    )
    .unwrap();
    assert_eq!(sink.into_string(), "\\\"x");
}

#[test]
fn named_and_invalid_characters_have_stable_output() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    for (code, expected) in [
        (32, "#\\Space"),
        (10, "#\\Newline"),
        (9, "#\\Tab"),
        (0, "#\\Null"),
    ] {
        let mut sink = StringSink::new();
        write(
            &mut ctx,
            &runtime,
            Word::character(code),
            &mut sink,
            &PrintOptions::new(),
        )
        .unwrap();
        assert_eq!(sink.into_string(), expected);
    }
    let mut sink = StringSink::new();
    let mut printer = Printer::new(&mut ctx, &runtime, &mut sink, PrintOptions::new());
    printer.print_character(0x11_0000).unwrap();
    assert_eq!(sink.into_string(), "#\\?");
}
