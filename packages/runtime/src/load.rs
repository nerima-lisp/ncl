use std::path::Path;

use crate::evalwhen::{TopLevelMode, eval_when_start};
use crate::{Runtime, RuntimeError, compile};
use ncl_compiler_front::form::word_string;
use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, FileError, FunctionObject, LambdaList, LispError, MultipleValues, ObjectError,
    ObjectRef, ObjectType, Package, Parameter, ParameterType, Readtable as ObjectReadtable,
    Runtime as ObjectRuntime, ThreadContext, Word, car, cdr, classify_object, make_cons,
    pop_heap_root, push_heap_root, set_symbol_value, symbol_name, symbol_package, symbol_value,
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

    fn compile_file(
        &self,
        ctx: &mut ThreadContext,
        object: &ObjectRuntime,
        args: &BuiltinArgs<'_>,
        values: &mut MultipleValues,
    ) -> Result<Word, ObjectError> {
        let Some(pointer) = ctx.evaluator_runtime() else {
            return Err(ObjectError::Layout);
        };
        ncl_sys::with_opaque_mut(pointer, |runtime: &mut Runtime| {
            let path_word = args.required(0)?;
            let path = path_from_word(ctx, object, path_word)?;
            let value =
                compile::file(runtime, Path::new(&path)).map_err(|_| ObjectError::TypeError)?;
            values.set(&[value]);
            Ok(value)
        })
    }
}

fn path_from_word(
    ctx: &mut ThreadContext,
    object: &ObjectRuntime,
    path_word: Word,
) -> Result<String, ObjectError> {
    if matches!(classify_object(ctx, path_word), ObjectRef::Structure(_)) {
        let function = object
            .function(ctx, "COMMON-LISP", "NAMESTRING")
            .ok_or(ObjectError::UndefinedFunction)?;
        let function = FunctionObject::try_from(function)?;
        let namestring = object.call_builtin(ctx, function, &[path_word])?;
        ncl_compiler_front::form::word_string(ctx, namestring).map_err(|_| ObjectError::TypeError)
    } else if matches!(classify_object(ctx, path_word), ObjectRef::String(_)) {
        ncl_compiler_front::form::word_string(ctx, path_word).map_err(|_| ObjectError::TypeError)
    } else {
        ctx.set_pending_lisp_error(LispError::TypeError {
            datum: path_word,
            expected: ObjectType::String,
        });
        Err(ObjectError::TypeError)
    }
}

fn load_with_runtime(
    ctx: &mut ThreadContext,
    object: &ObjectRuntime,
    runtime: &mut Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let path_word = args.required(0)?;
    let path = if matches!(classify_object(ctx, path_word), ObjectRef::Structure(_)) {
        let function = object
            .function(ctx, "COMMON-LISP", "NAMESTRING")
            .ok_or(ObjectError::UndefinedFunction)?;
        let function = FunctionObject::try_from(function)?;
        let namestring = object.call_builtin(ctx, function, &[path_word])?;
        ncl_compiler_front::form::word_string(ctx, namestring)
            .map_err(|_| ObjectError::TypeError)?
    } else if matches!(classify_object(ctx, path_word), ObjectRef::String(_)) {
        ncl_compiler_front::form::word_string(ctx, path_word).map_err(|_| ObjectError::TypeError)?
    } else {
        ctx.set_pending_lisp_error(LispError::TypeError {
            datum: path_word,
            expected: ObjectType::String,
        });
        return Err(ObjectError::TypeError);
    };
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
    match file_with_load_pathnames(ctx, object, runtime, path_word, Path::new(&path)) {
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
            ctx.set_pending_lisp_error(LispError::FileError(FileError::NotFound {
                pathname: path_word,
            }));
            Err(ObjectError::TypeError)
        }
        Err(_error) => {
            ctx.set_pending_lisp_error(LispError::FileError(FileError::InvalidPath {
                pathname: path_word,
            }));
            Err(ObjectError::TypeError)
        }
    }
}

