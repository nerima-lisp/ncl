use std::path::Path;

use crate::evalwhen::{TopLevelMode, eval_when_start};
use crate::{Runtime, RuntimeError, compile, file_environment};
use ncl_compiler_front::form::word_string;
use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, FileError, HeapRootedWord, LambdaList, LispError, MultipleValues, ObjectError,
    ObjectRef, Parameter, ParameterType, Readtable as ObjectReadtable, Runtime as ObjectRuntime,
    ThreadContext, Word, car, cdr, classify_object, make_cons, make_string, symbol_name,
    symbol_package,
};
use ncl_reader::{ReadOptions, Readtable, StringSource, read};

pub fn file(runtime: &mut Runtime, path: &Path) -> Result<Word, RuntimeError> {
    let pathname = file_environment::absolute_path(path)?;
    let truename = std::fs::canonicalize(&pathname).map_err(|error| RuntimeError::Io {
        path: pathname.display().to_string(),
        error,
    })?;
    file_environment::enter_load(runtime, &pathname, &truename)?;
    let result = file_contents(runtime, &pathname);
    let restored = runtime.context.leave_progv().map_err(RuntimeError::from);
    match (result, restored) {
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        (Ok(value), Ok(())) => Ok(value),
    }
}

fn file_contents(runtime: &mut Runtime, path: &Path) -> Result<Word, RuntimeError> {
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

#[derive(Debug)]
pub struct RuntimeLoadPort;

impl ncl_object::LoadPort for RuntimeLoadPort {
    fn load(
        &self,
        ctx: &mut ThreadContext,
        object: &ObjectRuntime,
        args: &BuiltinArgs<'_>,
        values: &mut MultipleValues,
    ) -> Result<Word, ObjectError> {
        let Some(pointer) = ctx.evaluator_runtime() else {
            return Err(ObjectError::Layout);
        };
        // The pointer is installed only around an active Runtime evaluation and
        // is cleared before that Runtime can be moved or dropped.
        ncl_sys::with_opaque_mut(pointer, |runtime: &mut Runtime| {
            load_with_runtime(ctx, object, runtime, args, values)
        })
    }
}

fn load_with_runtime(
    ctx: &mut ThreadContext,
    object: &ObjectRuntime,
    runtime: &mut Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let path = ncl_lib_pathnames::resolve_pathname(ctx, object, args.required(0)?)?;
    let mut if_missing = true;
    for index in (1..args.len()).step_by(2) {
        let keyword = args.get(index).ok_or(ObjectError::Layout)?;
        let value = args.get(index + 1).ok_or(ObjectError::Layout)?;
        let Some(name) = keyword_name(ctx, object, keyword)? else {
            ctx.set_pending_lisp_error(LispError::ProgramError(
                ncl_object::ProgramError::UnknownKeyword,
            ));
            return Err(ObjectError::TypeError);
        };
        match name.as_str() {
            "VERBOSE" | "PRINT" => {}
            "IF-DOES-NOT-EXIST" => if_missing = value != Word::NIL,
            "EXTERNAL-FORMAT" => {
                if keyword_name(ctx, object, value)?.as_deref() != Some("DEFAULT") {
                    return Err(ObjectError::TypeError);
                }
            }
            _unknown_keyword => {
                ctx.set_pending_lisp_error(LispError::ProgramError(
                    ncl_object::ProgramError::UnknownKeyword,
                ));
                return Err(ObjectError::TypeError);
            }
        }
    }
    match file(runtime, &path) {
        Ok(value) => {
            values.set(&[value]);
            Ok(value)
        }
        Err(RuntimeError::Io { error, .. })
            if error.kind() == std::io::ErrorKind::NotFound && !if_missing =>
        {
            values.set(&[Word::NIL]);
            Ok(Word::NIL)
        }
        Err(RuntimeError::Io { error, .. }) if error.kind() == std::io::ErrorKind::NotFound => {
            ctx.set_pending_lisp_error(LispError::FileError(FileError::NotFound));
            Err(ObjectError::TypeError)
        }
        Err(RuntimeError::Native(message)) => {
            signal_native_load_error(ctx, object, &message)?;
            Err(ObjectError::TypeError)
        }
        Err(
            RuntimeError::Io { .. }
            | RuntimeError::Object(_)
            | RuntimeError::Read(_)
            | RuntimeError::Front(_)
            | RuntimeError::Lower(_)
            | RuntimeError::UndefinedFunction { .. }
            | RuntimeError::NativeFailure { .. },
        ) => {
            ctx.set_pending_lisp_error(LispError::FileError(FileError::InvalidPath));
            Err(ObjectError::TypeError)
        }
    }
}

fn signal_native_load_error(
    ctx: &mut ThreadContext,
    object: &ObjectRuntime,
    message: &str,
) -> Result<(), ObjectError> {
    let format_control = make_string(ctx, object, &['~', 'A'])?;
    let format_control = HeapRootedWord::new(ctx, format_control)?;
    let message = make_string(ctx, object, &message.chars().collect::<Vec<_>>())?;
    let message = HeapRootedWord::new(ctx, message)?;
    let format_arguments = make_cons(ctx, object, message.get(), Word::NIL)?;
    let format_arguments = HeapRootedWord::new(ctx, format_arguments)?;
    ctx.set_pending_lisp_error(LispError::SimpleError {
        format_control: format_control.get(),
        format_arguments: format_arguments.get(),
    });
    Ok(())
}

pub fn keyword_name(
    ctx: &ThreadContext,
    runtime: &ObjectRuntime,
    word: Word,
) -> Result<Option<String>, ObjectError> {
    let ObjectRef::Symbol(_) = classify_object(ctx, word) else {
        return Ok(None);
    };
    let Some(keyword) = runtime.find_package(ctx, "KEYWORD") else {
        return Ok(None);
    };
    if symbol_package(ctx, word)? != keyword {
        return Ok(None);
    }
    Ok(Some(
        word_string(ctx, symbol_name(ctx, word)?).map_err(|_| ObjectError::TypeError)?,
    ))
}

const LOAD_PATH: &[Parameter] = &[Parameter {
    name: BuiltinName::new("PATHNAME"),
    ty: ParameterType::StringDesignator,
}];
const LOAD_REST: Parameter = Parameter {
    name: BuiltinName::new("OPTIONS"),
    ty: ParameterType::Any,
};

pub fn register_builtin(
    ctx: &mut ThreadContext,
    object: &ObjectRuntime,
) -> Result<(), ObjectError> {
    object.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("LOAD")),
        BuiltinImplementation::adapted(
            Builtin {
                lambda_list: LambdaList::with_rest(LOAD_PATH, LOAD_REST),
                convention: BuiltinConvention::Adapted,
            },
            load_builtin,
            validate_load_arguments,
        )
        .with_nested_evaluation(),
    )?;
    Ok(())
}

