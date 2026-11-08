//! Public FORMAT output, control, and directive semantics.
#![expect(
    clippy::expect_used,
    reason = "Test setup uses expect to keep fixture failures local."
)]

use ncl_object::{
    FunctionObject, HeapRootedWord, ObjectError, Runtime, ThreadContext, Word,
    make_bignum_from_i128, make_string, string_length, string_ref,
};

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("register context");
    (runtime, ctx)
}

fn string(runtime: &Runtime, ctx: &mut ThreadContext, value: &str) -> Word {
    make_string(ctx, runtime, &value.chars().collect::<Vec<_>>()).expect("string")
}

fn read_string(ctx: &ThreadContext, value: Word) -> String {
    (0..string_length(ctx, value).expect("string length"))
        .map(|index| string_ref(ctx, value, index).expect("string character"))
        .collect()
}

fn format_function(runtime: &Runtime, ctx: &mut ThreadContext) -> FunctionObject {
    ncl_lib_format::register(runtime).expect("FORMAT registration");
    runtime
        .function(ctx, "COMMON-LISP", "FORMAT")
        .and_then(|word| FunctionObject::try_from(word).ok())
        .expect("FORMAT function")
}

fn list(runtime: &Runtime, ctx: &mut ThreadContext, values: &[Word]) -> Word {
    values.iter().rev().copied().fold(Word::NIL, |tail, value| {
        ncl_object::make_cons(ctx, runtime, value, tail).expect("cons")
    })
}

fn call_format(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    format: FunctionObject,
    destination: Word,
    control: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let control_word = string(runtime, ctx, control);
    let control_root = HeapRootedWord::new(ctx, control_word)?;
    let argument_roots = args
        .iter()
        .map(|argument| HeapRootedWord::new(ctx, *argument))
        .collect::<Result<Vec<_>, _>>()?;
    let mut arguments = Vec::with_capacity(args.len() + 2);
    arguments.extend([destination, control_root.get()]);
    arguments.extend(argument_roots.iter().map(HeapRootedWord::get));
    runtime.call_builtin(ctx, format, &arguments)
}

fn format_nil(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    format: FunctionObject,
    control: &str,
    args: &[Word],
) -> Result<String, ObjectError> {
    let result = call_format(runtime, ctx, format, Word::NIL, control, args)?;
    Ok(read_string(ctx, result))
}

#[test]
fn public_format_nil_destination_returns_formatted_text() {
    let (runtime, mut ctx) = context();
    let format = format_function(&runtime, &mut ctx);
    let control_word = string(&runtime, &mut ctx, "~A~%");
    let control = HeapRootedWord::new(&ctx, control_word).expect("root control");
    let value_word = string(&runtime, &mut ctx, "visible");
    let value = HeapRootedWord::new(&ctx, value_word).expect("root value");
    let result = runtime
        .call_builtin(&mut ctx, format, &[Word::NIL, control.get(), value.get()])
        .expect("FORMAT result");
    // CLHS 22.3: NIL destination returns a string; 22.3.4.1 and 22.3.1.2 specify ~A and ~%.
    assert_eq!(read_string(&ctx, result), "visible\n");
}

#[test]
fn public_format_moves_the_argument_cursor_with_star() {
    let (runtime, mut ctx) = context();
    let format = format_function(&runtime, &mut ctx);
    let skipped_word = string(&runtime, &mut ctx, "skip");
    let skipped = HeapRootedWord::new(&ctx, skipped_word).expect("root skipped value");
    let value_word = string(&runtime, &mut ctx, "kept");
    let value = HeapRootedWord::new(&ctx, value_word).expect("root value");

    // CLHS 22.3.7.1: ~* advances past the next argument before ~A consumes one.
    assert_eq!(
        format_nil(
            &runtime,
            &mut ctx,
            format,
            "~*~A",
            &[skipped.get(), value.get()],
        ),
        Ok("kept".to_owned())
    );
    // CLHS 22.3.7.1: the colon modifier moves backward, allowing the value to print twice.
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "~A~:*~A", &[value.get()],),
        Ok("keptkept".to_owned())
    );
    // CLHS 22.3.7.1: the at-sign form sets the argument position absolutely.
    assert_eq!(
        format_nil(
            &runtime,
            &mut ctx,
            format,
            "~1@*~A",
            &[skipped.get(), value.get()],
        ),
        Ok("kept".to_owned())
    );
}

