use crate::general_category;
use ncl_object::{FunctionObject, Package, Runtime, ThreadContext, Word};

fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Word {
    let function = runtime
        .function(ctx, "COMMON-LISP", name)
        .and_then(|word| FunctionObject::try_from(word).ok())
        .unwrap_or_else(|| panic!("missing builtin {name}"));
    runtime
        .call_builtin(ctx, function, args)
        .unwrap_or_else(|error| panic!("{name} failed: {error:?}"))
}

fn call_with_gc_stress(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
) -> Word {
    ctx.set_gc_stress(false);
    let function = runtime
        .function(ctx, "COMMON-LISP", name)
        .and_then(|word| FunctionObject::try_from(word).ok())
        .unwrap_or_else(|| panic!("missing builtin {name}"));
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(false);
    runtime
        .call_builtin(ctx, function, args)
        .unwrap_or_else(|error| panic!("{name} failed: {error:?}"))
}

fn call_result(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
) -> Result<Word, ncl_object::ObjectError> {
    let function = runtime
        .function(ctx, "COMMON-LISP", name)
        .and_then(|word| FunctionObject::try_from(word).ok())
        .unwrap_or_else(|| panic!("missing builtin {name}"));
    runtime.call_builtin(ctx, function, args)
}

fn keyword(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
    let package = runtime
        .find_package(ctx, "KEYWORD")
        .unwrap_or_else(|| panic!("KEYWORD package missing"));
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap_or_else(|error| panic!("intern keyword {name}: {error:?}"))
        .0
}

