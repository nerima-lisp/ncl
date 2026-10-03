#![allow(clippy::too_many_lines, clippy::unwrap_used, missing_docs)]

use std::collections::HashMap;

use ncl_object::{FunctionObject, ObjectError, Package, Runtime, ThreadContext, Word};

fn setup() -> (Runtime, ThreadContext, HashMap<String, FunctionObject>) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_macros::register(&runtime).unwrap();
    let names = [
        "DESTRUCTURING-BIND",
        "DOLIST",
        "DOTIMES",
        "LOOP",
        "RESTART-CASE",
        "WITH-INPUT-FROM-STRING",
        "WITH-OUTPUT-TO-STRING",
    ];
    let functions = names
        .into_iter()
        .map(|name| {
            let word = runtime.function(&mut ctx, "COMMON-LISP", name).unwrap();
            (name.to_owned(), FunctionObject::try_from(word).unwrap())
        })
        .collect();
    (runtime, ctx, functions)
}

fn symbol(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
    Package::from_word(runtime.find_package(ctx, "COMMON-LISP").unwrap())
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn keyword(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
    Package::from_word(runtime.find_package(ctx, "KEYWORD").unwrap())
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn list(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Word {
    ncl_lib_macros::list(ctx, runtime, values).unwrap()
}

fn elements(ctx: &ThreadContext, value: Word) -> Vec<Word> {
    let mut values = Vec::new();
    let mut cursor = value;
    while cursor != Word::NIL {
        values.push(ncl_object::car(ctx, cursor).unwrap());
        cursor = ncl_object::cdr(ctx, cursor).unwrap();
    }
    values
}

fn symbol_name(ctx: &ThreadContext, value: Word) -> Option<String> {
    let name = ncl_object::symbol_name(ctx, value).ok()?;
    (0..ncl_object::string_length(ctx, name).ok()?)
        .map(|index| ncl_object::string_ref(ctx, name, index).ok())
        .collect::<Option<String>>()
}

fn contains_symbol(ctx: &ThreadContext, value: Word, name: &str) -> bool {
    if symbol_name(ctx, value).as_deref() == Some(name) {
        return true;
    }
    elements_checked(ctx, value).is_ok_and(|items| {
        items
            .into_iter()
            .any(|item| contains_symbol(ctx, item, name))
    })
}

fn elements_checked(ctx: &ThreadContext, value: Word) -> Result<Vec<Word>, ObjectError> {
    let mut values = Vec::new();
    let mut cursor = value;
    while cursor != Word::NIL {
        values.push(ncl_object::car(ctx, cursor)?);
        cursor = ncl_object::cdr(ctx, cursor)?;
    }
    Ok(values)
}

fn call(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    functions: &HashMap<String, FunctionObject>,
    name: &str,
    form: Word,
) -> Result<Word, ObjectError> {
    let function = functions[name];
    ncl_object::with_roots(ctx, &[form], |ctx, roots| {
        runtime.call_builtin(
            ctx,
            function,
            &roots.iter().map(|root| **root).collect::<Vec<_>>(),
        )
    })
}

#[test]
fn loop_public_macro_covers_clause_families_and_errors() {
    let (runtime, mut ctx, functions) = setup();
    let loop_word = symbol(&mut ctx, &runtime, "LOOP");
    let named = symbol(&mut ctx, &runtime, "NAMED");
    let done = symbol(&mut ctx, &runtime, "DONE");
    let with = symbol(&mut ctx, &runtime, "WITH");
    let x = symbol(&mut ctx, &runtime, "X");
    let equals = symbol(&mut ctx, &runtime, "=");
    let initially = symbol(&mut ctx, &runtime, "INITIALLY");
    let finally = symbol(&mut ctx, &runtime, "FINALLY");
    let do_word = symbol(&mut ctx, &runtime, "DO");
    let for_word = symbol(&mut ctx, &runtime, "FOR");
    let from = symbol(&mut ctx, &runtime, "FROM");
    let by = symbol(&mut ctx, &runtime, "BY");
    let to = symbol(&mut ctx, &runtime, "TO");
    let when = symbol(&mut ctx, &runtime, "WHEN");
    let and = symbol(&mut ctx, &runtime, "AND");
    let collect = symbol(&mut ctx, &runtime, "COLLECT");
    let into = symbol(&mut ctx, &runtime, "INTO");
    let result = symbol(&mut ctx, &runtime, "RESULT");
    let else_word = symbol(&mut ctx, &runtime, "ELSE");
    let return_word = symbol(&mut ctx, &runtime, "RETURN");
    let end = symbol(&mut ctx, &runtime, "END");
    let form = list(
        &mut ctx,
        &runtime,
        &[
            loop_word,
            named,
            done,
            with,
            x,
            equals,
            Word::fixnum(1),
            initially,
            x,
            finally,
            x,
            for_word,
            x,
            from,
            Word::fixnum(0),
            by,
            Word::fixnum(2),
            to,
            Word::fixnum(10),
            when,
            x,
            do_word,
            x,
            and,
            collect,
            x,
            into,
            result,
            else_word,
            return_word,
            x,
            end,
        ],
    );
    let expansion = call(&runtime, &mut ctx, &functions, "LOOP", form).unwrap();
    let parts = elements(&ctx, expansion);
    assert_eq!(symbol_name(&ctx, parts[0]).as_deref(), Some("LET"));
    assert!(contains_symbol(&ctx, expansion, "BLOCK"));
    assert!(contains_symbol(&ctx, expansion, "TAGBODY"));

    let unknown = symbol(&mut ctx, &runtime, "UNKNOWN");
    let malformed_for = list(&mut ctx, &runtime, &[loop_word, for_word]);
    let malformed_when = list(&mut ctx, &runtime, &[loop_word, when, Word::fixnum(1), and]);
    let malformed_unknown = list(&mut ctx, &runtime, &[loop_word, unknown]);
    for malformed in [malformed_for, malformed_when, malformed_unknown] {
        assert_eq!(
            call(&runtime, &mut ctx, &functions, "LOOP", malformed),
            Err(ObjectError::TypeError)
        );
    }
}

#[test]
fn destructuring_public_macro_covers_lambda_list_markers_and_errors() {
    let (runtime, mut ctx, functions) = setup();
    let macro_word = symbol(&mut ctx, &runtime, "DESTRUCTURING-BIND");
    let a = symbol(&mut ctx, &runtime, "A");
    let whole = symbol(&mut ctx, &runtime, "WHOLE");
    let rest = symbol(&mut ctx, &runtime, "REST");
    let optional = symbol(&mut ctx, &runtime, "OPTIONAL");
    let supplied = symbol(&mut ctx, &runtime, "SUPPLIED-P");
    let key = symbol(&mut ctx, &runtime, "KEY");
    let whole_marker = symbol(&mut ctx, &runtime, "&WHOLE");
    let optional_marker = symbol(&mut ctx, &runtime, "&OPTIONAL");
    let rest_marker = symbol(&mut ctx, &runtime, "&REST");
    let key_marker = symbol(&mut ctx, &runtime, "&KEY");
    let allow_other_keys = symbol(&mut ctx, &runtime, "&ALLOW-OTHER-KEYS");
    let pattern = list(
        &mut ctx,
        &runtime,
        &[
            whole_marker,
            whole,
            a,
            optional_marker,
            optional,
            Word::fixnum(9),
            supplied,
            rest_marker,
            rest,
            key_marker,
            key,
            allow_other_keys,
        ],
    );
    let form = list(&mut ctx, &runtime, &[macro_word, pattern, a]);
    let expansion = call(&runtime, &mut ctx, &functions, "DESTRUCTURING-BIND", form).unwrap();
    assert_eq!(
        elements(&ctx, expansion)[0],
        symbol(&mut ctx, &runtime, "LET*")
    );

    let malformed_rest = list(&mut ctx, &runtime, &[macro_word, rest_marker]);
    let malformed_key = list(&mut ctx, &runtime, &[macro_word, key_marker]);
    for malformed in [malformed_rest, malformed_key] {
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                &functions,
                "DESTRUCTURING-BIND",
                malformed
            ),
            Err(ObjectError::TypeError)
        );
    }
}

#[test]
fn string_and_restart_public_macros_cover_options_and_errors() {
    let (runtime, mut ctx, functions) = setup();
    let input = symbol(&mut ctx, &runtime, "WITH-INPUT-FROM-STRING");
    let output = symbol(&mut ctx, &runtime, "WITH-OUTPUT-TO-STRING");
    let stream = symbol(&mut ctx, &runtime, "STREAM");
    let body = symbol(&mut ctx, &runtime, "BODY");
    let text = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b', 'c']).unwrap();
    let start = keyword(&mut ctx, &runtime, "START");
    let end = keyword(&mut ctx, &runtime, "END");
    let index = keyword(&mut ctx, &runtime, "INDEX");
    let input_spec = list(
        &mut ctx,
        &runtime,
        &[
            stream,
            text,
            start,
            Word::fixnum(1),
            end,
            Word::fixnum(2),
            index,
            stream,
        ],
    );
    let input_form = list(&mut ctx, &runtime, &[input, input_spec, body]);
    let input_expansion = call(
        &runtime,
        &mut ctx,
        &functions,
        "WITH-INPUT-FROM-STRING",
        input_form,
    )
    .unwrap();
    assert!(elements(&ctx, input_expansion).len() >= 3);

    let element_type = keyword(&mut ctx, &runtime, "ELEMENT-TYPE");
    let quote = symbol(&mut ctx, &runtime, "QUOTE");
    let character = symbol(&mut ctx, &runtime, "CHARACTER");
    let type_form = list(&mut ctx, &runtime, &[quote, character]);
    let output_spec = list(&mut ctx, &runtime, &[stream, element_type, type_form]);
    let output_form = list(&mut ctx, &runtime, &[output, output_spec, body]);
    let output_expansion = call(
        &runtime,
        &mut ctx,
        &functions,
        "WITH-OUTPUT-TO-STRING",
        output_form,
    )
    .unwrap();
    assert!(elements(&ctx, output_expansion).len() >= 3);

    let restart = symbol(&mut ctx, &runtime, "RESTART-CASE");
    let signal = symbol(&mut ctx, &runtime, "SIGNAL");
    let condition = symbol(&mut ctx, &runtime, "CONDITION");
    let clause = list(&mut ctx, &runtime, &[condition, body]);
    let signal_form = list(&mut ctx, &runtime, &[signal]);
    let restart_form = list(&mut ctx, &runtime, &[restart, signal_form, clause]);
    let restart_expansion =
        call(&runtime, &mut ctx, &functions, "RESTART-CASE", restart_form).unwrap();
    assert!(elements(&ctx, restart_expansion).len() >= 3);

    let unknown = keyword(&mut ctx, &runtime, "UNKNOWN");
    let bad_spec = list(&mut ctx, &runtime, &[stream, unknown, Word::TRUE]);
    let bad = list(&mut ctx, &runtime, &[input, bad_spec, body]);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "WITH-INPUT-FROM-STRING",
            bad
        ),
        Err(ObjectError::TypeError)
    );
}
