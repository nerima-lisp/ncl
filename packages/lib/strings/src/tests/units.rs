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
    ctx.set_strict_forwarding(true);
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

fn call_unicode(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Word {
    let function = runtime
        .function(ctx, "NCL-UNICODE", name)
        .and_then(|word| FunctionObject::try_from(word).ok())
        .unwrap_or_else(|| panic!("missing unicode builtin {name}"));
    runtime
        .call_builtin(ctx, function, args)
        .unwrap_or_else(|error| panic!("{name} failed: {error:?}"))
}

fn call_unicode_result(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
) -> Result<Word, ncl_object::ObjectError> {
    let function = runtime
        .function(ctx, "NCL-UNICODE", name)
        .and_then(|word| FunctionObject::try_from(word).ok())
        .unwrap_or_else(|| panic!("missing unicode builtin {name}"));
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
    let mut scope = ncl_object::Scope::new(ctx);
    scope.context_mut().set_gc_stress(false);
    let input = scope
        .make_string(runtime, source)
        .unwrap_or_else(|error| panic!("input string: {error:?}"));
    scope
        .collect(false)
        .unwrap_or_else(|error| panic!("collect input: {error:?}"));
    let mut args = Vec::with_capacity(range.len() + 1);
    args.push(scope.get(input).as_word());
    args.extend_from_slice(range);
    scope.context_mut().set_gc_stress(true);
    let result = call_with_gc_stress(runtime, scope.context_mut(), name, &args);
    let result = scope.root::<Word>(ncl_object::Local::from_word(result));
    assert_eq!(
        string_value(scope.context(), scope.get(result).as_word()),
        expected,
        "{name} {start:?} {end:?}"
    );
}
#[test]
fn generated_unicode_categories_cover_scalar_boundaries() {
    assert_eq!(general_category('A' as u32), Some("Lu"));
    assert_eq!(general_category('a' as u32), Some("Ll"));
    assert_eq!(general_category(0xD800), None);
    assert_eq!(general_category(0x11_0000), None);
}

#[test]
fn character_predicates_conversions_and_comparisons_return_expected_values() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    crate::register(&runtime).unwrap_or_else(|error| panic!("register: {error:?}"));

    let a = Word::character('a' as u32);
    let b = Word::character('B' as u32);
    let one = Word::character('1' as u32);
    assert_eq!(call(&runtime, &mut ctx, "CHARACTERP", &[a]), Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, "ALPHA-CHAR-P", &[a]), Word::TRUE);
    assert_eq!(
        call(&runtime, &mut ctx, "ALPHANUMERICP", &[one]),
        Word::TRUE
    );
    assert_eq!(call(&runtime, &mut ctx, "UPPER-CASE-P", &[b]), Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, "LOWER-CASE-P", &[a]), Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, "BOTH-CASE-P", &[a]), Word::TRUE);
    assert_eq!(
        call(&runtime, &mut ctx, "CHAR-UPCASE", &[a]),
        Word::character('A' as u32)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "CHAR-DOWNCASE", &[b]),
        Word::character('b' as u32)
    );
    assert_eq!(call(&runtime, &mut ctx, "CHAR-INT", &[b]), Word::fixnum(66));
    assert_eq!(
        call(&runtime, &mut ctx, "CODE-CHAR", &[Word::fixnum(66)]),
        b
    );
    assert_eq!(
        call(&runtime, &mut ctx, "DIGIT-CHAR-P", &[one, Word::fixnum(10)]),
        Word::fixnum(1)
    );
    assert_eq!(call(&runtime, &mut ctx, "DIGIT-CHAR-P", &[a]), Word::NIL);
    assert_eq!(call(&runtime, &mut ctx, "CHAR=", &[a, a]), Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, "CHAR<", &[a, b]), Word::NIL);
    assert_eq!(call(&runtime, &mut ctx, "CHAR-EQUAL", &[a, b]), Word::NIL);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "CHAR-EQUAL",
            &[a, Word::character('A' as u32)]
        ),
        Word::TRUE
    );
}