fn string_value(ctx: &ThreadContext, string: Word) -> String {
    let length = ncl_object::string_length(ctx, string)
        .unwrap_or_else(|error| panic!("string length: {error:?}"));
    (0..length)
        .map(|index| {
            ncl_object::string_ref(ctx, string, index)
                .unwrap_or_else(|error| panic!("string ref: {error:?}"))
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn assert_case_result(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    source: &[char],
    expected: &str,
    start: Word,
    end: Word,
    range: &[Word],
) {
    ctx.set_gc_stress(false);
    let mut input = ncl_object::make_string(ctx, runtime, source)
        .unwrap_or_else(|error| panic!("input string: {error:?}"));
    let input_token = ncl_object::push_root(ctx, &mut input);
    ctx.collect(false)
        .unwrap_or_else(|error| panic!("collect input: {error:?}"));
    let mut args = Vec::with_capacity(range.len() + 1);
    args.push(input);
    args.extend_from_slice(range);
    ctx.set_gc_stress(true);
    let result = call_with_gc_stress(runtime, ctx, name, &args);
    let mut result = result;
    let result_token = ncl_object::push_root(ctx, &mut result);
    assert_eq!(
        string_value(ctx, result),
        expected,
        "{name} {start:?} {end:?}"
    );
    assert!(ncl_object::pop_root(ctx, result_token));
    assert!(ncl_object::pop_root(ctx, input_token));
}
#[test]
fn generated_unicode_categories_cover_scalar_boundaries() {
    assert_eq!(general_category('A' as u32), Some("Lu"));
    assert_eq!(general_category('a' as u32), Some("Ll"));
    assert_eq!(general_category(0xD800), None);
    assert_eq!(general_category(0x11_0000), None);
}

#[test]
fn string_builtins_cover_comparison_case_trim_and_construction() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    crate::register(&runtime).unwrap_or_else(|error| panic!("register: {error:?}"));

    let hello = ncl_object::make_string(&mut ctx, &runtime, &['H', 'i'])
        .unwrap_or_else(|error| panic!("string: {error:?}"));
    let hi = ncl_object::make_string(&mut ctx, &runtime, &['h', 'i'])
        .unwrap_or_else(|error| panic!("string: {error:?}"));
    assert_eq!(
        call(&runtime, &mut ctx, "STRING-EQUAL", &[hello, hi]),
        Word::TRUE
    );

    let upper = call(&runtime, &mut ctx, "STRING-UPCASE", &[hi]);
    assert_eq!(ncl_object::string_length(&ctx, upper), Ok(2));
    assert_eq!(ncl_object::string_ref(&ctx, upper, 0), Ok('H'));

    let padded = ncl_object::make_string(&mut ctx, &runtime, &[' ', 'H', 'i', ' '])
        .unwrap_or_else(|error| panic!("string: {error:?}"));
    let spaces = ncl_object::make_string(&mut ctx, &runtime, &[' '])
        .unwrap_or_else(|error| panic!("string: {error:?}"));
    let trimmed = call(&runtime, &mut ctx, "STRING-TRIM", &[spaces, padded]);
    assert_eq!(ncl_object::string_ref(&ctx, trimmed, 0), Ok('H'));

    let made = call(
        &runtime,
        &mut ctx,
        "MAKE-STRING",
        &[Word::fixnum(3), Word::character('x' as u32)],
    );
    assert_eq!(ncl_object::string_ref(&ctx, made, 2), Ok('x'));
}

#[test]
#[allow(clippy::too_many_lines)]
fn character_and_mutating_string_builtins_cover_boundaries() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    crate::register(&runtime).unwrap_or_else(|error| panic!("register: {error:?}"));

    let source = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b'])
        .unwrap_or_else(|error| panic!("string: {error:?}"));
    assert_eq!(
        call(&runtime, &mut ctx, "CHAR", &[source, Word::fixnum(1)]),
        Word::character('b' as u32)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SCHAR", &[source, Word::fixnum(0)]),
        Word::character('a' as u32)
    );
    assert_eq!(
        call_result(&runtime, &mut ctx, "CHAR", &[source, Word::fixnum(2)]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        call_result(&runtime, &mut ctx, "CHAR", &[Word::TRUE, Word::fixnum(0)]),
        Err(ncl_object::ObjectError::TypeError)
    );

    let char_name = call(
        &runtime,
        &mut ctx,
        "CHAR-NAME",
        &[Word::character(' ' as u32)],
    );
    assert_eq!(ncl_object::string_length(&ctx, char_name), Ok(5));
    assert_eq!(
        (0..5)
            .map(|index| {
                ncl_object::string_ref(&ctx, char_name, index)
                    .unwrap_or_else(|error| panic!("string-ref: {error:?}"))
            })
            .collect::<String>(),
        "SPACE"
    );
    let name = ncl_object::make_string(&mut ctx, &runtime, &['t', 'a', 'b'])
        .unwrap_or_else(|error| panic!("string: {error:?}"));
    assert_eq!(
        call(&runtime, &mut ctx, "NAME-CHAR", &[name]),
        Word::character('\t' as u32)
    );
    let unknown = ncl_object::make_string(&mut ctx, &runtime, &['N', 'O', 'P', 'E'])
        .unwrap_or_else(|error| panic!("string: {error:?}"));
    assert_eq!(call(&runtime, &mut ctx, "NAME-CHAR", &[unknown]), Word::NIL);

    assert_eq!(
        call(&runtime, &mut ctx, "DIGIT-CHAR", &[Word::fixnum(15)]),
        Word::NIL
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "DIGIT-CHAR",
            &[Word::fixnum(15), Word::fixnum(16)]
        ),
        Word::character('F' as u32)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "DIGIT-CHAR",
            &[Word::fixnum(10), Word::fixnum(10)]
        ),
        Word::NIL
    );
    assert_eq!(
        call_result(
            &runtime,
            &mut ctx,
            "DIGIT-CHAR",
            &[Word::fixnum(1), Word::fixnum(1)]
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "GRAPHIC-CHAR-P",
            &[Word::character('A' as u32)]
        ),
        Word::TRUE
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "GRAPHIC-CHAR-P",
            &[Word::character('\n' as u32)]
        ),
        Word::NIL
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "STANDARD-CHAR-P",
            &[Word::character('~' as u32)]
        ),
        Word::TRUE
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "STANDARD-CHAR-P",
            &[Word::character('\u{1b}' as u32)]
        ),
        Word::NIL
    );

    let mutable = ncl_object::make_string(
        &mut ctx,
        &runtime,
        &['h', 'I', ' ', 'T', 'H', 'E', 'R', 'E'],
    )
    .unwrap_or_else(|error| panic!("string: {error:?}"));
    assert_eq!(
        call(&runtime, &mut ctx, "NSTRING-UPCASE", &[mutable]),
        mutable
    );
    assert_eq!(ncl_object::string_ref(&ctx, mutable, 0), Ok('H'));
    assert_eq!(
        call(&runtime, &mut ctx, "NSTRING-DOWNCASE", &[mutable]),
        mutable
    );
    assert_eq!(ncl_object::string_ref(&ctx, mutable, 1), Ok('i'));
    assert_eq!(
        call(&runtime, &mut ctx, "NSTRING-CAPITALIZE", &[mutable]),
        mutable
    );
    assert_eq!(ncl_object::string_ref(&ctx, mutable, 0), Ok('H'));
    assert_eq!(ncl_object::string_ref(&ctx, mutable, 1), Ok('i'));
    assert_eq!(
        call(&runtime, &mut ctx, "SIMPLE-STRING-P", &[mutable]),
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SIMPLE-STRING-P", &[Word::TRUE]),
        Word::NIL
    );
}

