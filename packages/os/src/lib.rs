//! Safe operating-system primitives exposed through `NCL-OS`.

#![forbid(unsafe_code)]

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, LambdaList, MultipleValues, ObjectError, Parameter, ParameterType,
    Runtime, ThreadContext, Word, make_simple_vector, make_string, string_length, string_ref,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static RANDOM_STATE: AtomicU64 = AtomicU64::new(0x9e37_79b9_7f4a_7c15);
const ONE: &[Parameter] = &[Parameter {
    name: BuiltinName::new("PATH"),
    ty: ParameterType::Any,
}];
const TWO: &[Parameter] = &[
    Parameter {
        name: BuiltinName::new("OLD"),
        ty: ParameterType::Any,
    },
    Parameter {
        name: BuiltinName::new("NEW"),
        ty: ParameterType::Any,
    },
];

fn text(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    (0..string_length(ctx, word)?)
        .map(|index| string_ref(ctx, word, index))
        .collect()
}
fn lisp_string(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: &str,
) -> Result<Word, ObjectError> {
    make_string(ctx, runtime, &value.chars().collect::<Vec<_>>())
}

fn getenv(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let name = text(ctx, args.required(0)?)?;
    ncl_sys::enter_native(ctx.thread_mut());
    let value = std::env::var_os(name);
    ncl_sys::leave_native(ctx.thread_mut());
    value.map_or(Ok(Word::NIL), |v| {
        let value = v.to_string_lossy();
        lisp_string(ctx, runtime, &value)
    })
}
fn setenv(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let name = text(ctx, args.required(0)?)?;
    let value = text(ctx, args.required(1)?)?;
    ncl_sys::enter_native(ctx.thread_mut());
    ncl_sys::set_environment_variable(&name, &value).map_err(|_| ObjectError::Unsupported)?;
    ncl_sys::leave_native(ctx.thread_mut());
    Ok(Word::fixnum(1))
}
fn current_directory(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    _: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    ncl_sys::enter_native(ctx.thread_mut());
    let value = std::env::current_dir();
    ncl_sys::leave_native(ctx.thread_mut());
    let value = value
        .map_err(|_| ObjectError::Unsupported)?
        .to_string_lossy()
        .into_owned();
    lisp_string(ctx, runtime, &value)
}
fn delete_file(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let path = text(ctx, args.required(0)?)?;
    ncl_sys::enter_native(ctx.thread_mut());
    let ok = std::fs::remove_file(path).is_ok();
    ncl_sys::leave_native(ctx.thread_mut());
    Ok(if ok { Word::fixnum(1) } else { Word::NIL })
}
fn rename_file(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let old = text(ctx, args.required(0)?)?;
    let new = text(ctx, args.required(1)?)?;
    ncl_sys::enter_native(ctx.thread_mut());
    let ok = std::fs::rename(old, new).is_ok();
    ncl_sys::leave_native(ctx.thread_mut());
    Ok(if ok { Word::fixnum(1) } else { Word::NIL })
}
fn file_stat(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let path = text(ctx, args.required(0)?)?;
    ncl_sys::enter_native(ctx.thread_mut());
    let metadata = std::fs::metadata(path);
    ncl_sys::leave_native(ctx.thread_mut());
    let metadata = metadata.map_err(|_| ObjectError::Unsupported)?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    make_simple_vector(
        ctx,
        runtime,
        &[
            Word::fixnum(i64::try_from(metadata.len()).map_err(|_| ObjectError::Layout)?),
            Word::fixnum(i64::try_from(modified).map_err(|_| ObjectError::Layout)?),
            if metadata.is_dir() {
                Word::fixnum(1)
            } else {
                Word::NIL
            },
        ],
    )
}
fn get_time(
    _: &mut ThreadContext,
    _: &Runtime,
    _: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ObjectError::Unsupported)?
        .as_secs();
    Ok(Word::fixnum(
        i64::try_from(seconds).map_err(|_| ObjectError::Layout)?,
    ))
}
fn random_u64(
    _: &mut ThreadContext,
    _: &Runtime,
    _: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut current = RANDOM_STATE.load(Ordering::Relaxed);
    loop {
        let next = current ^ (current << 7) ^ (current >> 9);
        match RANDOM_STATE.compare_exchange_weak(
            current,
            next,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => {
                return Ok(Word::fixnum(
                    i64::try_from(next >> 1).map_err(|_| ObjectError::Layout)?,
                ));
            }
            Err(observed) => current = observed,
        }
    }
}
fn register_one(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &'static str,
    params: &'static [Parameter],
    callback: ncl_object::RustBuiltin,
) -> Result<(), ObjectError> {
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::NclOs, BuiltinName::new(name)),
        BuiltinImplementation::direct(
            Builtin {
                lambda_list: LambdaList::fixed(params),
                convention: BuiltinConvention::Direct(Arity::exact(
                    u8::try_from(params.len()).map_err(|_| ObjectError::Layout)?,
                )),
            },
            callback,
        ),
    )?;
    Ok(())
}
/// Register the implemented operating-system primitives.
///
/// # Errors
/// Returns an object-layer error when package or builtin registration fails.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    register_one(runtime, &mut ctx, "GETENV", ONE, getenv)?;
    register_one(runtime, &mut ctx, "SETENV", TWO, setenv)?;
    register_one(runtime, &mut ctx, "DELETE-FILE", ONE, delete_file)?;
    register_one(runtime, &mut ctx, "RENAME-FILE", TWO, rename_file)?;
    register_one(runtime, &mut ctx, "FILE-STAT", ONE, file_stat)?;
    register_one(
        runtime,
        &mut ctx,
        "CURRENT-DIRECTORY",
        &[],
        current_directory,
    )?;
    register_one(runtime, &mut ctx, "GET-TIME", &[], get_time)?;
    register_one(runtime, &mut ctx, "RANDOM-U64", &[], random_u64)?;
    Ok(())
}