#[test]
fn unicode_transforms_and_encoding_validate_outputs_and_errors() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    crate::register(&runtime).unwrap_or_else(|error| panic!("register: {error:?}"));
    let text = ncl_object::make_string(&mut ctx, &runtime, &['é', ' ', 'ß'])
        .unwrap_or_else(|error| panic!("string: {error:?}"));
    let nfc = call_unicode(&runtime, &mut ctx, "NORMALIZE-NFC", &[text]);
    assert_eq!(string_value(&ctx, nfc), "é ß");
    let upper = call_unicode(&runtime, &mut ctx, "FULL-UPCASE", &[text]);
    assert_eq!(string_value(&ctx, upper), "É SS");
    let bytes = call_unicode(&runtime, &mut ctx, "STRING-TO-UTF8", &[text]);
    assert_eq!(ncl_object::simple_vector_length(&ctx, bytes), Ok(5));
    let round_trip = call_unicode(&runtime, &mut ctx, "UTF8-TO-STRING", &[bytes]);
    assert_eq!(string_value(&ctx, round_trip), "é ß");
    let category = call_unicode(
        &runtime,
        &mut ctx,
        "GENERAL-CATEGORY",
        &[Word::character('A' as u32)],
    );
    assert_eq!(string_value(&ctx, category), "Lu");
    assert_eq!(
        call_result(&runtime, &mut ctx, "CODE-CHAR", &[Word::fixnum(-1)]),
        Err(ncl_object::ObjectError::TypeError)
    );
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

    let initial_element = keyword(&mut ctx, &runtime, "INITIAL-ELEMENT");
    let made = call(
        &runtime,
        &mut ctx,
        "MAKE-STRING",
        &[
            Word::fixnum(3),
            initial_element,
            Word::character('x' as u32),
        ],
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
    let made = call(&runtime, &mut ctx, "MAKE-STRING", &[Word::fixnum(4)]);
    let mut scope = ncl_object::Scope::new(&mut ctx);
    let made = scope.root::<Word>(ncl_object::Local::from_word(made));
    assert_eq!(
        ncl_object::string_length(scope.context(), scope.get(made).as_word()),
        Ok(4)
    );
    assert_eq!(
        ncl_object::string_ref(scope.context(), scope.get(made).as_word(), 0),
        Ok(' ')
    );
    assert_eq!(
        call(
            &runtime,
            scope.context_mut(),
            "DIGIT-CHAR-P",
            &[Word::character('A' as u32), Word::fixnum(16)],
        ),
        Word::fixnum(10)
    );
}

#[test]
fn case_conversion_builtins_survive_gc_stress_with_ranges() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    crate::register(&runtime).unwrap_or_else(|error| panic!("register: {error:?}"));
    let start = keyword(&mut ctx, &runtime, "START");
    let end = keyword(&mut ctx, &runtime, "END");
    let mut scope = ncl_object::Scope::new(&mut ctx);
    let start = scope.root::<Word>(ncl_object::Local::from_word(start));
    let end = scope.root::<Word>(ncl_object::Local::from_word(end));
    scope.context_mut().set_gc_stress(true);
    scope.context_mut().set_strict_forwarding(true);

    let source = ['a', 'B', ' ', 'C', 'D'];
    let ranges = ["full", "start", "end", "start-end"];
    let cases = [
        ("STRING-UPCASE", ["AB CD", "aB CD", "AB CD", "aB CD"]),
        ("STRING-DOWNCASE", ["ab cd", "ab cd", "ab cD", "ab cD"]),
        ("STRING-CAPITALIZE", ["Ab Cd", "aB Cd", "Ab CD", "aB CD"]),
        ("NSTRING-UPCASE", ["AB CD", "aB CD", "AB CD", "aB CD"]),
        ("NSTRING-DOWNCASE", ["ab cd", "ab cd", "ab cD", "ab cD"]),
        ("NSTRING-CAPITALIZE", ["Ab Cd", "aB Cd", "Ab CD", "aB CD"]),
    ];
    for (name, expected) in cases {
        for (range_name, expected) in ranges.iter().zip(expected) {
            let range = match *range_name {
                "full" => Vec::new(),
                "start" => vec![scope.get(start).as_word(), Word::fixnum(1)],
                "end" => vec![scope.get(end).as_word(), Word::fixnum(4)],
                "start-end" => vec![
                    scope.get(start).as_word(),
                    Word::fixnum(1),
                    scope.get(end).as_word(),
                    Word::fixnum(4),
                ],
                _ => unreachable!(),
            };
            assert_case_result(
                &runtime,
                scope.context_mut(),
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
                &range,
            );
        }
    }
}

