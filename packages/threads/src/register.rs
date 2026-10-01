//! Registration of the owned symbols and the thread-layer classes.
//!
//! This is the per-crate entry point that `ncl-stdlib` calls in dependency
//! order. It interns every `conformance/ownership/symbols.tsv` row assigned to
//! `ncl-threads`, sets the special, macro, and constant bits the ownership gate
//! checks, registers the runtime-side function for each function and macro
//! symbol, and installs the class descriptors the object model uses.

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, LambdaList, MultipleValues, ObjectError, Package, Parameter,
    ParameterType, Runtime, ThreadContext, Word, make_simple_vector, make_string, pop_root,
    push_root, set_symbol_constant, set_symbol_macro, set_symbol_special, string_length,
    string_ref,
};

use crate::symbols::{SymbolKind, SymbolRow, rows};

/// Classes installed by this crate, paired with their home package.
///
/// The first seven come from the ownership table; `PROCESS`, `RWLOCK`, and
/// `SPINLOCK` are support classes for objects this crate creates. `SPINLOCK` is
/// a `type` row in the table and is interned as well as installed here.
const CLASSES: &[(&str, &str)] = &[
    ("TIMER", "NCL-THREADS"),
    ("FOREIGN-THREAD", "NCL-THREADS"),
    ("MUTEX", "NCL-THREADS"),
    ("RWLOCK", "NCL-THREADS"),
    ("SEMAPHORE", "NCL-THREADS"),
    ("SEMAPHORE-NOTIFICATION", "NCL-THREADS"),
    ("SPINLOCK", "NCL-THREADS"),
    ("THREAD", "NCL-THREADS"),
    ("WAITQUEUE", "NCL-THREADS"),
    ("PROCESS", "NCL-THREADS"),
];

/// Register every owned symbol and class with `runtime`.
///
/// # Errors
/// Returns an object-layer error when allocation, interning, or registration
/// fails.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    for row in rows() {
        register_row(runtime, &mut ctx, row)?;
    }
    install_classes(runtime, &mut ctx)?;
    install_builtins(runtime, &mut ctx)
}

const ANY1: &[Parameter] = &[Parameter {
    name: BuiltinName::new("OBJECT"),
    ty: ParameterType::Any,
}];
const ANY2: &[Parameter] = &[
    Parameter {
        name: BuiltinName::new("NAME"),
        ty: ParameterType::Any,
    },
    Parameter {
        name: BuiltinName::new("FUNCTION"),
        ty: ParameterType::Any,
    },
];
const ANY3: &[Parameter] = &[
    Parameter {
        name: BuiltinName::new("QUEUE"),
        ty: ParameterType::Any,
    },
    Parameter {
        name: BuiltinName::new("MUTEX"),
        ty: ParameterType::Any,
    },
    Parameter {
        name: BuiltinName::new("TIMEOUT"),
        ty: ParameterType::Any,
    },
];

fn install_builtins(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
    register_direct(runtime, ctx, "MAKE-THREAD", ANY2, make_thread_builtin)?;
    register_direct(runtime, ctx, "JOIN-THREAD", ANY1, join_thread_builtin)?;
    register_direct(runtime, ctx, "THREAD-ALIVE-P", ANY1, thread_alive_builtin)?;
    register_direct(runtime, ctx, "CURRENT-THREAD", &[], current_thread_builtin)?;
    register_direct(
        runtime,
        ctx,
        "TERMINATE-THREAD",
        ANY1,
        terminate_thread_builtin,
    )?;
    register_direct(runtime, ctx, "THREAD-YIELD", &[], thread_yield_builtin)?;
    register_direct(runtime, ctx, "MUTEX-MAKE", ANY1, mutex_make_builtin)?;
    register_direct(runtime, ctx, "MUTEX-LOCK", ANY1, mutex_lock_builtin)?;
    register_direct(runtime, ctx, "MUTEX-UNLOCK", ANY1, mutex_unlock_builtin)?;
    register_direct(runtime, ctx, "SEMAPHORE-MAKE", ANY2, semaphore_make_builtin)?;
    register_direct(runtime, ctx, "SEMAPHORE-WAIT", ANY1, semaphore_wait_builtin)?;
    register_direct(runtime, ctx, "SEMAPHORE-POST", ANY1, semaphore_post_builtin)?;
    register_direct(runtime, ctx, "CONDITION-WAIT", ANY3, condition_wait_builtin)?;
    register_direct(
        runtime,
        ctx,
        "CONDITION-NOTIFY",
        ANY1,
        condition_notify_builtin,
    )?;
    register_direct(
        runtime,
        ctx,
        "CONDITION-BROADCAST",
        ANY1,
        condition_broadcast_builtin,
    )
}

