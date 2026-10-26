use std::path::Path;

use crate::evalwhen::TopLevelMode;
use crate::{Runtime, RuntimeError, compile, file_environment};
use ncl_compiler_front::form::word_string;
use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, FileError, FunctionObject, HeapRootedWord, LambdaList, LispError,
    MultipleValues, ObjectError, ObjectRef, Package, Parameter, ParameterType,
    Readtable as ObjectReadtable, Runtime as ObjectRuntime, ThreadContext, Word, car, cdr,
    classify_object, make_string, symbol_name, symbol_package, symbol_value, with_root,
};
use ncl_reader::{ReadOptions, Readtable, StringSource, read};

mod top_level;

use top_level::eval_top_level;

pub fn file(runtime: &mut Runtime, path: &Path) -> Result<Word, RuntimeError> {
    file_with_options(runtime, path, FileLoadOptions::default()).map_err(|error| match error {
        FileLoadError::Runtime(error) => error,
        FileLoadError::UnsupportedExternalFormat => {
            RuntimeError::Native("unsupported source external format".to_owned())
        }
    })
}

#[derive(Clone, Copy, Debug)]
struct FileLoadOptions {
    print_results: bool,
    verbose: bool,
    external_format_supported: bool,
}

impl Default for FileLoadOptions {
    fn default() -> Self {
        Self {
            print_results: false,
            verbose: false,
            external_format_supported: true,
        }
    }
}

#[derive(Debug)]
enum FileLoadError {
    Runtime(RuntimeError),
    UnsupportedExternalFormat,
}

impl From<RuntimeError> for FileLoadError {
    fn from(error: RuntimeError) -> Self {
        Self::Runtime(error)
    }
}

impl From<ncl_objfile::ObjectError> for FileLoadError {
    fn from(error: ncl_objfile::ObjectError) -> Self {
        Self::Runtime(RuntimeError::from(error))
    }
}

fn file_with_options(
    runtime: &mut Runtime,
    path: &Path,
    options: FileLoadOptions,
) -> Result<Word, FileLoadError> {
    let pathname = file_environment::absolute_path(path)?;
    let truename = std::fs::canonicalize(&pathname).map_err(|error| RuntimeError::Io {
        path: pathname.display().to_string(),
        error,
    })?;
    file_environment::enter_load(runtime, &pathname, &truename)?;
    let result = (|| -> Result<Word, FileLoadError> {
        if options.verbose {
            write_standard_output(
                runtime,
                &format!("; Loading contents of file {}\n", pathname.display()),
            )?;
        }
        let value = file_contents(runtime, &pathname, options)?;
        if options.verbose {
            write_standard_output(
                runtime,
                &format!("; Finished loading {}\n", pathname.display()),
            )?;
        }
        Ok(value)
    })();
    let restored = runtime.context.leave_progv().map_err(RuntimeError::from);
    match (result, restored) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error.into()),
        (Ok(value), Ok(())) => Ok(value),
    }
}

fn file_contents(
    runtime: &mut Runtime,
    path: &Path,
    options: FileLoadOptions,
) -> Result<Word, FileLoadError> {
    let bytes = std::fs::read(path).map_err(|error| RuntimeError::Io {
        path: path.display().to_string(),
        error,
    })?;
    if compile::is_fasl(&bytes) {
        let fasl = ncl_objfile::FaslReader::read(&bytes, host_architecture(), 0)?;
        let source = compile::decode_payload(&fasl.sections.debug)?;
        return source_forms_with_print(
            runtime,
            &source,
            TopLevelMode::Execute,
            options.print_results,
            true,
        )
        .map_err(FileLoadError::from);
    }
    if !options.external_format_supported {
        return Err(FileLoadError::UnsupportedExternalFormat);
    }
    let source = String::from_utf8(bytes)
        .map_err(|_| RuntimeError::Native("source file is not valid UTF-8".to_owned()))?;
    source_forms_with_print(
        runtime,
        &source,
        TopLevelMode::Execute,
        options.print_results,
        true,
    )
    .map_err(FileLoadError::from)
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
    let mut print = None;
    let mut verbose = None;
    let mut external_format_supported = true;
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
            "VERBOSE" => verbose = Some(value != Word::NIL),
            "PRINT" => print = Some(value != Word::NIL),
            "IF-DOES-NOT-EXIST" => if_missing = value != Word::NIL,
            "EXTERNAL-FORMAT" => {
                external_format_supported =
                    keyword_name(ctx, object, value)?.as_deref() == Some("DEFAULT");
            }
            _unknown_keyword => {
                ctx.set_pending_lisp_error(LispError::ProgramError(
                    ncl_object::ProgramError::UnknownKeyword,
                ));
                return Err(ObjectError::TypeError);
            }
        }
    }
    let print_results =
        print.unwrap_or(common_lisp_variable_value(ctx, object, "*LOAD-PRINT*")? != Word::NIL);
    let verbose =
        verbose.unwrap_or(common_lisp_variable_value(ctx, object, "*LOAD-VERBOSE*")? != Word::NIL);
    match file_with_options(
        runtime,
        &path,
        FileLoadOptions {
            print_results,
            verbose,
            external_format_supported,
        },
    ) {
        Ok(_) => {
            values.set(&[Word::TRUE]);
            Ok(Word::TRUE)
        }
        Err(FileLoadError::UnsupportedExternalFormat) => Err(ObjectError::TypeError),
        Err(FileLoadError::Runtime(RuntimeError::Io { error, .. }))
            if error.kind() == std::io::ErrorKind::NotFound && !if_missing =>
        {
            values.set(&[Word::NIL]);
            Ok(Word::NIL)
        }
        Err(FileLoadError::Runtime(RuntimeError::Io { error, .. }))
            if error.kind() == std::io::ErrorKind::NotFound =>
        {
            ctx.set_pending_lisp_error(LispError::FileError(FileError::NotFound));
            Err(ObjectError::TypeError)
        }
        Err(FileLoadError::Runtime(RuntimeError::Native(message))) => {
            crate::native_error::signal_native_load_error(ctx, object, &message)?;
            Err(ObjectError::TypeError)
        }
        Err(FileLoadError::Runtime(
            RuntimeError::Io { .. }
            | RuntimeError::Object(_)
            | RuntimeError::Read(_)
            | RuntimeError::Front(_)
            | RuntimeError::Lower(_)
            | RuntimeError::UndefinedFunction { .. }
            | RuntimeError::NativeFailure { .. },
        )) => {
            ctx.set_pending_lisp_error(LispError::FileError(FileError::InvalidPath));
            Err(ObjectError::TypeError)
        }
    }
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
    source_forms_with_print(runtime, source, TopLevelMode::Execute, false, false)
}