#[test]
#[allow(
    clippy::similar_names,
    clippy::too_many_lines,
    reason = "the test covers string comparison and Unicode boundary contracts"
)]
fn string_comparisons_ranges_and_unicode_boundaries_are_observable() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    crate::register(&runtime).unwrap_or_else(|error| panic!("register: {error:?}"));

    let left = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b', 'c'])
        .unwrap_or_else(|error| panic!("left: {error:?}"));
    let right = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b', 'd'])
        .unwrap_or_else(|error| panic!("right: {error:?}"));
    assert_eq!(
        call(&runtime, &mut ctx, "STRING<", &[left, right]),
        Word::fixnum(2)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "STRING/=", &[left, right]),
        Word::fixnum(2)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "STRING<=", &[left, right]),
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "STRING>=", &[right, left]),
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "STRING>", &[right, left]),
        Word::fixnum(2)
    );

    let upper = ncl_object::make_string(&mut ctx, &runtime, &['A', 'B', 'C'])
        .unwrap_or_else(|error| panic!("upper: {error:?}"));
    assert_eq!(
        call(&runtime, &mut ctx, "STRING=", &[left, upper]),
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, "STRING-EQUAL", &[left, upper]),
        Word::TRUE
    );

    let start = keyword(&mut ctx, &runtime, "START1");
    let end = keyword(&mut ctx, &runtime, "END1");
    let start2 = keyword(&mut ctx, &runtime, "START2");
    let end2 = keyword(&mut ctx, &runtime, "END2");
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "STRING-EQUAL",
            &[
                left,
                right,
                start,
                Word::fixnum(0),
                end,
                Word::fixnum(2),
                start2,
                Word::fixnum(0),
                end2,
                Word::fixnum(2),
            ],
        ),
        Word::TRUE
    );
    assert_eq!(
        call_result(
            &runtime,
            &mut ctx,
            "STRING-EQUAL",
            &[left, right, start, Word::fixnum(4)],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );

    let composed = ncl_object::make_string(&mut ctx, &runtime, &['é'])
        .unwrap_or_else(|error| panic!("composed: {error:?}"));
    let nfd = call_unicode(&runtime, &mut ctx, "NORMALIZE-NFD", &[composed]);
    assert_eq!(ncl_object::string_length(&ctx, nfd), Ok(2));
    let compatibility = ncl_object::make_string(&mut ctx, &runtime, &['①'])
        .unwrap_or_else(|error| panic!("compatibility: {error:?}"));
    let nfkc = call_unicode(&runtime, &mut ctx, "NORMALIZE-NFKC", &[compatibility]);
    assert_eq!(string_value(&ctx, nfkc), "1");
    let nfkd = call_unicode(&runtime, &mut ctx, "NORMALIZE-NFKD", &[compatibility]);
    assert_eq!(string_value(&ctx, nfkd), "1");
    let title = ncl_object::make_string(
        &mut ctx,
        &runtime,
        &['h', 'i', ' ', 'W', 'O', 'R', 'L', 'D'],
    )
    .unwrap_or_else(|error| panic!("title: {error:?}"));
    let title_case = call_unicode(&runtime, &mut ctx, "FULL-TITLECASE", &[title]);
    assert_eq!(string_value(&ctx, title_case), "Hi World");

    let graphemes = ncl_object::make_string(&mut ctx, &runtime, &['a', '\u{301}', '🇯', '🇵'])
        .unwrap_or_else(|error| panic!("graphemes: {error:?}"));
    let boundaries = call_unicode(&runtime, &mut ctx, "GRAPHEME-BOUNDARIES", &[graphemes]);
    assert_eq!(ncl_object::simple_vector_length(&ctx, boundaries), Ok(3));
    assert_eq!(
        ncl_object::simple_vector_ref(&ctx, boundaries, 1),
        Ok(Word::fixnum(2))
    );
    let invalid = ncl_object::make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(255)])
        .unwrap_or_else(|error| panic!("invalid bytes: {error:?}"));
    assert_eq!(
        call_unicode_result(&runtime, &mut ctx, "UTF8-TO-STRING", &[invalid]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn string_designators_comparators_and_trim_directions_cover_edges() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    crate::register(&runtime).unwrap_or_else(|error| panic!("register: {error:?}"));

    let a = Word::character('a' as u32);
    let b = Word::character('b' as u32);
    let upper_a = Word::character('A' as u32);
    assert_eq!(call(&runtime, &mut ctx, "CHAR/=", &[a, b]), Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, "CHAR>", &[b, a]), Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, "CHAR<=", &[a, a]), Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, "CHAR>=", &[a, a]), Word::TRUE);
    assert_eq!(
        call(&runtime, &mut ctx, "CHAR-NOT-EQUAL", &[a, upper_a]),
        Word::NIL
    );
    assert_eq!(call(&runtime, &mut ctx, "CHAR-LESSP", &[a, b]), Word::TRUE);
    assert_eq!(
        call(&runtime, &mut ctx, "CHAR-GREATERP", &[b, a]),
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "CHAR-NOT-LESSP", &[a, a]),
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "CHAR-NOT-GREATERP", &[a, a]),
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "CHAR-EQUAL", &[a, upper_a]),
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "CHAR-NOT-EQUAL", &[a, b]),
        Word::TRUE
    );

    let one_char = ncl_object::make_string(&mut ctx, &runtime, &['x'])
        .unwrap_or_else(|error| panic!("one-char string: {error:?}"));
    let many_chars = ncl_object::make_string(&mut ctx, &runtime, &['x', 'y'])
        .unwrap_or_else(|error| panic!("many-char string: {error:?}"));
    assert_eq!(
        call(&runtime, &mut ctx, "CHARACTER", &[one_char]),
        Word::character('x' as u32)
    );
    assert_eq!(
        call_result(&runtime, &mut ctx, "CHARACTER", &[many_chars]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(call(&runtime, &mut ctx, "CHARACTER", &[a]), a);
    assert_eq!(call(&runtime, &mut ctx, "STRINGP", &[one_char]), Word::TRUE);
    assert_eq!(
        call(&runtime, &mut ctx, "STRINGP", &[Word::TRUE]),
        Word::NIL
    );

    let symbol = keyword(&mut ctx, &runtime, "SymbolName");
    let symbol_name = call(&runtime, &mut ctx, "STRING", &[symbol]);
    assert_eq!(string_value(&ctx, symbol_name), "SymbolName");
    assert_eq!(
        call(&runtime, &mut ctx, "CHAR", &[symbol, Word::fixnum(0)]),
        Word::character('S' as u32)
    );
    assert_eq!(
        call_result(&runtime, &mut ctx, "STRING", &[Word::TRUE]),
        Err(ncl_object::ObjectError::TypeError)
    );

    let left = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b'])
        .unwrap_or_else(|error| panic!("left: {error:?}"));
    let right = ncl_object::make_string(&mut ctx, &runtime, &['a', 'c'])
        .unwrap_or_else(|error| panic!("right: {error:?}"));
    assert_eq!(
        call(&runtime, &mut ctx, "STRING/=", &[left, right]),
        Word::fixnum(1)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "STRING<", &[left, right]),
        Word::fixnum(1)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "STRING>", &[right, left]),
        Word::fixnum(1)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "STRING<=", &[left, left]),
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "STRING>=", &[left, left]),
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "STRING-NOT-EQUAL", &[left, right]),
        Word::fixnum(1)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "STRING-LESSP", &[left, right]),
        Word::fixnum(1)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "STRING-GREATERP", &[right, left]),
        Word::fixnum(1)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "STRING-NOT-LESSP", &[left, left]),
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "STRING-NOT-GREATERP", &[left, left]),
        Word::TRUE
    );
    let unknown = keyword(&mut ctx, &runtime, "UNKNOWN");
    assert_eq!(
        call_result(
            &runtime,
            &mut ctx,
            "STRING=",
            &[left, right, unknown, Word::fixnum(0)],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );

    let padded = ncl_object::make_string(&mut ctx, &runtime, &['.', ' ', 'x', ' ', '.'])
        .unwrap_or_else(|error| panic!("padded: {error:?}"));
    let bag = ncl_object::make_string(&mut ctx, &runtime, &['.', ' '])
        .unwrap_or_else(|error| panic!("bag: {error:?}"));
    let left_trimmed = call(&runtime, &mut ctx, "STRING-LEFT-TRIM", &[bag, padded]);
    assert_eq!(string_value(&ctx, left_trimmed), "x .");
    let right_trimmed = call(&runtime, &mut ctx, "STRING-RIGHT-TRIM", &[bag, padded]);
    assert_eq!(string_value(&ctx, right_trimmed), ". x");
    let trimmed = call(&runtime, &mut ctx, "STRING-TRIM", &[bag, padded]);
    assert_eq!(string_value(&ctx, trimmed), "x");
    assert_eq!(
        call_result(&runtime, &mut ctx, "MAKE-STRING", &[Word::fixnum(-1)]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn unicode_case_normalization_and_grapheme_edges_are_distinct() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    crate::register(&runtime).unwrap_or_else(|error| panic!("register: {error:?}"));

    let decomposed = ncl_object::make_string(&mut ctx, &runtime, &['e', '\u{301}'])
        .unwrap_or_else(|error| panic!("decomposed: {error:?}"));
    let nfc = call_unicode(&runtime, &mut ctx, "NORMALIZE-NFC", &[decomposed]);
    assert_eq!(string_value(&ctx, nfc), "é");
    let upper = ncl_object::make_string(&mut ctx, &runtime, &['É'])
        .unwrap_or_else(|error| panic!("upper: {error:?}"));
    let downcase = call_unicode(&runtime, &mut ctx, "FULL-DOWNCASE", &[upper]);
    assert_eq!(string_value(&ctx, downcase), "é");

    let clusters = ncl_object::make_string(
        &mut ctx,
        &runtime,
        &[
            '👩', '\u{200d}', '💻', '🇯', '🇵', '🇦', '🏳', '\u{fe0f}', '\u{200d}', '🌈',
        ],
    )
    .unwrap_or_else(|error| panic!("clusters: {error:?}"));
    let boundaries = call_unicode(&runtime, &mut ctx, "GRAPHEME-BOUNDARIES", &[clusters]);
    let actual: Vec<_> = (0..ncl_object::simple_vector_length(&ctx, boundaries)
        .unwrap_or_else(|error| panic!("boundary length: {error:?}")))
        .map(|index| {
            ncl_object::simple_vector_ref(&ctx, boundaries, index)
                .unwrap_or_else(|error| panic!("boundary: {error:?}"))
                .as_fixnum()
                .unwrap_or_else(|| panic!("boundary is not a fixnum"))
        })
        .collect();
    assert_eq!(actual, vec![0, 3, 5, 6, 10]);

    let invalid_value = ncl_object::make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(256)])
        .unwrap_or_else(|error| panic!("invalid value: {error:?}"));
    assert_eq!(
        call_unicode_result(&runtime, &mut ctx, "UTF8-TO-STRING", &[invalid_value]),
        Err(ncl_object::ObjectError::TypeError)
    );
}
