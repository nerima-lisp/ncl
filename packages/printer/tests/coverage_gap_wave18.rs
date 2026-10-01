#![allow(
    clippy::unwrap_used,
    reason = "coverage tests assert on printer output"
)]

//! Eighteenth-wave coverage for symbol/string branches, dispatch fallbacks,
//! stream errors, and numeric/array boundaries.

use ncl_object::{
    ArrayElementType, ArrayOptions, ObjectError, Package, Runtime, ThreadContext, Word,
    make_array, make_cons, make_simple_vector, make_specialized_array, make_string, make_symbol,
    set_symbol_value,
};
use ncl_printer::{
    PrintCase, PrintError, PrintOptions, StringSink, copy_pprint_dispatch, pprint_dispatch,
    set_pprint_dispatch, write,
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
fn symbol_matrix_covers_package_prefix_case_and_gensym_escaping() {
    let (runtime, mut ctx) = context();
    let common = intern(&runtime, &mut ctx, "COMMON-LISP", "MiXeD");
    let keyword = intern(&runtime, &mut ctx, "KEYWORD", "tag");
    let external = intern(&runtime, &mut ctx, "WAVE18", "name");
    let unusual_name = make_string(&mut ctx, &runtime, &['1', '2', '3']).unwrap();
    let uninterned = make_symbol(&mut ctx, &runtime, unusual_name).unwrap();

    assert_eq!(
        render(&runtime, &mut ctx, common, PrintOptions::new()),
        Ok("|MiXeD|".into())
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            common,
            PrintOptions::new()
                .with_escape(false)
                .with_case(PrintCase::Downcase),
        ),
        Ok("mixed".into())
    );
    assert_eq!(
        render(&runtime, &mut ctx, keyword, PrintOptions::new()),
        Ok(":|tag|".into())
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            external,
            PrintOptions::new().with_case(PrintCase::Capitalize),
        ),
        Ok("WAVE18:|name|".into())
    );
    assert_eq!(
        render(&runtime, &mut ctx, uninterned, PrintOptions::new()),
        Ok("#:|123|".into())
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            uninterned,
            PrintOptions::new().with_gensym(false),
        ),
        Ok("|123|".into())
    );
}

#[test]
fn string_and_character_matrix_covers_escape_control_names_and_raw_output() {
    let (runtime, mut ctx) = context();
    let text = make_string(&mut ctx, &runtime, &['"', '\\', '\n', '\t']).unwrap();
    assert_eq!(
        render(&runtime, &mut ctx, text, PrintOptions::new()),
        Ok("\"\\\"\\\\\n\t\"".into())
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            text,
            PrintOptions::new().with_escape(false),
        ),
        Ok("\"\\\n\t".into())
    );
    for (character, expected) in [
        (' ', "#\\Space"),
        ('\n', "#\\Newline"),
        ('\t', "#\\Tab"),
        ('\0', "#\\Null"),
        ('A', "#\\A"),
    ] {
        assert_eq!(
            render(
                &runtime,
                &mut ctx,
                Word::character(u32::from(character)),
                PrintOptions::new(),
            ),
            Ok(expected.into())
        );
    }
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            Word::character(u32::from('A')),
            PrintOptions::new().with_escape(false),
        ),
        Ok("A".into())
    );
}

#[test]
fn dispatch_matrix_skips_malformed_entries_and_matches_default_identity() {
    let (runtime, mut ctx) = context();
    let malformed = make_cons(&mut ctx, &runtime, Word::fixnum(99), Word::NIL).unwrap();
    let default_entry = make_cons(&mut ctx, &runtime, Word::TRUE, Word::fixnum(7)).unwrap();
    let table = make_cons(&mut ctx, &runtime, malformed, Word::NIL).unwrap();
    let table = make_cons(&mut ctx, &runtime, default_entry, table).unwrap();
    assert_eq!(
        pprint_dispatch(&mut ctx, Word::fixnum(1), table).unwrap(),
        Word::fixnum(7)
    );

    let marker = Word::fixnum(18);
    let shadowed = set_pprint_dispatch(&mut ctx, &runtime, Word::fixnum(1), marker, table).unwrap();
    assert_eq!(
        pprint_dispatch(&mut ctx, Word::fixnum(1), shadowed).unwrap(),
        marker
    );
    let copied = copy_pprint_dispatch(&mut ctx, &runtime, shadowed).unwrap();
    assert_eq!(
        pprint_dispatch(&mut ctx, Word::fixnum(2), copied).unwrap(),
        Word::fixnum(7)
    );
    assert_eq!(
        pprint_dispatch(&mut ctx, Word::fixnum(3), Word::fixnum(0)),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn standard_output_unbound_is_reported_before_printing() {
    let (runtime, mut ctx) = context();
    let standard = intern(&runtime, &mut ctx, "COMMON-LISP", "*STANDARD-OUTPUT*");
    set_symbol_value(&mut ctx, standard, Word::UNBOUND).unwrap();
    let princ = ncl_object::FunctionObject::try_from(
        runtime.function(&mut ctx, "COMMON-LISP", "PRINC").unwrap(),
    )
    .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, princ, &[Word::fixnum(4), Word::NIL]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn radix_matrix_covers_prefixes_suffix_and_clamped_constructor() {
    let (runtime, mut ctx) = context();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            Word::fixnum(31),
            PrintOptions::new().with_base(2).with_radix(true),
        ),
        Ok("#b11111".into())
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            Word::fixnum(31),
            PrintOptions::new().with_base(36).with_radix(true),
        ),
        Ok("#36rV".into())
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            Word::fixnum(-31),
            PrintOptions::new().with_base(10).with_radix(true),
        ),
        Ok("-31.".into())
    );
    assert_eq!(PrintOptions::new().try_with_base(1), None);
    assert_eq!(PrintOptions::new().try_with_base(37), None);
    assert_eq!(PrintOptions::new().try_with_base(2).unwrap().base().get(), 2);
}

#[test]
fn array_matrix_covers_empty_specialized_and_length_zero_paths() {
    let (runtime, mut ctx) = context();
    let empty_vector = make_simple_vector(&mut ctx, &runtime, &[]).unwrap();
    let empty_specialized = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Fixnum,
        &[],
    )
    .unwrap();
    let empty_array = make_array(
        &mut ctx,
        &runtime,
        &[0],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap();
    assert_eq!(
        render(&runtime, &mut ctx, empty_vector, PrintOptions::new()),
        Ok("#()".into())
    );
    assert_eq!(
        render(&runtime, &mut ctx, empty_specialized, PrintOptions::new()),
        Ok("#()".into())
    );
    assert_eq!(
        render(&runtime, &mut ctx, empty_array, PrintOptions::new()),
        Ok("#()".into())
    );
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)]).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            vector,
            PrintOptions::new().with_length(Some(0)),
        ),
        Ok("#(...)".into())
    );
}

#[test]
fn write_to_string_preserves_raw_and_escaped_forms() {
    let (runtime, mut ctx) = context();
    let text = make_string(&mut ctx, &runtime, &['a', '\\', 'b']).unwrap();
    let escaped = ncl_printer::write_to_string(&mut ctx, &runtime, text, &PrintOptions::new())
        .unwrap();
    let raw = ncl_printer::write_to_string(
        &mut ctx,
        &runtime,
        text,
        &PrintOptions::new().with_escape(false),
    )
    .unwrap();
    assert_eq!(string_value(&ctx, escaped), "\"a\\\\b\"");
    assert_eq!(string_value(&ctx, raw), "a\\b");
}
