//! Public registration tests for character, Unicode, and range edge cases.

use ncl_object::{FunctionObject, Package, Runtime, ThreadContext, Word};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut context = ThreadContext::new();
    context
        .register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    ncl_lib_strings::register(&runtime).unwrap_or_else(|error| panic!("strings: {error:?}"));
    (runtime, context)
}

fn call(
    runtime: &Runtime,
    context: &mut ThreadContext,
    package: &str,
    name: &str,
    args: &[Word],
) -> Word {
    let function = runtime
        .function(context, package, name)
        .and_then(|word| FunctionObject::try_from(word).ok())
        .unwrap_or_else(|| panic!("missing builtin {package}::{name}"));
    runtime
        .call_builtin(context, function, args)
        .unwrap_or_else(|error| panic!("{package}::{name}: {error:?}"))
}

fn call_result(
    runtime: &Runtime,
    context: &mut ThreadContext,
    package: &str,
    name: &str,
    args: &[Word],
) -> Result<Word, ncl_object::ObjectError> {
    let function = runtime
        .function(context, package, name)
        .and_then(|word| FunctionObject::try_from(word).ok())
        .unwrap_or_else(|| panic!("missing builtin {package}::{name}"));
    runtime.call_builtin(context, function, args)
}

fn string(runtime: &Runtime, context: &mut ThreadContext, value: &str) -> Word {
    ncl_object::make_string(context, runtime, &value.chars().collect::<Vec<_>>())
        .unwrap_or_else(|error| panic!("string {value:?}: {error:?}"))
}

fn value(context: &ThreadContext, word: Word) -> String {
    (0..ncl_object::string_length(context, word)
        .unwrap_or_else(|error| panic!("string length: {error:?}")))
        .map(|index| {
            ncl_object::string_ref(context, word, index)
                .unwrap_or_else(|error| panic!("string ref: {error:?}"))
        })
        .collect()
}

fn keyword(runtime: &Runtime, context: &mut ThreadContext, name: &str) -> Word {
    let package = runtime
        .find_package(context, "KEYWORD")
        .unwrap_or_else(|| panic!("KEYWORD package missing"));
    Package::from_word(package)
        .intern(context, runtime, name)
        .unwrap_or_else(|error| panic!("keyword {name}: {error:?}"))
        .0
}

#[test]
fn named_characters_and_optional_radices_are_observable_through_registration() {
    let (runtime, mut context) = setup();
    for (name, expected) in [
        ("NULL", '\0'),
        ("BELL", '\u{7}'),
        ("BACKSPACE", '\u{8}'),
        ("TAB", '\t'),
        ("LINEFEED", '\n'),
        ("PAGE", '\u{c}'),
        ("RETURN", '\r'),
        ("ESCAPE", '\u{1b}'),
        ("SPACE", ' '),
        ("RUBOUT", '\u{7f}'),
        ("NUL", '\0'),
        ("NEWLINE", '\n'),
        ("DELETE", '\u{7f}'),
    ] {
        let input = string(&runtime, &mut context, name);
        assert_eq!(
            call(&runtime, &mut context, "COMMON-LISP", "NAME-CHAR", &[input],),
            Word::character(expected as u32),
            "NAME-CHAR {name}"
        );
    }
    for (character, expected) in [
        ('\0', "NULL"),
        ('\u{7}', "BELL"),
        ('\u{8}', "BACKSPACE"),
        ('\t', "TAB"),
        ('\n', "LINEFEED"),
        ('\u{c}', "PAGE"),
        ('\r', "RETURN"),
        ('\u{1b}', "ESCAPE"),
        (' ', "SPACE"),
        ('\u{7f}', "RUBOUT"),
    ] {
        let result = call(
            &runtime,
            &mut context,
            "COMMON-LISP",
            "CHAR-NAME",
            &[Word::character(character as u32)],
        );
        assert_eq!(value(&context, result), expected);
    }
    assert_eq!(
        call(
            &runtime,
            &mut context,
            "COMMON-LISP",
            "DIGIT-CHAR",
            &[Word::fixnum(35), Word::fixnum(36)]
        ),
        Word::character('Z' as u32)
    );
    assert_eq!(
        call(
            &runtime,
            &mut context,
            "COMMON-LISP",
            "DIGIT-CHAR-P",
            &[Word::character('z' as u32), Word::fixnum(36)]
        ),
        Word::fixnum(35)
    );
    for (name, args) in [
        ("DIGIT-CHAR", vec![Word::fixnum(1), Word::fixnum(37)]),
        (
            "DIGIT-CHAR-P",
            vec![Word::character('1' as u32), Word::fixnum(1)],
        ),
    ] {
        let error = match call_result(&runtime, &mut context, "COMMON-LISP", name, &args) {
            Ok(word) => panic!("expected {name} to fail, got {word:?}"),
            Err(error) => error,
        };
        assert_eq!(format!("{error:?}"), "TypeError", "{name} error kind");
    }
}