fn register_direct(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &'static str,
    parameters: &'static [Parameter],
    function: ncl_object::RustBuiltin,
) -> Result<(), ObjectError> {
    let descriptor = Builtin {
        lambda_list: LambdaList::fixed(parameters),
        convention: BuiltinConvention::Direct(Arity::exact(
            u8::try_from(parameters.len()).map_err(|_| ObjectError::Layout)?,
        )),
    };
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::NclThreads, BuiltinName::new(name)),
        BuiltinImplementation::direct(descriptor, function),
    )?;
    Ok(())
}

fn string_arg(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    let length = string_length(ctx, word)?;
    (0..length)
        .map(|index| string_ref(ctx, word, index))
        .collect::<Result<String, _>>()
}

fn make_thread_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let name = string_arg(ctx, args.required(0)?)?;
    let function = args.required(1)?;
    let shared = runtime.shared_handle().ok_or(ObjectError::Layout)?;
    crate::make_thread(ctx, &shared, &name, function).map_err(|_| ObjectError::TypeError)
}

fn join_thread_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    crate::join_thread_value(ctx, args.required(0)?).map_err(|_| ObjectError::TypeError)
}

fn thread_alive_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    crate::thread_alive_p(ctx, args.required(0)?).map_err(|_| ObjectError::TypeError)
}

fn current_thread_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    _args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    crate::current_thread(ctx, runtime).map_err(|_| ObjectError::TypeError)
}

fn terminate_thread_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    crate::terminate_thread(ctx, args.required(0)?).map_err(|_| ObjectError::TypeError)?;
    Ok(Word::TRUE)
}

fn thread_yield_builtin(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    _args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    crate::thread_yield();
    Ok(Word::NIL)
}

fn mutex_make_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let name = string_arg(ctx, args.required(0)?)?;
    crate::make_mutex(ctx, runtime, &name, crate::MutexKind::NonRecursive)
        .map_err(|_| ObjectError::TypeError)
}

fn mutex_lock_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    crate::get_mutex(ctx, args.required(0)?, true, None).map_err(|_| ObjectError::TypeError)
}

fn mutex_unlock_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    crate::release_mutex(ctx, args.required(0)?)
        .map(|()| Word::TRUE)
        .map_err(|_| ObjectError::TypeError)
}

fn semaphore_make_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let name = string_arg(ctx, args.required(0)?)?;
    let count = args
        .required(1)?
        .as_fixnum()
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(ObjectError::TypeError)?;
    crate::make_semaphore(ctx, runtime, &name, count).map_err(|_| ObjectError::TypeError)
}

fn semaphore_wait_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    crate::wait_on_semaphore(ctx, args.required(0)?, None).map_err(|_| ObjectError::TypeError)
}

fn semaphore_post_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    crate::signal_semaphore(ctx, args.required(0)?).map_err(|_| ObjectError::TypeError)
}

fn condition_wait_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    crate::condition_wait(ctx, args.required(0)?, args.required(1)?, None)
        .map_err(|_| ObjectError::TypeError)
}

fn condition_notify_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    crate::condition_notify(ctx, args.required(0)?).map_err(|_| ObjectError::TypeError)
}

fn condition_broadcast_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    crate::condition_broadcast(ctx, args.required(0)?).map_err(|_| ObjectError::TypeError)
}

fn register_row(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    row: &SymbolRow,
) -> Result<(), ObjectError> {
    let package = runtime.ensure_package(ctx, row.package)?;
    let (mut symbol, _status) = Package::from_word(package).intern(ctx, runtime, row.name)?;
    let token = push_root(ctx, &mut symbol);
    let result = apply_kind(runtime, ctx, row, symbol);
    let _ = pop_root(ctx, token);
    result
}

fn apply_kind(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    row: &SymbolRow,
    symbol: Word,
) -> Result<(), ObjectError> {
    match row.kind {
        SymbolKind::Function => runtime.define_function(ctx, row.package, row.name, Word::UNBOUND),
        SymbolKind::Macro => {
            runtime.define_function(ctx, row.package, row.name, Word::UNBOUND)?;
            set_symbol_macro(ctx, symbol, true)
        }
        SymbolKind::Variable => set_symbol_special(ctx, symbol, true),
        SymbolKind::Constant => set_symbol_constant(ctx, symbol, true),
        SymbolKind::Class
        | SymbolKind::Type
        | SymbolKind::Other
        | SymbolKind::ClassAndFunction
        | SymbolKind::MacroAndClass
        | SymbolKind::SpecialOperatorAndClass
        | SymbolKind::VariableAndFunction => Ok(()),
    }
}

fn install_classes(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
    for (name, package) in CLASSES {
        let package = runtime.ensure_package(ctx, package)?;
        let _ = Package::from_word(package).intern(ctx, runtime, name)?;
        let descriptor = class_descriptor(ctx, runtime, name)?;
        runtime.define_class(ctx, *name, descriptor)?;
    }
    Ok(())
}

fn class_descriptor(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<Word, ObjectError> {
    let name_word = make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
    make_simple_vector(ctx, runtime, &[name_word, Word::NIL, Word::NIL])
}
