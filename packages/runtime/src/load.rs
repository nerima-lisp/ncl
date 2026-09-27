use std::path::Path;

use crate::{Runtime, RuntimeError, compile};
use ncl_compiler_front::form::word_string;
use ncl_object::{
    ObjectRef, Readtable as ObjectReadtable, Word, car, cdr, classify_object, pop_heap_root,
    push_heap_root, symbol_name,
};
use ncl_reader::{ReadOptions, Readtable, StringSource, read};

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

    // Read one form, evaluate it, then read the next: package markers such as
    // `cl-bench:` only resolve once a preceding `defpackage`/`in-package` in
    // this same file has run, so the two steps must interleave rather than
    // reading every form up front.
    while let Some(form) = read(&mut runtime.context, &runtime.object, &mut input, &options)? {
        let package = in_package_name(&runtime.context, form)?;
        result = eval_rooted(runtime, &mut options, form)?;
        if let Some(package) = package {
            options.set_current_package(package)?;
        }
    }
    Ok(result)
}

/// Evaluate `form`, keeping both it and the reader's readtable rooted across
/// the call.
///
/// `Runtime::eval_form` compiles `form` before running it, and a successful
/// compilation *permanently* grows the per-thread root stack: each freshly
/// compiled function's code object and constants table are pushed via
/// [`ncl_object::push_root`] and intentionally never popped, so the compiled
/// code stays reachable for the runtime's lifetime (see
/// `Runtime::make_function_object`/`root_entry_code`). A caller cannot
/// therefore bracket this call with an ordinary `push_root`/`pop_root` pair
/// on that same stack: by the time `eval_form` returns, the stack is taller
/// than the caller's token expects, `pop_root` no longer sees its own push at
/// the top, and it reports corruption instead of silently misbehaving (this
/// is the "form root stack corrupted" failure the previous rooting attempt
/// hit). [`ncl_object::Scope`] cannot help either: its root slots are tied to
/// an exclusive borrow of one `ThreadContext`, and `eval_form` needs the
/// whole `Runtime` (which owns that context as a field) back before it can
/// run.
///
/// [`push_heap_root`]/[`pop_heap_root`] register roots in the runtime's
/// heap-level registry instead, a separate stack that ordinary compilation
/// does not touch, so bracketing `eval_form` with that pair stays balanced
/// regardless of how much the per-thread stack grows underneath it.
///
/// Both roots are read back from their (possibly forwarded) post-call value
/// before being released, and the readtable stored in `options` is refreshed
/// from that value: `eval_form` can allocate and collect, which would
/// otherwise leave the previously-cached readtable word pointing at a stale
/// address on the next `read` call.
fn eval_rooted(
    runtime: &mut Runtime,
    options: &mut ReadOptions,
    form: Word,
) -> Result<Word, RuntimeError> {
    let mut form_word = form;
    let form_token = push_heap_root(&runtime.object, &mut form_word);
    let mut readtable_word = options.readtable().object().as_word();
    let readtable_token = push_heap_root(&runtime.object, &mut readtable_word);

    let evaluated = runtime.eval_form(form_word);

    // Pop in the reverse of push order (LIFO): the readtable was pushed
    // last, so it must be popped first.
    let readtable_popped = pop_heap_root(&runtime.object, readtable_token);
    let form_popped = pop_heap_root(&runtime.object, form_token);
    if !readtable_popped || !form_popped {
        return Err(RuntimeError::Native(
            "load: form root stack corrupted".to_owned(),
        ));
    }
    options.set_readtable(Readtable::from_object(ObjectReadtable::from_word(
        readtable_word,
    )));
    evaluated
}

fn in_package_name(
    ctx: &ncl_object::ThreadContext,
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
