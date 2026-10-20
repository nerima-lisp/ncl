#![allow(
    clippy::unwrap_used,
    reason = "coverage tests assert on printer output"
)]

//! Eleventh-wave coverage for the remaining ordinary printer branches.

use ncl_object::{
    FunctionObject, ObjectError, Package, Runtime, ThreadContext, Word, make_cons,
    make_simple_vector, make_string, make_symbol, set_symbol_special, set_symbol_value,
};
use ncl_printer::{
    PrintCase, PrintError, PrintOptions, StringSink, copy_pprint_dispatch, pprint_dispatch,
    set_pprint_dispatch, write, write_to_string,
};

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_printer::register(&mut ctx, &runtime).unwrap();
    ncl_lib_streams::register(&runtime).unwrap();
    (runtime, ctx)
}

fn intern(runtime: &Runtime, ctx: &mut ThreadContext, package: &str, name: &str) -> Word {
    let package = runtime.ensure_package(ctx, package).unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
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

fn string_value(ctx: &ThreadContext, value: Word) -> String {
    let length = ncl_object::string_length(ctx, value).unwrap();
    (0..length)
        .map(|index| ncl_object::string_ref(ctx, value, index).unwrap())
        .collect()
}

#[test]
fn ambient_options_cover_boolean_modes_and_valid_limits() {
    let (runtime, mut ctx) = context();
    let names = [
        "*PRINT-ESCAPE*",
        "*PRINT-READABLY*",
        "*PRINT-RADIX*",
        "*PRINT-CIRCLE*",
        "*PRINT-PRETTY*",
        "*PRINT-ARRAY*",
        "*PRINT-GENSYM*",
        "*PRINT-BASE*",
        "*PRINT-LENGTH*",
        "*PRINT-LEVEL*",
    ];
    let symbols: Vec<_> = names
        .iter()
        .map(|name| intern(&runtime, &mut ctx, "COMMON-LISP", name))
        .collect();
    for symbol in symbols.iter().copied() {
        set_symbol_special(&mut ctx, symbol, true).unwrap();
    }
    for (index, value) in [
        Word::TRUE,
        Word::NIL,
        Word::TRUE,
        Word::TRUE,
        Word::TRUE,
        Word::NIL,
        Word::NIL,
        Word::fixnum(8),
        Word::fixnum(2),
        Word::fixnum(1),
    ]
    .into_iter()
    .enumerate()
    {
        set_symbol_value(&mut ctx, symbols[index], value).unwrap();
    }
    let options = PrintOptions::from_specials(&mut ctx, &runtime);
    assert!(options.escape());
    assert!(!options.readably());
    assert!(options.radix());
    assert!(options.circle());
    assert!(options.pretty());
    assert!(!options.array());
    assert!(!options.gensym());
    assert_eq!(options.base().get(), 8);
    assert_eq!(options.length().map(ncl_printer::NonNegative::get), Some(2));
    assert_eq!(options.level().map(ncl_printer::NonNegative::get), Some(1));
}

#[test]
fn strings_and_characters_cover_raw_escape_and_special_character_names() {
    let (runtime, mut ctx) = context();
    let text = make_string(&mut ctx, &runtime, &['"', '\\', 'x']).unwrap();
    assert_eq!(
        render(&runtime, &mut ctx, text, PrintOptions::new()),
        Ok("\"\\\"\\\\x\"".to_string())
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            text,
            PrintOptions::new().with_escape(false),
        ),
        Ok("\"\\x".to_string())
    );
    for (character, expected) in [
        ('\n', "#\\Newline"),
        ('\t', "#\\Tab"),
        (' ', "#\\Space"),
        ('A', "#\\A"),
    ] {
        assert_eq!(
            render(
                &runtime,
                &mut ctx,
                Word::character(u32::from(character)),
                PrintOptions::new(),
            ),
            Ok(expected.to_string())
        );
    }
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            Word::character(u32::from('\n')),
            PrintOptions::new().with_escape(false),
        ),
        Ok("\n".to_string())
    );
}

#[test]
fn symbols_cover_keyword_common_lisp_and_uninterned_prefixes() {
    let (runtime, mut ctx) = context();
    let keyword = intern(&runtime, &mut ctx, "KEYWORD", "READY");
    let common = intern(&runtime, &mut ctx, "COMMON-LISP", "READY");
    let name = make_string(&mut ctx, &runtime, &['N', 'E', 'W']).unwrap();
    let uninterned = make_symbol(&mut ctx, &runtime, name).unwrap();
    assert_eq!(
        render(&runtime, &mut ctx, keyword, PrintOptions::new()),
        Ok(":READY".to_string())
    );
    assert_eq!(
        render(&runtime, &mut ctx, common, PrintOptions::new()),
        Ok("READY".to_string())
    );
    assert_eq!(
        render(&runtime, &mut ctx, uninterned, PrintOptions::new()),
        Ok("#:NEW".to_string())
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            uninterned,
            PrintOptions::new().with_gensym(false),
        ),
        Ok("NEW".to_string())
    );
}

