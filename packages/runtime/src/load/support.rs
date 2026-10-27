use crate::{Runtime, RuntimeError};
use ncl_compiler_front::form::word_string;
use ncl_object::{
    FunctionObject, HeapRootedWord, ObjectError, ObjectRef, Runtime as ObjectRuntime,
    ThreadContext, Word, car, cdr, classify_object, make_string, symbol_name, with_root,
};

pub(super) fn print_loaded_value(runtime: &mut Runtime, value: Word) -> Result<(), RuntimeError> {
    let object = &runtime.object;
    let context = &mut runtime.context;
    let value = HeapRootedWord::new(context, value)?;
    let printed = call_output_builtin(context, object, "PRIN1", &[value.get()])
        .and_then(|_| call_output_builtin(context, object, "TERPRI", &[]));
    let released = value
        .release()
        .map_err(|_| RuntimeError::Native("load: printed value root stack corrupted".to_owned()));
    match (printed, released) {
        (Err(error), _) => Err(RuntimeError::from(error)),
        (Ok(_), Err(error)) => Err(error),
        (Ok(_), Ok(())) => Ok(()),
    }
}

pub(super) fn write_standard_output(runtime: &mut Runtime, text: &str) -> Result<(), RuntimeError> {
    let object = &runtime.object;
    let context = &mut runtime.context;
    let mut text = make_string(context, object, &text.chars().collect::<Vec<_>>())?;
    with_root(context, &mut text, |context, text| {
        call_output_builtin(context, object, "WRITE-STRING", &[*text]).map(|_| ())
    })?;
    Ok(())
}

pub(super) fn call_output_builtin(
    ctx: &mut ThreadContext,
    object: &ObjectRuntime,
    name: &str,
    arguments: &[Word],
) -> Result<Word, ObjectError> {
    let function = object
        .function(ctx, "COMMON-LISP", name)
        .ok_or(ObjectError::UndefinedFunction)?;
    object.call_builtin(ctx, FunctionObject::try_from(function)?, arguments)
}

pub(super) fn in_package_name(
    ctx: &ThreadContext,
    form: Word,
) -> Result<Option<String>, RuntimeError> {
    if !matches!(classify_object(ctx, form), ObjectRef::Cons(_)) {
        return Ok(None);
    }
    let head = car(ctx, form)?;
    let ObjectRef::Symbol(_) = classify_object(ctx, head) else {
        return Ok(None);
    };
    let head_name = symbol_name(ctx, head)?;
    if head == Word::NIL || !word_string(ctx, head_name)?.eq_ignore_ascii_case("IN-PACKAGE") {
        return Ok(None);
    }
    let arguments = cdr(ctx, form)?;
    let argument = car(ctx, arguments)?;
    let package = if matches!(classify_object(ctx, argument), ObjectRef::Symbol(_)) {
        word_string(ctx, symbol_name(ctx, argument)?)?
    } else if matches!(classify_object(ctx, argument), ObjectRef::String(_)) {
        word_string(ctx, argument)?
    } else {
        return Ok(None);
    };
    Ok(Some(package))
}

pub(super) const fn host_architecture() -> ncl_objfile::Architecture {
    if cfg!(target_arch = "x86_64") {
        ncl_objfile::Architecture::X86_64
    } else {
        ncl_objfile::Architecture::Aarch64
    }
}