#[test]
fn public_format_iterates_over_lists_and_sublists() {
    let (runtime, mut ctx) = context();
    let format = format_function(&runtime, &mut ctx);
    let skipped_word = string(&runtime, &mut ctx, "skip");
    let skipped = HeapRootedWord::new(&ctx, skipped_word).expect("root skipped value");
    let value_word = string(&runtime, &mut ctx, "kept");
    let value = HeapRootedWord::new(&ctx, value_word).expect("root value");
    let values = list(&runtime, &mut ctx, &[skipped.get(), value.get()]);
    let values = HeapRootedWord::new(&ctx, values).expect("root values");
    let row = list(&runtime, &mut ctx, &[value.get()]);
    let row = HeapRootedWord::new(&ctx, row).expect("root row");
    let rows = list(&runtime, &mut ctx, &[row.get()]);
    let rows = HeapRootedWord::new(&ctx, rows).expect("root rows");

    // CLHS 22.3.7.4/.5: an iteration limit restricts the number of list-body repetitions.
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "~1{~A~}", &[values.get()]),
        Ok("skip".to_owned())
    );
    // CLHS 22.3.7.4/.5: list iteration supplies each element to the body in order.
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "~{~A~}", &[values.get()]),
        Ok("skipkept".to_owned())
    );
    // CLHS 22.3.7.4/.5: at-sign iteration uses the remaining arguments as its list.
    assert_eq!(
        format_nil(
            &runtime,
            &mut ctx,
            format,
            "~@{~A~}",
            &[skipped.get(), value.get()],
        ),
        Ok("skipkept".to_owned())
    );
    // CLHS 22.3.7.4/.5: colon iteration treats each sublist as a body argument list.
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "~:{~A~}", &[rows.get()]),
        Ok("kept".to_owned())
    );
    // CLHS 22.3.7.4/.5: at-sign plus colon consumes each remaining argument as a sublist.
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "~@:{~A~}", &[row.get()]),
        Ok("kept".to_owned())
    );
}

#[test]
fn public_format_requires_each_iteration_body_argument() {
    let (runtime, mut ctx) = context();
    let format = format_function(&runtime, &mut ctx);
    // CLHS 22.3.7.4/.5: the body consumes an argument from each supplied sublist.
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "~@:{~A~}", &[Word::NIL]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn public_format_handles_out_of_range_conditional_indices() {
    let (runtime, mut ctx) = context();
    let format = format_function(&runtime, &mut ctx);
    // CLHS 22.3.7.2: the final colon separator selects its clause when no index matches.
    assert_eq!(
        format_nil(
            &runtime,
            &mut ctx,
            format,
            "~[zero~;one~;~:;many~]",
            &[Word::fixnum(9)],
        ),
        Ok("many".to_owned())
    );

    let too_negative = make_bignum_from_i128(&mut ctx, &runtime, i128::from(i64::MIN) + 1)
        .map(Into::into)
        .expect("negative bignum selector");
    let too_negative = HeapRootedWord::new(&ctx, too_negative).expect("root selector");
    // CLHS 22.3.7.2: an out-of-range index without a default selects no clause and signals no error.
    assert_eq!(
        format_nil(
            &runtime,
            &mut ctx,
            format,
            "~v[zero~;one~]",
            &[too_negative.get()],
        ),
        Ok(String::new())
    );
}

#[test]
fn public_format_applies_up_arrow_escape_conditions() {
    let (runtime, mut ctx) = context();
    let format = format_function(&runtime, &mut ctx);
    // CLHS 22.3.9.2: a nonzero V parameter does not terminate control-string processing.
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "~v^tail", &[Word::fixnum(1)],),
        Ok("tail".to_owned())
    );
    // CLHS 22.3.9.2: with no arguments remaining, unparameterized ~^ stops processing.
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "head~^tail", &[]),
        Ok("head".to_owned())
    );
    // CLHS 22.3.9.2: two equal parameters terminate processing.
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "~2,2^tail", &[]),
        Ok(String::new())
    );
    // CLHS 22.3.9.2: ordered three parameters satisfy the termination condition.
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "~1,1,2^tail", &[]),
        Ok(String::new())
    );
}

