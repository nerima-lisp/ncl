#![allow(missing_docs)]

use ncl_object::{FunctionObject, ObjectError, Runtime, ThreadContext, Word};

use ncl_lib_macros::{elements, list, register, symbol};

fn destructuring_bind_expansion(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    form: Word,
) -> Result<Word, ObjectError> {
    let function = runtime
        .function(ctx, "COMMON-LISP", "DESTRUCTURING-BIND")
        .ok_or(ObjectError::TypeError)?;
    runtime.call_builtin(ctx, FunctionObject::try_from(function)?, &[form])
}

#[test]
fn key_expansion_recurses_past_a_nonmatching_keyword() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    let key_marker = symbol(&mut ctx, &runtime, "&KEY")?;
    let wanted = symbol(&mut ctx, &runtime, "WANTED")?;
    let keyword_other = symbol(&mut ctx, &runtime, "KEYWORD::OTHER")?;
    let keyword_wanted = symbol(&mut ctx, &runtime, "KEYWORD::WANTED")?;
    let pattern = list(&mut ctx, &runtime, &[key_marker, wanted])?;
    let plist = list(
        &mut ctx,
        &runtime,
        &[
            keyword_other,
            Word::fixnum(1),
            keyword_wanted,
            Word::fixnum(7),
        ],
    )?;
    let quote = symbol(&mut ctx, &runtime, "QUOTE")?;
    let quoted_plist = list(&mut ctx, &runtime, &[quote, plist])?;
    let operator = symbol(&mut ctx, &runtime, "DESTRUCTURING-BIND")?;
    let form = list(
        &mut ctx,
        &runtime,
        &[operator, pattern, quoted_plist, wanted],
    )?;

    let expansion = destructuring_bind_expansion(&mut ctx, &runtime, form)?;
    let expanded_parts = elements(&mut ctx, expansion)?;
    assert_eq!(expanded_parts[0], symbol(&mut ctx, &runtime, "LET*")?);
    let bindings = elements(&mut ctx, expanded_parts[1])?;
    assert_eq!(bindings.len(), 3);

    let source_binding = elements(&mut ctx, bindings[0])?;
    assert_eq!(source_binding[1], quoted_plist);

    let found_binding = elements(&mut ctx, bindings[1])?;
    let found_symbol = found_binding[0];
    let labels = elements(&mut ctx, found_binding[1])?;
    assert_eq!(labels[0], symbol(&mut ctx, &runtime, "LABELS")?);
    let definitions = elements(&mut ctx, labels[1])?;
    let definition = elements(&mut ctx, definitions[0])?;
    let parameters = elements(&mut ctx, definition[1])?;
    let parameter = parameters[0];
    let clauses = elements(&mut ctx, definition[2])?;
    assert_eq!(clauses[0], symbol(&mut ctx, &runtime, "COND")?);

    let match_clause = elements(&mut ctx, clauses[2])?;
    let keyword_test = elements(&mut ctx, match_clause[0])?;
    assert_eq!(keyword_test[0], symbol(&mut ctx, &runtime, "EQ")?);
    let first_key = elements(&mut ctx, keyword_test[1])?;
    assert_eq!(
        first_key,
        vec![symbol(&mut ctx, &runtime, "CAR")?, parameter]
    );
    let quoted_keyword = elements(&mut ctx, keyword_test[2])?;
    assert_eq!(quoted_keyword[0], quote);
    assert_eq!(quoted_keyword[1], keyword_wanted);
    assert_eq!(match_clause[1], parameter);

    let recurse_clause = elements(&mut ctx, clauses[3])?;
    assert_eq!(recurse_clause[0], symbol(&mut ctx, &runtime, "T")?);
    let recurse_call = elements(&mut ctx, recurse_clause[1])?;
    assert_eq!(recurse_call[0], definition[0]);
    let second_pair_tail = elements(&mut ctx, recurse_call[1])?;
    assert_eq!(second_pair_tail[0], symbol(&mut ctx, &runtime, "CDR")?);
    let first_pair_tail = elements(&mut ctx, second_pair_tail[1])?;
    assert_eq!(
        first_pair_tail,
        vec![symbol(&mut ctx, &runtime, "CDR")?, parameter]
    );

    let value_binding = elements(&mut ctx, bindings[2])?;
    assert_eq!(value_binding[0], wanted);
    let selected_value = elements(&mut ctx, value_binding[1])?;
    assert_eq!(selected_value[0], symbol(&mut ctx, &runtime, "IF")?);
    assert_eq!(selected_value[1], found_symbol);
    let value_from_match = elements(&mut ctx, selected_value[2])?;
    assert_eq!(
        value_from_match,
        vec![symbol(&mut ctx, &runtime, "CADR")?, found_symbol]
    );
    assert_eq!(selected_value[3], Word::NIL);
    Ok(())
}