pub fn source_forms_with_mode(
    runtime: &mut Runtime,
    source: &str,
    mode: TopLevelMode,
) -> Result<Word, RuntimeError> {
    source_forms_with_print(runtime, source, mode, false, true)
}

fn source_forms_with_print(
    runtime: &mut Runtime,
    source: &str,
    mode: TopLevelMode,
    print_results: bool,
    use_dynamic_reader: bool,
) -> Result<Word, RuntimeError> {
    let mut input = StringSource::new(source);
    let mut options = None;
    let result_root = HeapRootedWord::new(&runtime.context, Word::NIL)?;

    // Read one form, evaluate it, then read the next: package markers such as
    // `cl-bench:` only resolve once a preceding `defpackage`/`in-package` in
    // this same file has run, so the two steps must interleave rather than
    // reading every form up front.
    let loaded = (|| {
        loop {
            let mut current_options = if use_dynamic_reader {
                current_read_options(runtime, options.take())?
            } else {
                match options.take() {
                    Some(options) => options,
                    None => ReadOptions::standard(&mut runtime.context, &runtime.object)?,
                }
            };
            let Some(form) = read(
                &mut runtime.context,
                &runtime.object,
                &mut input,
                &current_options,
            )?
            else {
                break;
            };
            let package = if use_dynamic_reader {
                None
            } else {
                in_package_name(&runtime.context, form)?
            };
            result_root.set(eval_top_level(runtime, &mut current_options, form, mode)?);
            if let Some(package) = package {
                current_options.set_current_package(package)?;
            }
            if print_results {
                print_loaded_value(runtime, result_root.get())?;
            }
            options = Some(current_options);
        }
        Ok(result_root.get())
    })();
    result_root
        .release()
        .map_err(|_| RuntimeError::Native("load: result root stack corrupted".to_owned()))?;
    loaded
}

pub fn current_read_options(
    runtime: &mut Runtime,
    previous: Option<ReadOptions>,
) -> Result<ReadOptions, RuntimeError> {
    let mut options = match previous {
        Some(options) => options,
        None => ReadOptions::standard(&mut runtime.context, &runtime.object)?,
    };
    let package_word =
        common_lisp_variable_value(&mut runtime.context, &runtime.object, "*PACKAGE*")?;
    let ObjectRef::Package(package) = classify_object(&runtime.context, package_word) else {
        return Err(RuntimeError::Object(ObjectError::TypeError));
    };
    let package_name = word_string(
        &runtime.context,
        Package::from_word(package).name(&runtime.context)?,
    )?;
    let readtable_word =
        common_lisp_variable_value(&mut runtime.context, &runtime.object, "*READTABLE*")?;
    if !matches!(
        classify_object(&runtime.context, readtable_word),
        ObjectRef::Readtable(_)
    ) {
        return Err(RuntimeError::Object(ObjectError::TypeError));
    }
    options.set_current_package(package_name)?;
    options.set_readtable(Readtable::from_object(ObjectReadtable::from_word(
        readtable_word,
    )));
    Ok(options)
}

fn common_lisp_variable_value(
    ctx: &mut ThreadContext,
    object: &ObjectRuntime,
    name: &str,
) -> Result<Word, ObjectError> {
    let mut common_lisp = object
        .find_package(ctx, "COMMON-LISP")
        .ok_or(ObjectError::Layout)?;
    with_root(ctx, &mut common_lisp, |ctx, common_lisp| {
        let (mut symbol, _) = Package::from_word(*common_lisp).intern(ctx, object, name)?;
        with_root(ctx, &mut symbol, |ctx, symbol| symbol_value(ctx, *symbol))
    })
}

fn print_loaded_value(runtime: &mut Runtime, value: Word) -> Result<(), RuntimeError> {
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

fn write_standard_output(runtime: &mut Runtime, text: &str) -> Result<(), RuntimeError> {
    let object = &runtime.object;
    let context = &mut runtime.context;
    let mut text = make_string(context, object, &text.chars().collect::<Vec<_>>())?;
    with_root(context, &mut text, |context, text| {
        call_output_builtin(context, object, "WRITE-STRING", &[*text]).map(|_| ())
    })?;
    Ok(())
}

fn call_output_builtin(
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