#[test]
fn public_format_emits_fresh_lines_and_line_controls() {
    let (runtime, mut ctx) = context();
    let format = format_function(&runtime, &mut ctx);
    // CLHS 22.3.1.3: ~& starts a new line only when output is not already at line start.
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "~&x", &[]),
        Ok("x".to_owned())
    );
    // CLHS 22.3.1.3: ~2& starts a line and emits one additional newline.
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "x~2&", &[]),
        Ok("x\n\n".to_owned())
    );
    // CLHS 22.3.1.2: ~2% emits two newline characters.
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "~2%", &[]),
        Ok("\n\n".to_owned())
    );
    // CLHS 22.3.1.4: ~2| emits two page characters.
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "~2|", &[]),
        Ok("\u{c}\u{c}".to_owned())
    );
    // CLHS 22.3.1.5: ~3~ emits three literal tilde characters.
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "~3~", &[]),
        Ok("~~~".to_owned())
    );
}

#[test]
fn public_format_ignores_whitespace_after_tilde_newline() {
    let (runtime, mut ctx) = context();
    let format = format_function(&runtime, &mut ctx);
    // CLHS 22.3.9.3: at-sign newline emits the newline and ignores following indentation.
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "a~@\n  b", &[]),
        Ok("a\nb".to_owned())
    );
    // CLHS 22.3.9.3: plain newline and following whitespace are ignored.
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "a~\n  b", &[]),
        Ok("ab".to_owned())
    );
}

#[test]
fn public_format_prints_characters_in_literal_and_reader_notation() {
    let (runtime, mut ctx) = context();
    let format = format_function(&runtime, &mut ctx);
    // CLHS 22.3.1.1: ~C prints characters literally, ~:C spells nonprinting characters,
    // and ~@C uses #\ reader syntax.
    assert_eq!(
        format_nil(
            &runtime,
            &mut ctx,
            format,
            "~C|~:C|~@C",
            &[
                Word::character(u32::from('x')),
                Word::character(u32::from('x')),
                Word::character(u32::from('\n')),
            ],
        ),
        Ok("x|x|#\\Newline".to_owned())
    );
}

#[test]
fn public_format_reuses_the_argument_to_choose_a_plural_suffix() {
    let (runtime, mut ctx) = context();
    let format = format_function(&runtime, &mut ctx);
    // CLHS 22.3.8.3: ~:P backs up to the preceding argument and omits the singular suffix.
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "~D item~:P", &[Word::fixnum(1)],),
        Ok("1 item".to_owned())
    );
    // CLHS 22.3.8.3: ~:P adds s when the preceding argument is not one.
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "~D item~:P", &[Word::fixnum(2)],),
        Ok("2 items".to_owned())
    );
}

#[test]
fn public_format_uses_remaining_arguments_as_a_prefix_parameter() {
    let (runtime, mut ctx) = context();
    let format = format_function(&runtime, &mut ctx);
    let value_word = string(&runtime, &mut ctx, "x");
    let value = HeapRootedWord::new(&ctx, value_word).expect("root value");
    // CLHS 22.3: # represents the number of arguments remaining at the directive.
    assert_eq!(
        format_nil(
            &runtime,
            &mut ctx,
            format,
            "~#A",
            &[value.get(), value.get()],
        ),
        Ok("x ".to_owned())
    );
}

#[test]
fn public_format_rejects_dynamic_and_character_parameters_of_the_wrong_type() {
    let (runtime, mut ctx) = context();
    let format = format_function(&runtime, &mut ctx);
    let wrong_parameter_word = string(&runtime, &mut ctx, "not an integer");
    let wrong_parameter =
        HeapRootedWord::new(&ctx, wrong_parameter_word).expect("root invalid parameter");
    let value_word = string(&runtime, &mut ctx, "x");
    let value = HeapRootedWord::new(&ctx, value_word).expect("root value");

    // CLHS 22.3 requires V parameters to supply values accepted by the directive.
    for (control, args) in [
        ("~v*~A", vec![wrong_parameter.get(), value.get()]),
        ("~vP", vec![wrong_parameter.get()]),
        ("~v%", vec![Word::character(u32::from('2'))]),
        ("~'xA", vec![value.get()]),
        ("~1,'xA", vec![value.get()]),
    ] {
        assert_eq!(
            format_nil(&runtime, &mut ctx, format, control, &args),
            Err(ObjectError::TypeError),
            "{control}"
        );
    }
}

