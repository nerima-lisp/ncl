use super::*;
use ncl_object::MultipleValues;

fn expand(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    input: Word,
) -> std::result::Result<Word, ObjectError> {
    let callback = callback_for(name).ok_or(ObjectError::UndefinedFunction)?;
    let input_args = [input];
    let args = BuiltinArgs::new(&input_args);
    callback(ctx, runtime, &args, &mut MultipleValues::new())
}

#[test]
fn with_open_file_expansion_builds_normal_and_abort_close_forms()
-> std::result::Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    let macro_name = symbol(&mut ctx, &runtime, "WITH-OPEN-FILE")?;
    let variable = symbol(&mut ctx, &runtime, "STREAM")?;
    let filespec_operator = symbol(&mut ctx, &runtime, "FILESPEC-ONCE")?;
    let filespec_call = list(&mut ctx, &runtime, &[filespec_operator])?;
    let option_operator = symbol(&mut ctx, &runtime, "OPTION-ONCE")?;
    let option_call = list(&mut ctx, &runtime, &[option_operator])?;
    let keyword_package = runtime.ensure_package(&mut ctx, "KEYWORD")?;
    let if_missing = ncl_object::Package::from_word(keyword_package)
        .intern(&mut ctx, &runtime, "IF-DOES-NOT-EXIST")?
        .0;
    let special = symbol(&mut ctx, &runtime, "SPECIAL")?;
    let special_declaration = list(&mut ctx, &runtime, &[special, variable])?;
    let declare = symbol(&mut ctx, &runtime, "DECLARE")?;
    let declaration = list(&mut ctx, &runtime, &[declare, special_declaration])?;
    let first = symbol(&mut ctx, &runtime, "KEYWORD::FIRST")?;
    let second = symbol(&mut ctx, &runtime, "KEYWORD::SECOND")?;
    let values = symbol(&mut ctx, &runtime, "VALUES")?;
    let body = list(&mut ctx, &runtime, &[values, first, second])?;
    let spec = list(
        &mut ctx,
        &runtime,
        &[variable, filespec_call, if_missing, option_call],
    )?;
    let form = list(&mut ctx, &runtime, &[macro_name, spec, declaration, body])?;

    let expansion = expand(&mut ctx, &runtime, "WITH-OPEN-FILE", form)?;
    let outer = elements(&mut ctx, expansion)?;
    assert_eq!(outer[0], symbol(&mut ctx, &runtime, "LET*")?);
    let outer_bindings = elements(&mut ctx, outer[1])?;
    assert_eq!(outer_bindings.len(), 2);
    let stream_binding = elements(&mut ctx, outer_bindings[0])?;
    let open_call = elements(&mut ctx, stream_binding[1])?;
    assert_eq!(open_call[0], symbol(&mut ctx, &runtime, "OPEN")?);
    assert_eq!(open_call[1], filespec_call);
    assert_eq!(open_call[2], if_missing);
    assert_eq!(open_call[3], option_call);
    let abort_binding = elements(&mut ctx, outer_bindings[1])?;
    assert_eq!(abort_binding[1], symbol(&mut ctx, &runtime, "T")?);

    let inner = elements(&mut ctx, outer[2])?;
    assert_eq!(inner[0], symbol(&mut ctx, &runtime, "LET")?);
    assert_eq!(inner[2], declaration);
    let unwind = elements(&mut ctx, *inner.last().ok_or(ObjectError::Layout)?)?;
    assert_eq!(unwind[0], symbol(&mut ctx, &runtime, "UNWIND-PROTECT")?);
    let protected = elements(&mut ctx, unwind[1])?;
    assert_eq!(
        protected[0],
        symbol(&mut ctx, &runtime, "MULTIPLE-VALUE-PROG1")?
    );
    assert_eq!(
        elements(&mut ctx, protected[1])?[0],
        symbol(&mut ctx, &runtime, "PROGN")?
    );
    let mark_normal_exit = elements(&mut ctx, protected[2])?;
    assert_eq!(mark_normal_exit[0], symbol(&mut ctx, &runtime, "SETQ")?);
    assert_eq!(mark_normal_exit[1], abort_binding[0]);
    assert_eq!(mark_normal_exit[2], Word::NIL);

    let close_if_open = elements(&mut ctx, unwind[2])?;
    assert_eq!(close_if_open[0], symbol(&mut ctx, &runtime, "IF")?);
    assert_eq!(close_if_open[1], stream_binding[0]);
    let close_abort = elements(&mut ctx, close_if_open[2])?;
    assert_eq!(close_abort[0], symbol(&mut ctx, &runtime, "CLOSE")?);
    assert_eq!(
        close_abort[2],
        symbol(&mut ctx, &runtime, "KEYWORD::ABORT")?
    );
    assert_eq!(close_abort[3], abort_binding[0]);
    Ok(())
}

#[test]
fn with_open_file_rejects_malformed_specification() -> std::result::Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    let macro_name = symbol(&mut ctx, &runtime, "WITH-OPEN-FILE")?;
    let variable = symbol(&mut ctx, &runtime, "STREAM")?;
    let spec = list(&mut ctx, &runtime, &[variable, Word::NIL, Word::TRUE])?;
    let form = list(&mut ctx, &runtime, &[macro_name, spec])?;

    assert_eq!(
        expand(&mut ctx, &runtime, "WITH-OPEN-FILE", form),
        Err(ObjectError::TypeError)
    );
    Ok(())
}
