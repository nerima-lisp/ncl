use std::path::Path;

use crate::{Runtime, RuntimeError, compile};
use ncl_compiler_front::form::word_string;
use ncl_object::{ObjectRef, Word, car, cdr, classify_object, symbol_name};
use ncl_reader::{ReadOptions, StringSource, read};

pub fn file(runtime: &mut Runtime, path: &Path) -> Result<Word, RuntimeError> {
    let bytes = std::fs::read(path).map_err(|error| RuntimeError::Io {
        path: path.display().to_string(),
        error,
    })?;
    if compile::is_fasl(&bytes) {
        let fasl = ncl_objfile::FaslReader::read(&bytes, host_architecture(), 0)?;
        return runtime.load(&compile::decode_payload(&fasl.sections.debug)?);
    }
    let source = String::from_utf8(bytes)
        .map_err(|_| RuntimeError::Native("source file is not valid UTF-8".to_owned()))?;
    source_forms(runtime, &source)
}

fn source_forms(runtime: &mut Runtime, source: &str) -> Result<Word, RuntimeError> {
    let mut input = StringSource::new(source);
    let mut options = ReadOptions::standard(&mut runtime.context, &runtime.object)?;
    let mut result = Word::NIL;

    while let Some(form) = read(&mut runtime.context, &runtime.object, &mut input, &options)? {
        let package = in_package_name(&mut runtime.context, form)?;
        result = runtime.eval_form(form)?;
        if let Some(package) = package {
            options.set_current_package(package)?;
        }
    }
    Ok(result)
}

fn in_package_name(
    ctx: &mut ncl_object::ThreadContext,
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

const fn host_architecture() -> ncl_objfile::Architecture {
    if cfg!(target_arch = "x86_64") {
        ncl_objfile::Architecture::X86_64
    } else {
        ncl_objfile::Architecture::Aarch64
    }
}