#[test]
fn cons_abbreviations_require_exact_one_argument_and_preserve_dotted_tails() {
    let (runtime, mut ctx) = context();
    let quote = intern(&runtime, &mut ctx, "COMMON-LISP", "QUOTE");
    let function = intern(&runtime, &mut ctx, "COMMON-LISP", "FUNCTION");
    let quote_tail = make_cons(&mut ctx, &runtime, Word::fixnum(4), Word::NIL).unwrap();
    let quote_form = make_cons(&mut ctx, &runtime, quote, quote_tail).unwrap();
    let too_many_tail = make_cons(&mut ctx, &runtime, Word::fixnum(5), Word::NIL).unwrap();
    let too_many_args = make_cons(&mut ctx, &runtime, Word::fixnum(4), too_many_tail).unwrap();
    let too_many = make_cons(&mut ctx, &runtime, quote, too_many_args).unwrap();
    let function_tail = make_cons(&mut ctx, &runtime, Word::fixnum(6), Word::NIL).unwrap();
    let function_form = make_cons(&mut ctx, &runtime, function, function_tail).unwrap();
    let dotted = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2)).unwrap();
    assert_eq!(
        render(&runtime, &mut ctx, quote_form, PrintOptions::new()),
        Ok("'4".to_string())
    );
    assert_eq!(
        render(&runtime, &mut ctx, function_form, PrintOptions::new()),
        Ok("#'6".to_string())
    );
    assert_eq!(
        render(&runtime, &mut ctx, too_many, PrintOptions::new()),
        Ok("(QUOTE 4 5)".to_string())
    );
    assert_eq!(
        render(&runtime, &mut ctx, dotted, PrintOptions::new()),
        Ok("(1 . 2)".to_string())
    );
}

#[test]
fn dispatch_default_and_copy_keep_function_values_distinct() {
    let (runtime, mut ctx) = context();
    let table = Word::NIL;
    assert_eq!(
        pprint_dispatch(&mut ctx, Word::fixnum(9), table).unwrap(),
        Word::NIL
    );
    let marker = Word::fixnum(77);
    let changed = set_pprint_dispatch(&mut ctx, &runtime, Word::fixnum(9), marker, table).unwrap();
    assert_eq!(
        pprint_dispatch(&mut ctx, Word::fixnum(9), changed).unwrap(),
        marker
    );
    let copied = copy_pprint_dispatch(&mut ctx, &runtime, changed).unwrap();
    assert_eq!(
        pprint_dispatch(&mut ctx, Word::fixnum(9), copied).unwrap(),
        marker
    );
    assert_eq!(
        pprint_dispatch(&mut ctx, Word::fixnum(8), copied).unwrap(),
        Word::NIL
    );
}

#[test]
fn vector_length_and_readable_opaque_paths_are_asserted_together() {
    let (runtime, mut ctx) = context();
    let vector = make_simple_vector(
        &mut ctx,
        &runtime,
        &[Word::fixnum(1), Word::fixnum(2), Word::fixnum(3)],
    )
    .unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            vector,
            PrintOptions::new().with_vector_length(Some(1)),
        ),
        Ok("#(1 ...)".to_string())
    );
    assert!(
        render(
            &runtime,
            &mut ctx,
            vector,
            PrintOptions::new().with_array(false),
        )
        .unwrap()
        .starts_with("#<VECTOR ")
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            vector,
            PrintOptions::new().with_readably(true).with_array(false),
        ),
        Err(PrintError::NotReadable)
    );
}

#[test]
fn write_to_string_allocates_the_same_readable_text_as_a_sink() {
    let (runtime, mut ctx) = context();
    let list = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL).unwrap();
    let object_string = write_to_string(&mut ctx, &runtime, list, &PrintOptions::new()).unwrap();
    assert_eq!(string_value(&ctx, object_string), "(1)");
    let mut sink = StringSink::new();
    write(
        &mut ctx,
        &runtime,
        list,
        &mut sink,
        &PrintOptions::new().with_case(PrintCase::Upcase),
    )
    .unwrap();
    assert_eq!(sink.into_string(), "(1)");
}

#[test]
fn print_builtin_missing_object_reports_type_error_before_output_lookup() {
    let (runtime, mut ctx) = context();
    let print =
        FunctionObject::try_from(runtime.function(&mut ctx, "COMMON-LISP", "PRINT").unwrap())
            .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, print, &[]),
        Err(ObjectError::TypeError)
    );
}