#[test]
fn string_allocations_survive_gc_stress_and_strict_forwarding() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    crate::register(&runtime).unwrap_or_else(|error| panic!("register: {error:?}"));
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);

    let made = call(
        &runtime,
        &mut ctx,
        "MAKE-STRING",
        &[Word::fixnum(4), Word::character('x' as u32)],
    );
    let mut made = made;
    let token = ncl_object::push_root(&mut ctx, &mut made);
    assert_eq!(ncl_object::string_length(&ctx, made), Ok(4));
    assert_eq!(ncl_object::string_ref(&ctx, made, 0), Ok('x'));
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "DIGIT-CHAR-P",
            &[Word::character('A' as u32), Word::fixnum(16)],
        ),
        Word::fixnum(10)
    );
    assert!(ncl_object::pop_root(&mut ctx, token));
}

#[test]
fn case_conversion_builtins_survive_gc_stress_with_ranges() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    crate::register(&runtime).unwrap_or_else(|error| panic!("register: {error:?}"));
    let mut start = keyword(&mut ctx, &runtime, "START");
    let start_token = ncl_object::push_root(&mut ctx, &mut start);
    let mut end = keyword(&mut ctx, &runtime, "END");
    let end_token = ncl_object::push_root(&mut ctx, &mut end);
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);

    let source = ['a', 'B', ' ', 'C', 'D'];
    let ranges = [
        ("full", &[][..]),
        ("start", &[start, Word::fixnum(1)][..]),
        ("end", &[end, Word::fixnum(4)][..]),
        (
            "start-end",
            &[start, Word::fixnum(1), end, Word::fixnum(4)][..],
        ),
    ];
    let cases = [
        ("STRING-UPCASE", ["AB CD", "aB CD", "AB CD", "aB CD"]),
        ("STRING-DOWNCASE", ["ab cd", "ab cd", "ab cD", "ab cD"]),
        ("STRING-CAPITALIZE", ["Ab Cd", "aB Cd", "Ab CD", "aB CD"]),
        ("NSTRING-UPCASE", ["AB CD", "aB CD", "AB CD", "aB CD"]),
        ("NSTRING-DOWNCASE", ["ab cd", "ab cd", "ab cD", "ab cD"]),
        ("NSTRING-CAPITALIZE", ["Ab Cd", "aB Cd", "Ab CD", "aB CD"]),
    ];
    for (name, expected) in cases {
        for ((range_name, range), expected) in ranges.iter().zip(expected) {
            assert_case_result(
                &runtime,
                &mut ctx,
                name,
                &source,
                expected,
                if *range_name == "start" || *range_name == "start-end" {
                    Word::fixnum(1)
                } else {
                    Word::fixnum(0)
                },
                if *range_name == "end" || *range_name == "start-end" {
                    Word::fixnum(4)
                } else {
                    Word::fixnum(5)
                },
                range,
            );
        }
    }
    assert!(ncl_object::pop_root(&mut ctx, end_token));
    assert!(ncl_object::pop_root(&mut ctx, start_token));
}
