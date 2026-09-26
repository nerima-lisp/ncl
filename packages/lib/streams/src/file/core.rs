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