fn file_with_load_pathnames(
    ctx: &mut ThreadContext,
    object: &ObjectRuntime,
    runtime: &mut Runtime,
    path_word: Word,
    path: &Path,
) -> Result<Word, RuntimeError> {
    let pathname_function = object
        .function(ctx, "COMMON-LISP", "PATHNAME")
        .ok_or(ObjectError::UndefinedFunction)?;
    let pathname_function = FunctionObject::try_from(pathname_function)?;
    let pathname = object.call_builtin(ctx, pathname_function, &[path_word])?;
    let common_lisp = object
        .find_package(ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    let package = Package::from_word(common_lisp);
    let load_pathname = package.intern(ctx, object, "*LOAD-PATHNAME*")?.0;
    let load_truename = package.intern(ctx, object, "*LOAD-TRUENAME*")?.0;
    let previous = [
        symbol_value(ctx, load_pathname)?,
        symbol_value(ctx, load_truename)?,
    ];
    set_symbol_value(ctx, load_pathname, pathname)?;
    set_symbol_value(ctx, load_truename, pathname)?;
    let result = file(runtime, path);
    set_symbol_value(ctx, load_pathname, previous[0])?;
    set_symbol_value(ctx, load_truename, previous[1])?;
    result
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
    ty: ParameterType::Any,
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
    object.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("COMPILE-FILE")),
        BuiltinImplementation::adapted(
            Builtin {
                lambda_list: LambdaList::fixed(LOAD_PATH),
                convention: BuiltinConvention::Adapted,
            },
            compile_file_builtin,
            validate_compile_file_arguments,
        )
        .with_nested_evaluation(),
    )?;
    let common_lisp = object
        .find_package(ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    let compile_file_truename = Package::from_word(common_lisp)
        .intern(ctx, object, "*COMPILE-FILE-TRUENAME*")?
        .0;
    ncl_object::set_symbol_special(ctx, compile_file_truename, true)?;
    if symbol_value(ctx, compile_file_truename)? == Word::UNBOUND {
        set_symbol_value(ctx, compile_file_truename, Word::NIL)?;
    }
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

fn compile_file_builtin(
    ctx: &mut ThreadContext,
    object: &ObjectRuntime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    object.compile_file_port(ctx, args, values)
}

fn validate_load_arguments(args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    if args.is_empty() || args.len().is_multiple_of(2) {
        return Err(ObjectError::TypeError);
    }
    Ok(args.as_slice().to_vec())
}

fn validate_compile_file_arguments(args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    if args.len() != 1 {
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
    let mut result = Word::NIL;
    let result_token = push_heap_root(&runtime.object, &mut result);

    // Read one form, evaluate it, then read the next: package markers such as
    // `cl-bench:` only resolve once a preceding `defpackage`/`in-package` in
    // this same file has run, so the two steps must interleave rather than
    // reading every form up front.
    let loaded = (|| {
        while let Some(form) = read(&mut runtime.context, &runtime.object, &mut input, &options)? {
            let package = in_package_name(&runtime.context, form)?;
            result = match eval_top_level(runtime, &mut options, form, mode) {
                Ok(result) => result,
                Err(error) => return Err(error),
            };
            if let Some(package) = package {
                options.set_current_package(package)?;
            }
        }
        Ok(result)
    })();
    if !pop_heap_root(&runtime.object, result_token) {
        return Err(RuntimeError::Native(
            "load: result root stack corrupted".to_owned(),
        ));
    }
    loaded
}

fn eval_top_level(
    runtime: &mut Runtime,
    options: &mut ReadOptions,
    form: Word,
    mode: TopLevelMode,
) -> Result<Word, RuntimeError> {
    let mut rooted_form = form;
    let token = push_heap_root(&runtime.object, &mut rooted_form);
    let result = eval_top_level_inner(runtime, options, rooted_form, mode);
    if !pop_heap_root(&runtime.object, token) {
        return Err(RuntimeError::Native(
            "load: top-level form root stack corrupted".to_owned(),
        ));
    }
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
    let mut elements = ncl_compiler_front::form::list(&mut runtime.context, form)?;
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
    let mut element_tokens = Vec::with_capacity(elements.len());
    for element in &mut elements {
        element_tokens.push(push_heap_root(&runtime.object, element));
    }
    let mut result = Word::NIL;
    let evaluated = (|| {
        let body_forms = elements
            .get(1 + body_start..)
            .ok_or_else(|| RuntimeError::Native("load: top-level body is missing".to_owned()))?;
        for body in body_forms {
            let body_form = if name == "PROGN" || name == "EVAL-WHEN" {
                *body
            } else {
                let prefix = elements.get(..=body_start).ok_or_else(|| {
                    RuntimeError::Native("load: top-level prefix is missing".to_owned())
                })?;
                wrap_top_level_form(runtime, prefix, *body)?
            };
            result = if name == "PROGN" || name == "EVAL-WHEN" {
                eval_top_level(runtime, options, body_form, mode)?
            } else {
                eval_rooted(runtime, options, body_form)?
            };
        }
        Ok(result)
    })();
    for token in element_tokens.into_iter().rev() {
        if !pop_heap_root(&runtime.object, token) {
            return Err(RuntimeError::Native(
                "load: top-level form root stack corrupted".to_owned(),
            ));
        }
    }
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
    let mut words = elements.to_vec();
    words.push(body);
    let mut tokens = Vec::with_capacity(words.len());
    for word in &mut words {
        tokens.push(push_heap_root(&runtime.object, word));
    }
    let mut wrapped = Word::NIL;
    let wrapped_token = push_heap_root(&runtime.object, &mut wrapped);
    for word in words.iter().rev() {
        wrapped = make_cons(&mut runtime.context, &runtime.object, *word, wrapped)?;
    }
    let result = wrapped;
    let wrapped_popped = pop_heap_root(&runtime.object, wrapped_token);
    for token in tokens.into_iter().rev() {
        if !pop_heap_root(&runtime.object, token) {
            return Err(RuntimeError::Native(
                "load: top-level wrapper root stack corrupted".to_owned(),
            ));
        }
    }
    if !wrapped_popped {
        return Err(RuntimeError::Native(
            "load: top-level wrapper root stack corrupted".to_owned(),
        ));
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
