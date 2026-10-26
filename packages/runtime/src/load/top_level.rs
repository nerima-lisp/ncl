use crate::evalwhen::{TopLevelMode, eval_when_start};
use crate::{Runtime, RuntimeError};
use ncl_compiler_front::form::word_string;
use ncl_object::{
    HeapRootedWord, ObjectRef, Readtable as ObjectReadtable, Runtime as ObjectRuntime, Word,
    classify_object, make_cons, symbol_name, symbol_package,
};
use ncl_reader::{ReadOptions, Readtable};

pub(super) fn eval_top_level(
    runtime: &mut Runtime,
    options: &mut ReadOptions,
    form: Word,
    mode: TopLevelMode,
) -> Result<Word, RuntimeError> {
    let rooted_form = HeapRootedWord::new(&runtime.context, form)?;
    let result = eval_top_level_inner(runtime, options, rooted_form.get(), mode);
    rooted_form.release().map_err(|_| {
        RuntimeError::Native("load: top-level form root stack corrupted".to_owned())
    })?;
    result
}

fn eval_top_level_inner(
    runtime: &mut Runtime,
    options: &mut ReadOptions,
    form: Word,
    mode: TopLevelMode,
) -> Result<Word, RuntimeError> {
    if !matches!(classify_object(&runtime.context, form), ObjectRef::Cons(_)) {
        return eval_rooted(runtime, options, form);
    }
    let elements = ncl_compiler_front::form::list(&mut runtime.context, form)?;
    let Some((head, arguments)) = elements.split_first() else {
        return eval_rooted(runtime, options, form);
    };
    let Some(name) = top_level_name(&runtime.context, &runtime.object, *head)? else {
        return eval_rooted(runtime, options, form);
    };
    let (body_start, eval_when_active) = if name == "PROGN" {
        (0, true)
    } else if name == "LOCALLY" {
        (
            leading_declarations(&mut runtime.context, &runtime.object, arguments),
            true,
        )
    } else if name == "EVAL-WHEN" {
        let (body_start, active) = eval_when_start(runtime, arguments, mode)?;
        (body_start, active)
    } else if name == "MACROLET" || name == "SYMBOL-MACROLET" {
        let Some(rest) = arguments.get(1..) else {
            return eval_rooted(runtime, options, form);
        };
        (
            1 + leading_declarations(&mut runtime.context, &runtime.object, rest),
            true,
        )
    } else {
        return eval_rooted(runtime, options, form);
    };
    if name == "EVAL-WHEN" && !eval_when_active {
        return Ok(Word::NIL);
    }
    if body_start >= arguments.len() {
        return eval_rooted(runtime, options, form);
    }
    let rooted_elements = elements
        .iter()
        .copied()
        .map(|element| HeapRootedWord::new(&runtime.context, element))
        .collect::<Result<Vec<_>, _>>()?;
    let result_root = HeapRootedWord::new(&runtime.context, Word::NIL)?;
    let evaluated = (|| {
        for body_index in 1 + body_start..rooted_elements.len() {
            let body = rooted_elements
                .get(body_index)
                .map(HeapRootedWord::get)
                .ok_or_else(|| {
                    RuntimeError::Native("load: top-level body is missing".to_owned())
                })?;
            let body_form = if name == "PROGN" || name == "EVAL-WHEN" {
                body
            } else {
                let prefix = rooted_elements
                    .get(..=body_start)
                    .ok_or_else(|| {
                        RuntimeError::Native("load: top-level prefix is missing".to_owned())
                    })?
                    .iter()
                    .map(HeapRootedWord::get)
                    .collect::<Vec<_>>();
                wrap_top_level_form(runtime, &prefix, body)?
            };
            let result = if name == "PROGN" || name == "EVAL-WHEN" {
                eval_top_level(runtime, options, body_form, mode)?
            } else {
                eval_rooted(runtime, options, body_form)?
            };
            result_root.set(result);
        }
        Ok(result_root.get())
    })();
    for root in rooted_elements.into_iter().rev() {
        root.release().map_err(|_| {
            RuntimeError::Native("load: top-level form root stack corrupted".to_owned())
        })?;
    }
    result_root.release().map_err(|_| {
        RuntimeError::Native("load: top-level form root stack corrupted".to_owned())
    })?;
    evaluated
}

fn top_level_name(
    ctx: &ncl_object::ThreadContext,
    runtime: &ObjectRuntime,
    word: Word,
) -> Result<Option<String>, RuntimeError> {
    if !matches!(classify_object(ctx, word), ObjectRef::Symbol(_)) {
        return Ok(None);
    }
    let Some(common_lisp) = runtime.find_package(ctx, "COMMON-LISP") else {
        return Ok(None);
    };
    if symbol_package(ctx, word)? != common_lisp {
        return Ok(None);
    }
    Ok(Some(word_string(ctx, symbol_name(ctx, word)?)?))
}

fn leading_declarations(
    ctx: &mut ncl_object::ThreadContext,
    runtime: &ObjectRuntime,
    forms: &[Word],
) -> usize {
    forms
        .iter()
        .take_while(|form| {
            let Ok(elements) = ncl_compiler_front::form::list(ctx, **form) else {
                return false;
            };
            elements
                .first()
                .and_then(|head| top_level_name(ctx, runtime, *head).ok().flatten())
                .is_some_and(|name| name == "DECLARE")
        })
        .count()
}

fn wrap_top_level_form(
    runtime: &mut Runtime,
    elements: &[Word],
    body: Word,
) -> Result<Word, RuntimeError> {
    let words = elements
        .iter()
        .copied()
        .chain(std::iter::once(body))
        .map(|word| HeapRootedWord::new(&runtime.context, word))
        .collect::<Result<Vec<_>, _>>()?;
    let wrapped = HeapRootedWord::new(&runtime.context, Word::NIL)?;
    for word in words.iter().rev() {
        wrapped.set(make_cons(
            &mut runtime.context,
            &runtime.object,
            word.get(),
            wrapped.get(),
        )?);
    }
    let result = wrapped.get();
    for word in words.into_iter().rev() {
        word.release().map_err(|_| {
            RuntimeError::Native("load: top-level form root stack corrupted".to_owned())
        })?;
    }
    wrapped.release().map_err(|_| {
        RuntimeError::Native("load: top-level form root stack corrupted".to_owned())
    })?;
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
/// [`push_heap_root`]/[`unregister_heap_root`] register roots in the runtime's
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
    let form_root = HeapRootedWord::new(&runtime.context, form)?;
    let readtable_root =
        HeapRootedWord::new(&runtime.context, options.readtable().object().as_word())?;

    let evaluated = runtime.eval_form(form_root.get());

    let readtable = readtable_root.get();
    readtable_root
        .release()
        .map_err(|_| RuntimeError::Native("load: form root stack corrupted".to_owned()))?;
    form_root
        .release()
        .map_err(|_| RuntimeError::Native("load: form root stack corrupted".to_owned()))?;
    options.set_readtable(Readtable::from_object(ObjectReadtable::from_word(
        readtable,
    )));
    evaluated
}
