use ncl_object::{FunctionObject, Package, Runtime, ThreadContext, Word};

fn runtime() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    crate::register(&runtime).unwrap_or_else(|error| panic!("register: {error:?}"));
    (runtime, ctx)
}

fn call(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    package: &str,
    name: &str,
    args: &[Word],
) -> Word {
    let function = runtime
        .function(ctx, package, name)
        .and_then(|word| FunctionObject::try_from(word).ok())
        .unwrap_or_else(|| panic!("missing builtin {package}:{name}"));
    runtime
        .call_builtin(ctx, function, args)
        .unwrap_or_else(|error| panic!("{package}:{name} failed: {error:?}"))
}

fn call_result(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    package: &str,
    name: &str,
    args: &[Word],
) -> Result<Word, ncl_object::ObjectError> {
    let function = runtime
        .function(ctx, package, name)
        .and_then(|word| FunctionObject::try_from(word).ok())
        .unwrap_or_else(|| panic!("missing builtin {package}:{name}"));
    runtime.call_builtin(ctx, function, args)
}

fn text(ctx: &ThreadContext, value: Word) -> String {
    let length = ncl_object::string_length(ctx, value)
        .unwrap_or_else(|error| panic!("string length: {error:?}"));
    (0..length)
        .map(|index| {
            ncl_object::string_ref(ctx, value, index)
                .unwrap_or_else(|error| panic!("string ref: {error:?}"))
        })
        .collect()
}

fn call_text(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    package: &str,
    name: &str,
    args: &[Word],
) -> String {
    let result = call(runtime, ctx, package, name, args);
    text(ctx, result)
}

