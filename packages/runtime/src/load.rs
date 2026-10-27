use std::path::{Path, PathBuf};

use crate::evalwhen::TopLevelMode;
use crate::{Runtime, RuntimeError, compile, file_environment};
use ncl_compiler_front::form::word_string;
use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, HeapRootedWord, LambdaList, LispError, MultipleValues, ObjectError, ObjectRef,
    Parameter, ParameterType, Readtable as ObjectReadtable, Runtime as ObjectRuntime,
    ThreadContext, Word, classify_object, make_string, symbol_name, symbol_package,
};
use ncl_reader::{ReadOptions, Readtable, StringSource, read};

mod read_options;
mod support;
mod top_level;

pub use read_options::current_read_options;
use read_options::{apply_current_read_flags, common_lisp_variable_value};
use support::{host_architecture, in_package_name, print_loaded_value, write_standard_output};
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
    let ParsedLoadArguments {
        pathname,
        path,
        if_missing,
        options,
    } = parse_load_arguments(ctx, object, args)?;
    match file_with_options(runtime, &path, options) {
        Ok(_) => {
            values.set(&[Word::TRUE]);
            Ok(Word::TRUE)
        }
        Err(FileLoadError::UnsupportedExternalFormat) => {
            signal_load_error(ctx, object, "unsupported source external format")
        }
        Err(FileLoadError::Runtime(RuntimeError::Io { error, .. }))
            if error.kind() == std::io::ErrorKind::NotFound && !if_missing =>
        {
            values.set(&[Word::NIL]);
            Ok(Word::NIL)
        }
        Err(FileLoadError::Runtime(RuntimeError::Io { .. })) => {
            let condition = ncl_conditions::make_typed_condition(
                ctx,
                object,
                ncl_conditions::ConditionIdentifier::FileError,
                &[ncl_conditions::ConditionSlotValue::from_word(
                    pathname.get(),
                )],
            )
            .map_err(condition_error_to_object_error)?;
            ctx.set_pending_condition(condition.as_word());
            Err(ObjectError::TypeError)
        }
        Err(FileLoadError::Runtime(RuntimeError::Read(ncl_reader::ReadError::UnexpectedEof))) => {
            ctx.set_pending_lisp_error(LispError::EndOfFile);
            Err(ObjectError::TypeError)
        }
        Err(FileLoadError::Runtime(RuntimeError::Read(ncl_reader::ReadError::Object(error)))) => {
            Err(error)
        }
        Err(FileLoadError::Runtime(RuntimeError::Read(error))) => {
            let diagnostic =
                make_string(ctx, object, &error.to_string().chars().collect::<Vec<_>>())?;
            let diagnostic = HeapRootedWord::new(ctx, diagnostic)?;
            // LOAD has no Lisp stream for slot 0; slot 1 retains the reader diagnostic.
            let condition = ncl_conditions::make_typed_condition(
                ctx,
                object,
                ncl_conditions::ConditionIdentifier::ReaderError,
                &[
                    ncl_conditions::ConditionSlotValue::from_word(Word::NIL),
                    ncl_conditions::ConditionSlotValue::from_word(diagnostic.get()),
                ],
            )
            .map_err(condition_error_to_object_error)?;
            ctx.set_pending_condition(condition.as_word());
            Err(ObjectError::TypeError)
        }
        Err(FileLoadError::Runtime(RuntimeError::Native(message))) => {
            signal_load_error(ctx, object, &message)
        }
        Err(FileLoadError::Runtime(
            error @ (RuntimeError::Object(_)
            | RuntimeError::Front(ncl_compiler_front::FrontError::Object(_))
            | RuntimeError::UndefinedFunction { .. }
            | RuntimeError::NativeFailure { .. }
            | RuntimeError::PendingCondition { .. }),
        )) => Err(crate::native_error::runtime_error_to_object_error(
            ctx,
            object,
            error,
            runtime.pending_undefined_function_condition(),
        )),
        Err(FileLoadError::Runtime(error @ (RuntimeError::Front(_) | RuntimeError::Lower(_)))) => {
            signal_load_error(ctx, object, &error.to_string())
        }
    }
}

fn signal_load_error(
    ctx: &mut ThreadContext,
    object: &ObjectRuntime,
    message: &str,
) -> Result<Word, ObjectError> {
    crate::native_error::signal_native_load_error(ctx, object, message)?;
    Err(ObjectError::TypeError)
}

const fn condition_error_to_object_error(error: ncl_conditions::ConditionError) -> ObjectError {
    match error {
        ncl_conditions::ConditionError::Object(error) => error,
        // check-added-lines: allow(wildcard) ConditionError is non-exhaustive.
        _ => ObjectError::Layout,
    }
}

struct ParsedLoadArguments {
    pathname: HeapRootedWord,
    path: PathBuf,
    if_missing: bool,
    options: FileLoadOptions,
}

fn parse_load_arguments(
    ctx: &mut ThreadContext,
    object: &ObjectRuntime,
    args: &BuiltinArgs<'_>,
) -> Result<ParsedLoadArguments, ObjectError> {
    let pathname = HeapRootedWord::new(ctx, args.required(0)?)?;
    let path = ncl_lib_pathnames::resolve_pathname(ctx, object, pathname.get())?;
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
    Ok(ParsedLoadArguments {
        pathname,
        path,
        if_missing,
        options: FileLoadOptions {
            print_results,
            verbose,
            external_format_supported,
        },
    })
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
            if !use_dynamic_reader {
                let readtable_root = HeapRootedWord::new(
                    &runtime.context,
                    current_options.readtable().object().as_word(),
                )?;
                apply_current_read_flags(runtime, &mut current_options)?;
                current_options.set_readtable(Readtable::from_object(ObjectReadtable::from_word(
                    readtable_root.get(),
                )));
            }
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
