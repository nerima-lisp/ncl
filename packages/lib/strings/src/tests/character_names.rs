use super::{call, call_result, keyword, string_value};
use ncl_object::{Runtime, ThreadContext, Word};

#[test]
fn character_names_and_string_designators_cover_remaining_cases() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    crate::register(&runtime).unwrap_or_else(|error| panic!("register: {error:?}"));

    for (character, name) in [
        ('\0', "NULL"),
        ('\u{7}', "BELL"),
        ('\u{8}', "BACKSPACE"),
        ('\t', "TAB"),
        ('\n', "LINEFEED"),
        ('\u{c}', "PAGE"),
        ('\r', "RETURN"),
        ('\u{1b}', "ESCAPE"),
        ('\u{7f}', "RUBOUT"),
    ] {
        let name_word = call(
            &runtime,
            &mut ctx,
            "CHAR-NAME",
            &[Word::character(u32::from(character))],
        );
        assert_eq!(string_value(&ctx, name_word), name);
    }
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "CHAR-NAME",
            &[Word::character('A' as u32)]
        ),
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, "DIGIT-CHAR", &[Word::fixnum(5)]),
        Word::character('5' as u32)
    );
    let one_character = ncl_object::make_string(&mut ctx, &runtime, &['A'])
        .unwrap_or_else(|error| panic!("string: {error:?}"));
    assert_eq!(
        call(&runtime, &mut ctx, "STRINGP", &[one_character]),
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "STRINGP", &[Word::TRUE]),
        Word::NIL
    );
    let character = Word::character('A' as u32);
    assert_eq!(
        call(&runtime, &mut ctx, "CHARACTER", &[character]),
        character
    );
    assert_eq!(
        call(&runtime, &mut ctx, "CHARACTER", &[one_character]),
        character
    );
    let string_word = call(&runtime, &mut ctx, "STRING", &[character]);
    assert_eq!(string_value(&ctx, string_word), "A");
    let two_characters = ncl_object::make_string(&mut ctx, &runtime, &['A', 'B'])
        .unwrap_or_else(|error| panic!("string: {error:?}"));
    assert_eq!(
        call_result(&runtime, &mut ctx, "CHARACTER", &[two_characters]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        call_result(&runtime, &mut ctx, "STRING", &[Word::TRUE]),
        Err(ncl_object::ObjectError::TypeError)
    );
    let input = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b'])
        .unwrap_or_else(|error| panic!("string: {error:?}"));
    let start = keyword(&mut ctx, &runtime, "START");
    for name in ["NSTRING-UPCASE", "NSTRING-CAPITALIZE"] {
        assert_eq!(
            call_result(&runtime, &mut ctx, name, &[input, start, Word::fixnum(3)]),
            Err(ncl_object::ObjectError::TypeError)
        );
        assert_eq!(string_value(&ctx, input), "ab");
    }
}