fn load_builtin(
    ctx: &mut ThreadContext,
    object: &ObjectRuntime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    object.load_port(ctx, args, values)
}

fn validate_load_arguments(args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    if args.is_empty() || args.len().is_multiple_of(2) {
        return Err(ObjectError::TypeError);
    }
    Ok(args.as_slice().to_vec())
}

pub fn source_forms(runtime: &mut Runtime, source: &str) -> Result<Word, RuntimeError> {
    source_forms_with_mode(runtime, source, TopLevelMode::Execute)
}

pub fn source_forms_with_mode(
    runtime: &mut Runtime,
    source: &str,
    mode: TopLevelMode,
) -> Result<Word, RuntimeError> {
    let mut input = StringSource::new(source);
    let mut options = ReadOptions::standard(&mut runtime.context, &runtime.object)?;
    let result_root = HeapRootedWord::new(&runtime.context, Word::NIL)?;

    // Read one form, evaluate it, then read the next: package markers such as
    // `cl-bench:` only resolve once a preceding `defpackage`/`in-package` in
    // this same file has run, so the two steps must interleave rather than
    // reading every form up front.
    let loaded = (|| {
        while let Some(form) = read(&mut runtime.context, &runtime.object, &mut input, &options)? {
            let package = in_package_name(&runtime.context, form)?;
            result_root.set(eval_top_level(runtime, &mut options, form, mode)?);
            if let Some(package) = package {
                options.set_current_package(package)?;
            }
        }
        Ok(result_root.get())
    })();
    result_root
        .release()
        .map_err(|_| RuntimeError::Native("load: result root stack corrupted".to_owned()))?;
    loaded
}

fn eval_top_level(
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

#[cfg(test)]
mod tests {
    use super::signal_native_load_error;
    use crate::Runtime;
    use ncl_object::LispError;

    #[test]
    fn native_load_failure_preserves_its_message_in_the_condition_report() {
        let mut runtime =
            Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
        let message = "IR verification before optimization failed: function lambda-FunctionId(1): \
             [UndefinedValue(ValueId(4)), SuccessorType(BlockId(2))]";
        let (ctx, object) = (&mut runtime.context, &runtime.object);

        signal_native_load_error(ctx, object, message).expect("native load condition");
        let error = ctx.take_pending_lisp_error().expect("pending Lisp error");
        assert!(matches!(error, LispError::SimpleError { .. }));
        let converter = object.lisp_error_converter().expect("Lisp error converter");
        let condition = converter(ctx, object, error).expect("converted condition");

        assert_eq!(
            ncl_conditions::condition_report(ctx, condition).as_deref(),
            Some(message)
        );
    }
}