#[test]
fn unicode_normalization_and_grapheme_boundaries_cover_hangul_marks_and_regions() {
    let (runtime, mut context) = setup();
    let hangul = string(&runtime, &mut context, "각");
    let hangul_nfd = call(
        &runtime,
        &mut context,
        "NCL-UNICODE",
        "NORMALIZE-NFD",
        &[hangul],
    );
    assert_eq!(value(&context, hangul_nfd), "\u{1100}\u{1161}\u{11A8}");
    let compatibility = string(&runtime, &mut context, "㍑");
    let compatibility_nfkd = call(
        &runtime,
        &mut context,
        "NCL-UNICODE",
        "NORMALIZE-NFKD",
        &[compatibility],
    );
    assert_eq!(value(&context, compatibility_nfkd), "リットル");
    let marked = string(&runtime, &mut context, "a\u{301}\u{3099}");
    let marked_nfd = call(
        &runtime,
        &mut context,
        "NCL-UNICODE",
        "NORMALIZE-NFD",
        &[marked],
    );
    assert_eq!(value(&context, marked_nfd), "a\u{3099}\u{301}");
    let regions = string(&runtime, &mut context, "🇯🇵🇦");
    let boundaries = call(
        &runtime,
        &mut context,
        "NCL-UNICODE",
        "GRAPHEME-BOUNDARIES",
        &[regions],
    );
    let actual: Vec<_> = (0..ncl_object::simple_vector_length(&context, boundaries)
        .unwrap_or_else(|error| panic!("boundary length: {error:?}")))
        .map(|index| {
            ncl_object::simple_vector_ref(&context, boundaries, index)
                .unwrap_or_else(|error| panic!("boundary ref: {error:?}"))
                .as_fixnum()
                .unwrap_or_else(|| panic!("boundary value is not fixnum"))
        })
        .collect();
    assert_eq!(actual, vec![0, 2, 3]);
}

#[test]
fn public_api_reports_type_errors_for_invalid_vectors_and_ranges() {
    let (runtime, mut context) = setup();
    let invalid_type = ncl_object::make_simple_vector(&mut context, &runtime, &[Word::TRUE])
        .unwrap_or_else(|error| panic!("vector: {error:?}"));
    let error = match call_result(
        &runtime,
        &mut context,
        "NCL-UNICODE",
        "UTF8-TO-STRING",
        &[invalid_type],
    ) {
        Ok(word) => panic!("expected UTF8-TO-STRING to fail, got {word:?}"),
        Err(error) => error,
    };
    assert_eq!(format!("{error:?}"), "TypeError");

    let source = string(&runtime, &mut context, "abc");
    let start = keyword(&runtime, &mut context, "START");
    let end = keyword(&runtime, &mut context, "END");
    let error = match call_result(
        &runtime,
        &mut context,
        "COMMON-LISP",
        "STRING-UPCASE",
        &[source, start, Word::fixnum(2), end, Word::fixnum(1)],
    ) {
        Ok(word) => panic!("expected STRING-UPCASE to fail, got {word:?}"),
        Err(error) => error,
    };
    assert_eq!(format!("{error:?}"), "TypeError");
}