#[test]
fn public_format_applies_aesthetic_field_width_and_padding() {
    let (runtime, mut ctx) = context();
    let format = format_function(&runtime, &mut ctx);
    let value_word = string(&runtime, &mut ctx, "x");
    let value = HeapRootedWord::new(&ctx, value_word).expect("root value");
    // CLHS 22.3.4.1: the colon modifier prints NIL as ().
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "~:A", &[Word::NIL]),
        Ok("()".to_owned())
    );
    // CLHS 22.3.4.1: A padding is added on the right to reach the minimum column width.
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "~5A", &[value.get()]),
        Ok("x    ".to_owned())
    );
    // CLHS 22.3.4.1: the at-sign modifier places minimum-column padding on the left.
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "~5@A", &[value.get()]),
        Ok("    x".to_owned())
    );
    // CLHS 22.3.4.1: minpad adds at least two copies of padchar.
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "~5,1,2,'-A", &[value.get()]),
        Ok("x----".to_owned())
    );
    // CLHS 22.3.4.1: colinc inserts padding in groups until the minimum width is reached.
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "~5,2,0,'-A", &[value.get()]),
        Ok("x----".to_owned())
    );
}

#[test]
fn public_format_recursively_processes_nested_string_controls() {
    let (runtime, mut ctx) = context();
    let format = format_function(&runtime, &mut ctx);
    let nested_control_word = string(&runtime, &mut ctx, "~A");
    let nested_control =
        HeapRootedWord::new(&ctx, nested_control_word).expect("root nested control");
    let value_word = string(&runtime, &mut ctx, "Some Text");
    let value = HeapRootedWord::new(&ctx, value_word).expect("root value");
    let unused_word = string(&runtime, &mut ctx, "unused");
    let unused = HeapRootedWord::new(&ctx, unused_word).expect("root unused nested value");
    let nested_arguments_word = list(&runtime, &mut ctx, &[value.get(), unused.get()]);
    let nested_arguments =
        HeapRootedWord::new(&ctx, nested_arguments_word).expect("root nested arguments");
    let outer_word = string(&runtime, &mut ctx, "outer");
    let outer = HeapRootedWord::new(&ctx, outer_word).expect("root outer value");
    // CLHS 22.3.7.6: ~? processes the nested list, then resumes the containing control string.
    assert_eq!(
        format_nil(
            &runtime,
            &mut ctx,
            format,
            "~?/~A",
            &[nested_control.get(), nested_arguments.get(), outer.get()],
        ),
        Ok("Some Text/outer".to_owned())
    );
    // CLHS 22.3.7.6: ~@? uses caller arguments and resumes after the nested control string.
    assert_eq!(
        format_nil(
            &runtime,
            &mut ctx,
            format,
            "~@?/~A",
            &[nested_control.get(), value.get(), outer.get()],
        ),
        Ok("Some Text/outer".to_owned())
    );
}

#[test]
fn public_format_converts_case_for_the_enclosed_output() {
    let (runtime, mut ctx) = context();
    let format = format_function(&runtime, &mut ctx);
    let text_word = string(&runtime, &mut ctx, "SOME TEXT");
    let text = HeapRootedWord::new(&ctx, text_word).expect("root text");
    // CLHS 22.3.8.1: ~(...) converts uppercase letters in the enclosed output to lowercase.
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "~(~A~)", &[text.get()]),
        Ok("some text".to_owned())
    );
    // CLHS 22.3.8.1: ~:(...) capitalizes the words in the enclosed output.
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "~:(~A~)", &[text.get()]),
        Ok("Some Text".to_owned())
    );
}
