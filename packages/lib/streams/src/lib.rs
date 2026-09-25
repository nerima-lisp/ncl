//! ANSI file-stream builtins and the NCL-GRAY registration surface.

#![forbid(unsafe_code)]

use std::fs;

use ncl_object::{
    simple_vector_length, simple_vector_ref, simple_vector_set, stream_element_type,
    stream_external_format, stream_state, string_length, string_ref, symbol_name, Builtin,
    BuiltinImplementation, ObjectError, Runtime, Stream, ThreadContext, Word,
};

const POSITION: usize = 1;
const DATA: usize = 2;

/// Register the file-stream functions implemented by this crate.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    let open = BuiltinImplementation::adapted(
        Builtin {
            arity: 0,
            direct: false,
            lambda_list:
                "namestring &key direction element-type if-exists if-does-not-exist external-format",
        },
        open_builtin,
        pass_arguments,
    );
    let position = BuiltinImplementation::adapted(
        Builtin {
            arity: 0,
            direct: false,
            lambda_list: "stream &optional position",
        },
        file_position_builtin,
        pass_arguments,
    );
    for (name, implementation) in [("OPEN", open), ("FILE-POSITION", position)] {
        runtime.register_builtin(&mut ctx, "COMMON-LISP", name, implementation)?;
    }
    runtime.register_builtin(
        &mut ctx,
        "COMMON-LISP",
        "FILE-LENGTH",
        BuiltinImplementation::direct(
            Builtin {
                arity: 1,
                direct: true,
                lambda_list: "stream",
            },
            file_length_builtin,
        ),
    )?;
    runtime.register_builtin(
        &mut ctx,
        "COMMON-LISP",
        "FILE-STRING-LENGTH",
        BuiltinImplementation::direct(
            Builtin {
                arity: 2,
                direct: true,
                lambda_list: "stream string",
            },
            file_string_length_builtin,
        ),
    )?;
    Ok(())
}

fn pass_arguments(args: &[Word]) -> Result<Vec<Word>, ObjectError> {
    Ok(args.to_vec())
}

fn fail(ctx: &mut ThreadContext, error: ObjectError) -> Result<Word, ObjectError> {
    ctx.set_pending(error);
    Ok(Word::UNBOUND)
}

fn text(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    let len = string_length(ctx, word)?;
    (0..len).map(|index| string_ref(ctx, word, index)).collect()
}

fn name(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    text(ctx, symbol_name(ctx, word)?).map(|value| value.to_ascii_uppercase())
}

fn option(args: &[Word], ctx: &ThreadContext, keyword: &str) -> Result<Option<Word>, ObjectError> {
    let mut index = 1;
    while index + 1 < args.len() {
        if name(ctx, args[index])? == keyword {
            return Ok(Some(args[index + 1]));
        }
        index += 2;
    }
    if index != args.len() {
        return Err(ObjectError::TypeError);
    }
    Ok(None)
}

fn bytes_for_external_format(text: &str, format: &str) -> Result<Vec<u8>, ObjectError> {
    match format {
        "UTF-8" => Ok(text.as_bytes().to_vec()),
        "LATIN-1" => text
            .chars()
            .map(|character| u8::try_from(character as u32).map_err(|_| ObjectError::TypeError))
            .collect(),
        _ => Err(ObjectError::Unsupported),
    }
}

fn open_builtin(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    if args.is_empty() {
        return fail(ctx, ObjectError::TypeError);
    }
    let path = match text(ctx, args[0]) {
        Ok(path) => path,
        Err(error) => return fail(ctx, error),
    };
    let direction = match option(args, ctx, "DIRECTION")? {
        Some(word) => name(ctx, word)?,
        None => "INPUT".to_string(),
    };
    let external_format = match option(args, ctx, "EXTERNAL-FORMAT")? {
        Some(word) => name(ctx, word)?,
        None => "UTF-8".to_string(),
    };
    if !matches!(direction.as_str(), "INPUT" | "OUTPUT" | "IO") {
        return fail(ctx, ObjectError::TypeError);
    }
    if !matches!(external_format.as_str(), "UTF-8" | "LATIN-1") {
        return fail(ctx, ObjectError::Unsupported);
    }
    let if_exists = option(args, ctx, "IF-EXISTS")?
        .map(|word| name(ctx, word))
        .transpose()?;
    let if_missing = option(args, ctx, "IF-DOES-NOT-EXIST")?
        .map(|word| name(ctx, word))
        .transpose()?;
    let input = direction == "INPUT";
    let exists = fs::metadata(&path).is_ok();
    if input {
        if !exists && if_missing.as_deref() == Some("NIL") {
            return Ok(Word::NIL);
        }
        if !exists {
            return fail(ctx, ObjectError::Unsupported);
        }
    } else {
        if exists && if_exists.as_deref() == Some("ERROR") {
            return fail(ctx, ObjectError::Unsupported);
        }
        if !exists && if_missing.as_deref() == Some("ERROR") {
            return fail(ctx, ObjectError::Unsupported);
        }
        if !exists && if_missing.as_deref() == Some("NIL") {
            return Ok(Word::NIL);
        }
    }
    let _ = (path, input, exists, if_exists, if_missing);
    fail(ctx, ObjectError::Unsupported)
}

fn stream_data(ctx: &ThreadContext, stream: Word) -> Result<Word, ObjectError> {
    stream_state(ctx, Stream::from(stream))
}

fn file_position_builtin(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    if args.is_empty() || args.len() > 2 {
        return fail(ctx, ObjectError::TypeError);
    }
    let state = stream_data(ctx, args[0])?;
    if args.len() == 2 {
        let position = args[1].as_fixnum().ok_or(ObjectError::TypeError)?;
        let length = simple_vector_length(ctx, state)?.saturating_sub(DATA);
        if position < 0 || usize::try_from(position).map_err(|_| ObjectError::TypeError)? > length {
            return fail(ctx, ObjectError::TypeError);
        }
        simple_vector_set(ctx, state, POSITION, args[1])?;
    }
    simple_vector_ref(ctx, state, POSITION)
}

fn file_length_builtin(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = args.first().copied().ok_or(ObjectError::TypeError)?;
    let state = stream_data(ctx, stream)?;
    Ok(Word::fixnum(
        i64::try_from(simple_vector_length(ctx, state)?.saturating_sub(DATA))
            .map_err(|_| ObjectError::Layout)?,
    ))
}

fn file_string_length_builtin(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() != 2 {
        return fail(ctx, ObjectError::TypeError);
    }
    let format = stream_external_format(ctx, Stream::from(args[0]))?;
    let format = if format == Word::NIL {
        "UTF-8".to_string()
    } else {
        name(ctx, format)?
    };
    let bytes = bytes_for_external_format(&text(ctx, args[1])?, &format)?;
    Ok(Word::fixnum(
        i64::try_from(bytes.len()).map_err(|_| ObjectError::Layout)?,
    ))
}

#[allow(dead_code)]
fn _stream_accessors(ctx: &ThreadContext, stream: Stream) -> Result<(Word, Word), ObjectError> {
    Ok((
        stream_element_type(ctx, stream)?,
        stream_external_format(ctx, stream)?,
    ))
}