fn string(ctx: &mut ThreadContext, runtime: &Runtime, value: &str) -> Word {
    ncl_object::make_string(ctx, runtime, &value.chars().collect::<Vec<_>>())
        .unwrap_or_else(|error| panic!("string: {error:?}"))
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

fn vector_values(ctx: &ThreadContext, value: Word) -> Vec<Word> {
    (0..ncl_object::simple_vector_length(ctx, value)
        .unwrap_or_else(|error| panic!("vector length: {error:?}")))
        .map(|index| {
            ncl_object::simple_vector_ref(ctx, value, index)
                .unwrap_or_else(|error| panic!("vector ref: {error:?}"))
        })
        .collect()
}

#[test]
#[allow(clippy::too_many_lines)]
fn character_builtins_cover_predicates_conversions_and_names() {
    let (runtime, mut ctx) = runtime();
    let a = Word::character('A' as u32);
    let b = Word::character('b' as u32);
    let lower_a = Word::character('a' as u32);
    let upper_b = Word::character('B' as u32);
    let one = Word::character('1' as u32);
    assert_eq!(
        call(&runtime, &mut ctx, "COMMON-LISP", "CHARACTERP", &[a]),
        Word::TRUE
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "COMMON-LISP",
            "CHARACTERP",
            &[Word::TRUE]
        ),
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, "COMMON-LISP", "CHARACTER", &[a]),
        a
    );
    assert_eq!(
        call(&runtime, &mut ctx, "COMMON-LISP", "CHAR-UPCASE", &[b]),
        upper_b
    );
    assert_eq!(
        call(&runtime, &mut ctx, "COMMON-LISP", "CHAR-DOWNCASE", &[a]),
        lower_a
    );
    assert_eq!(
        call(&runtime, &mut ctx, "COMMON-LISP", "CHAR-CODE", &[a]),
        Word::fixnum(65)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "COMMON-LISP", "CHAR-INT", &[a]),
        Word::fixnum(65)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "COMMON-LISP",
            "CODE-CHAR",
            &[Word::fixnum(0x03bb)]
        ),
        Word::character('λ' as u32)
    );
    assert_eq!(
        call_result(
            &runtime,
            &mut ctx,
            "COMMON-LISP",
            "CODE-CHAR",
            &[Word::fixnum(0x11_0000)]
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    for (character, name) in [
        ('\0', "NULL"),
        ('\u{7}', "BELL"),
        ('\t', "TAB"),
        ('\n', "LINEFEED"),
        (' ', "SPACE"),
        ('\u{7f}', "RUBOUT"),
    ] {
        let result = call(
            &runtime,
            &mut ctx,
            "COMMON-LISP",
            "CHAR-NAME",
            &[Word::character(character as u32)],
        );
        assert_eq!(text(&ctx, result), name);
        let name_string = string(&mut ctx, &runtime, name);
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "COMMON-LISP",
                "NAME-CHAR",
                &[name_string]
            ),
            Word::character(character as u32)
        );
    }
    let unknown_name = string(&mut ctx, &runtime, "nope");
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "COMMON-LISP",
            "NAME-CHAR",
            &[unknown_name]
        ),
        Word::NIL
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "COMMON-LISP",
            "DIGIT-CHAR",
            &[Word::fixnum(10), Word::fixnum(16)]
        ),
        Word::character('A' as u32)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "COMMON-LISP", "DIGIT-CHAR-P", &[one]),
        Word::fixnum(1)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "COMMON-LISP",
            "DIGIT-CHAR-P",
            &[Word::character('z' as u32), Word::fixnum(36)]
        ),
        Word::fixnum(35)
    );
    for (name, value, expected) in [
        ("ALPHA-CHAR-P", a, Word::TRUE),
        ("ALPHANUMERICP", one, Word::TRUE),
        ("UPPER-CASE-P", a, Word::TRUE),
        ("LOWER-CASE-P", b, Word::TRUE),
        ("BOTH-CASE-P", b, Word::TRUE),
        ("GRAPHIC-CHAR-P", Word::character('\n' as u32), Word::NIL),
        (
            "STANDARD-CHAR-P",
            Word::character('\u{1b}' as u32),
            Word::NIL,
        ),
    ] {
        assert_eq!(
            call(&runtime, &mut ctx, "COMMON-LISP", name, &[value]),
            expected,
            "{name}"
        );
    }
    for (name, expected) in [
        ("CHAR=", Word::TRUE),
        ("CHAR/=", Word::TRUE),
        ("CHAR<", Word::TRUE),
        ("CHAR>", Word::TRUE),
        ("CHAR<=", Word::TRUE),
        ("CHAR>=", Word::TRUE),
        ("CHAR-EQUAL", Word::TRUE),
        ("CHAR-NOT-EQUAL", Word::TRUE),
        ("CHAR-LESSP", Word::TRUE),
        ("CHAR-GREATERP", Word::TRUE),
        ("CHAR-NOT-GREATERP", Word::TRUE),
        ("CHAR-NOT-LESSP", Word::TRUE),
    ] {
        let args = if name == "CHAR=" || name == "CHAR-EQUAL" {
            &[a, a][..]
        } else if matches!(
            name,
            "CHAR>" | "CHAR>=" | "CHAR-GREATERP" | "CHAR-NOT-LESSP"
        ) {
            &[b, a][..]
        } else {
            &[a, b][..]
        };
        assert_eq!(
            call(&runtime, &mut ctx, "COMMON-LISP", name, args),
            expected,
            "{name}"
        );
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn string_comparisons_ranges_and_trimming_return_values() {
    let (runtime, mut ctx) = runtime();
    let left = string(&mut ctx, &runtime, "abC");
    let right = string(&mut ctx, &runtime, "abc");
    let start1 = keyword(&mut ctx, &runtime, "START1");
    let end1 = keyword(&mut ctx, &runtime, "END1");
    let start2 = keyword(&mut ctx, &runtime, "START2");
    let end2 = keyword(&mut ctx, &runtime, "END2");
    assert_eq!(
        call(&runtime, &mut ctx, "COMMON-LISP", "STRING=", &[left, right]),
        Word::NIL
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "COMMON-LISP",
            "STRING/=",
            &[left, right]
        ),
        Word::fixnum(2)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "COMMON-LISP",
            "STRING-EQUAL",
            &[left, right]
        ),
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "COMMON-LISP", "STRING<", &[left, right]),
        Word::fixnum(2)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "COMMON-LISP",
            "STRING-GREATERP",
            &[right, left]
        ),
        Word::NIL
    );
    let range = [
        left,
        right,
        start1,
        Word::fixnum(1),
        end1,
        Word::fixnum(3),
        start2,
        Word::fixnum(1),
        end2,
        Word::fixnum(3),
    ];
    assert_eq!(
        call(&runtime, &mut ctx, "COMMON-LISP", "STRING-EQUAL", &range),
        Word::TRUE
    );
    for (name, expected) in [
        ("STRING-UPCASE", "aBC"),
        ("STRING-DOWNCASE", "abc"),
        ("STRING-CAPITALIZE", "aBc"),
    ] {
        let start = keyword(&mut ctx, &runtime, "START");
        let result = call(
            &runtime,
            &mut ctx,
            "COMMON-LISP",
            name,
            &[left, start, Word::fixnum(1)],
        );
        assert_eq!(text(&ctx, result), expected, "{name}");
    }
    let bag = string(&mut ctx, &runtime, " .");
    let input = string(&mut ctx, &runtime, " .hello. ");
    assert_eq!(
        call_text(
            &runtime,
            &mut ctx,
            "COMMON-LISP",
            "STRING-TRIM",
            &[bag, input]
        ),
        "hello"
    );
    assert_eq!(
        call_text(
            &runtime,
            &mut ctx,
            "COMMON-LISP",
            "STRING-LEFT-TRIM",
            &[bag, input]
        ),
        "hello. "
    );
    assert_eq!(
        call_text(
            &runtime,
            &mut ctx,
            "COMMON-LISP",
            "STRING-RIGHT-TRIM",
            &[bag, input]
        ),
        " .hello"
    );
    assert_eq!(
        call_text(
            &runtime,
            &mut ctx,
            "COMMON-LISP",
            "STRING",
            &[Word::character('x' as u32)]
        ),
        "x"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn unicode_builtins_cover_normalization_case_utf8_and_graphemes() {
    let (runtime, mut ctx) = runtime();
    let composed = string(&mut ctx, &runtime, "é");
    let decomposed = string(&mut ctx, &runtime, "e\u{301}");
    assert_eq!(
        call_text(
            &runtime,
            &mut ctx,
            "NCL-UNICODE",
            "NORMALIZE-NFC",
            &[decomposed]
        ),
        "é"
    );
    assert_eq!(
        call_text(
            &runtime,
            &mut ctx,
            "NCL-UNICODE",
            "NORMALIZE-NFD",
            &[composed]
        ),
        "e\u{301}"
    );
    let circled = string(&mut ctx, &runtime, "①");
    assert_eq!(
        call_text(
            &runtime,
            &mut ctx,
            "NCL-UNICODE",
            "NORMALIZE-NFKC",
            &[circled]
        ),
        "1"
    );
    assert_eq!(
        call_text(
            &runtime,
            &mut ctx,
            "NCL-UNICODE",
            "NORMALIZE-NFKD",
            &[circled]
        ),
        "1"
    );
    let sharp_s = string(&mut ctx, &runtime, "straße");
    let umlaut = string(&mut ctx, &runtime, "ÄBC");
    let title = string(&mut ctx, &runtime, "hello, WORLD");
    assert_eq!(
        call_text(&runtime, &mut ctx, "NCL-UNICODE", "FULL-UPCASE", &[sharp_s]),
        "STRASSE"
    );
    assert_eq!(
        call_text(
            &runtime,
            &mut ctx,
            "NCL-UNICODE",
            "FULL-DOWNCASE",
            &[umlaut]
        ),
        "äbc"
    );
    assert_eq!(
        call_text(
            &runtime,
            &mut ctx,
            "NCL-UNICODE",
            "FULL-TITLECASE",
            &[title]
        ),
        "Hello, World"
    );
    let lambda = string(&mut ctx, &runtime, "λ!");
    let utf8 = call(
        &runtime,
        &mut ctx,
        "NCL-UNICODE",
        "STRING-TO-UTF8",
        &[lambda],
    );
    assert_eq!(
        vector_values(&ctx, utf8),
        vec![Word::fixnum(206), Word::fixnum(187), Word::fixnum(33)]
    );
    let roundtrip = call(&runtime, &mut ctx, "NCL-UNICODE", "UTF8-TO-STRING", &[utf8]);
    assert_eq!(text(&ctx, roundtrip), "λ!");
    let graphemes = string(&mut ctx, &runtime, "A\u{301}🇯🇵");
    let boundaries = call(
        &runtime,
        &mut ctx,
        "NCL-UNICODE",
        "GRAPHEME-BOUNDARIES",
        &[graphemes],
    );
    assert_eq!(
        vector_values(&ctx, boundaries)
            .into_iter()
            .map(ncl_object::Word::as_fixnum)
            .collect::<Vec<_>>(),
        vec![Some(0), Some(2), Some(4)]
    );
    assert_eq!(
        call_text(
            &runtime,
            &mut ctx,
            "NCL-UNICODE",
            "GENERAL-CATEGORY",
            &[Word::character('A' as u32)]
        ),
        "Lu"
    );
}
