#![allow(
    clippy::unwrap_used,
    missing_docs,
    reason = "coverage tests assert on printer output"
)]

use ncl_object::{Runtime, ThreadContext, Word, make_string, make_symbol};
use ncl_printer::{PrintCase, PrintError, PrintOptions, StringSink, write};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    (runtime, ctx)
}

fn render(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    object: Word,
    options: PrintOptions,
) -> Result<String, PrintError> {
    let mut sink = StringSink::new();
    write(ctx, runtime, object, &mut sink, &options)?;
    Ok(sink.into_string())
}

fn uninterned(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
    let name = make_string(ctx, runtime, &name.chars().collect::<Vec<_>>()).unwrap();
    make_symbol(ctx, runtime, name).unwrap()
}

#[test]
fn symbol_output_escapes_reader_delimiters_and_case_changes() {
    let (runtime, mut ctx) = setup();
    let escaped = uninterned(&mut ctx, &runtime, r"a|b\c");
    assert_eq!(
        render(&runtime, &mut ctx, escaped, PrintOptions::new()),
        Ok(r"#:|a\|b\\c|".to_owned())
    );

    let dot = uninterned(&mut ctx, &runtime, ".");
    assert_eq!(
        render(&runtime, &mut ctx, dot, PrintOptions::new()),
        Ok("#:|.|".to_owned())
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            dot,
            PrintOptions::new().with_case(PrintCase::Downcase),
        ),
        Ok("#:|.|".to_owned())
    );
}

#[test]
fn character_and_readability_options_have_distinct_output_contracts() {
    let (runtime, mut ctx) = setup();
    let boundary = char::from_u32(0x10_FFFF).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            Word::character(0x10_FFFF),
            PrintOptions::new(),
        ),
        Ok(format!("#\\{boundary}"))
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            Word::character(0x10_FFFF),
            PrintOptions::new().with_escape(false),
        ),
        Ok(boundary.to_string())
    );

    let text = make_string(&mut ctx, &runtime, &['a', '"', '\\']).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            text,
            PrintOptions::new().with_escape(false),
        ),
        Ok("a\"\\".to_owned())
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            text,
            PrintOptions::new().with_escape(false).with_readably(true),
        ),
        Ok("\"a\\\"\\\\\"".to_owned())
    );

    let mut sink = StringSink::new();
    let vector = ncl_object::make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)]).unwrap();
    assert_eq!(
        write(
            &mut ctx,
            &runtime,
            vector,
            &mut sink,
            &PrintOptions::new().with_readably(true).with_array(false),
        )
        .unwrap_err(),
        PrintError::NotReadable
    );
}
