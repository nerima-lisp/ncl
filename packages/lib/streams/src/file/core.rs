use super::StreamKind;
use ncl_object::{
    BuiltinArgs, ObjectError, Runtime, ThreadContext, Word, make_simple_vector, make_stream,
    string_length, string_ref, symbol_name, with_roots,
};

pub fn text(ctx: &ThreadContext, value: Word) -> Result<String, ObjectError> {
    (0..string_length(ctx, value)?)
        .map(|index| string_ref(ctx, value, index))
        .collect()
}

pub fn symbol_text(ctx: &ThreadContext, value: Word) -> Result<String, ObjectError> {
    text(ctx, symbol_name(ctx, value)?)
}

pub fn option(
    ctx: &ThreadContext,
    args: &BuiltinArgs<'_>,
    keyword: &str,
) -> Result<Option<Word>, ObjectError> {
    let mut index = 1;
    while index < args.len() {
        let key = args.get(index).ok_or(ObjectError::TypeError)?;
        let value = args.get(index + 1).ok_or(ObjectError::TypeError)?;
        if symbol_text(ctx, key)?.eq_ignore_ascii_case(keyword) {
            return Ok(Some(value));
        }
        index += 2;
    }
    Ok(None)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExistsPolicy {
    Error,
    Nil,
    Append,
    Overwrite,
    Supersede,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissingPolicy {
    Error,
    Nil,
    Create,
}

pub fn exists_policy(
    ctx: &ThreadContext,
    args: &BuiltinArgs<'_>,
) -> Result<ExistsPolicy, ObjectError> {
    let value = match option(ctx, args, "IF-EXISTS")? {
        Some(value) => symbol_text(ctx, value)?,
        None => return Ok(ExistsPolicy::Supersede),
    };
    if value.eq_ignore_ascii_case("ERROR") {
        Ok(ExistsPolicy::Error)
    } else if value.eq_ignore_ascii_case("NIL") {
        Ok(ExistsPolicy::Nil)
    } else if value.eq_ignore_ascii_case("APPEND") {
        Ok(ExistsPolicy::Append)
    } else if value.eq_ignore_ascii_case("OVERWRITE") {
        Ok(ExistsPolicy::Overwrite)
    } else if value.eq_ignore_ascii_case("SUPERSEDE")
        || value.eq_ignore_ascii_case("NEW-VERSION")
        || value.eq_ignore_ascii_case("RENAME")
    {
        Ok(ExistsPolicy::Supersede)
    } else {
        Err(ObjectError::TypeError)
    }
}

pub fn missing_policy(
    ctx: &ThreadContext,
    args: &BuiltinArgs<'_>,
    default: MissingPolicy,
) -> Result<MissingPolicy, ObjectError> {
    let value = match option(ctx, args, "IF-DOES-NOT-EXIST")? {
        Some(value) => symbol_text(ctx, value)?,
        None => return Ok(default),
    };
    if value.eq_ignore_ascii_case("ERROR") {
        Ok(MissingPolicy::Error)
    } else if value.eq_ignore_ascii_case("NIL") {
        Ok(MissingPolicy::Nil)
    } else if value.eq_ignore_ascii_case("CREATE") {
        Ok(MissingPolicy::Create)
    } else {
        Err(ObjectError::TypeError)
    }
}

pub fn direction_word(
    ctx: &ThreadContext,
    args: &BuiltinArgs<'_>,
) -> Result<(Word, String), ObjectError> {
    let word = option(ctx, args, "DIRECTION")?.unwrap_or(Word::NIL);
    let direction = if word == Word::NIL {
        "INPUT".to_owned()
    } else {
        symbol_text(ctx, word)?
    };
    Ok((word, direction))
}

pub fn format_word(ctx: &ThreadContext, args: &BuiltinArgs<'_>) -> Result<Word, ObjectError> {
    option(ctx, args, "EXTERNAL-FORMAT")?.map_or(Ok(Word::NIL), Ok)
}

#[derive(Clone, Copy)]
pub struct FileStreamSpec {
    pub path: Word,
    pub direction: Word,
    pub format: Word,
    pub kind: StreamKind,
    pub position: usize,
    pub implementation: Word,
}

pub fn make_file_stream(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    spec: FileStreamSpec,
) -> Result<Word, ObjectError> {
    let FileStreamSpec {
        path,
        direction,
        format,
        kind,
        position,
        implementation,
    } = spec;
    with_roots(
        ctx,
        &[path, direction, format, implementation],
        |ctx, roots| {
            let path = roots.first().ok_or(ObjectError::Layout)?;
            let direction = roots.get(1).ok_or(ObjectError::Layout)?;
            let format = roots.get(2).ok_or(ObjectError::Layout)?;
            let implementation = roots.get(3).ok_or(ObjectError::Layout)?;
            let state = make_simple_vector(
                ctx,
                runtime,
                &[
                    Word::fixnum(kind.code()),
                    Word::fixnum(i64::try_from(position).map_err(|_| ObjectError::Layout)?),
                    **path,
                ],
            )?;
            Ok(make_stream(
                ctx,
                runtime,
                **direction,
                Word::NIL,
                **format,
                state,
                **implementation,
            )?
            .into())
        },
    )
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::items_after_test_module,
    clippy::too_many_lines
)]
mod tests {
    use super::*;
    use ncl_object::{Package, make_string};

    fn symbol(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
        let package = runtime
            .ensure_package(ctx, "KEYWORD")
            .expect("keyword package");
        Package::from_word(package)
            .intern(ctx, runtime, name)
            .expect("keyword symbol")
            .0
    }

    #[test]
    fn options_and_policies_accept_supported_values_and_reject_invalid_values() {
        let runtime = Runtime::new().expect("runtime");
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("register");
        let if_exists = symbol(&mut ctx, &runtime, "IF-EXISTS");
        let if_missing = symbol(&mut ctx, &runtime, "IF-DOES-NOT-EXIST");
        let direction = symbol(&mut ctx, &runtime, "DIRECTION");
        let external = symbol(&mut ctx, &runtime, "EXTERNAL-FORMAT");
        let values = [
            symbol(&mut ctx, &runtime, "ERROR"),
            symbol(&mut ctx, &runtime, "NIL"),
            symbol(&mut ctx, &runtime, "APPEND"),
            symbol(&mut ctx, &runtime, "OVERWRITE"),
            symbol(&mut ctx, &runtime, "SUPERSEDE"),
        ];
        assert_eq!(
            exists_policy(&ctx, &BuiltinArgs::new(&[Word::NIL, if_exists, values[0]])),
            Ok(ExistsPolicy::Error)
        );
        assert_eq!(
            exists_policy(&ctx, &BuiltinArgs::new(&[Word::NIL, if_exists, values[1]])),
            Ok(ExistsPolicy::Nil)
        );
        assert_eq!(
            exists_policy(&ctx, &BuiltinArgs::new(&[Word::NIL, if_exists, values[2]])),
            Ok(ExistsPolicy::Append)
        );
        assert_eq!(
            exists_policy(&ctx, &BuiltinArgs::new(&[Word::NIL, if_exists, values[3]])),
            Ok(ExistsPolicy::Overwrite)
        );
        assert_eq!(
            exists_policy(&ctx, &BuiltinArgs::new(&[Word::NIL, if_exists, values[4]])),
            Ok(ExistsPolicy::Supersede)
        );
        assert_eq!(
            exists_policy(&ctx, &BuiltinArgs::new(&[])),
            Ok(ExistsPolicy::Supersede)
        );
        let invalid = symbol(&mut ctx, &runtime, "UNKNOWN");
        assert_eq!(
            exists_policy(&ctx, &BuiltinArgs::new(&[Word::NIL, if_exists, invalid])),
            Err(ObjectError::TypeError)
        );
        assert_eq!(
            missing_policy(&ctx, &BuiltinArgs::new(&[]), MissingPolicy::Create),
            Ok(MissingPolicy::Create)
        );
        assert_eq!(
            missing_policy(
                &ctx,
                &BuiltinArgs::new(&[Word::NIL, if_missing, values[0]]),
                MissingPolicy::Nil
            ),
            Ok(MissingPolicy::Error)
        );
        assert_eq!(
            missing_policy(
                &ctx,
                &BuiltinArgs::new(&[Word::NIL, if_missing, values[1]]),
                MissingPolicy::Error
            ),
            Ok(MissingPolicy::Nil)
        );
        let create = symbol(&mut ctx, &runtime, "CREATE");
        assert_eq!(
            missing_policy(
                &ctx,
                &BuiltinArgs::new(&[Word::NIL, if_missing, create]),
                MissingPolicy::Error
            ),
            Ok(MissingPolicy::Create)
        );
        assert_eq!(
            missing_policy(
                &ctx,
                &BuiltinArgs::new(&[Word::NIL, if_missing, invalid]),
                MissingPolicy::Error
            ),
            Err(ObjectError::TypeError)
        );
        let direction_args = [Word::NIL, direction, symbol(&mut ctx, &runtime, "IO")];
        assert_eq!(
            direction_word(&ctx, &BuiltinArgs::new(&direction_args))
                .expect("direction")
                .1,
            "IO"
        );
        assert_eq!(
            direction_word(&ctx, &BuiltinArgs::new(&[]))
                .expect("default")
                .1,
            "INPUT"
        );
        let format_args = [Word::NIL, external, Word::fixnum(7)];
        assert_eq!(
            format_word(&ctx, &BuiltinArgs::new(&format_args)),
            Ok(Word::fixnum(7))
        );
        assert_eq!(format_word(&ctx, &BuiltinArgs::new(&[])), Ok(Word::NIL));
        let text_value = make_string(&mut ctx, &runtime, &['a', 'b']).expect("string");
        assert_eq!(text(&ctx, text_value).expect("text"), "ab");
    }
}
