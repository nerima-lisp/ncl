//! Native entry points for `catch`/`throw`/`unwind-protect`/`progv`.
//!
//! These `extern "C"` functions are the addresses [`crate::support::NativeAbi`]
//! resolves for the `EnterCatch`/`LeaveCatch`/`EnterUnwindProtect`/
//! `LeaveUnwindProtect`/`EnterProgv`/`LeaveProgv` runtime functions and the
//! `COMMON-LISP::throw` builtin. Each one reaches the full `ThreadContext`
//! through the synchronous native-callback context installed for the
//! duration of one compiled-code invocation, following the same
//! [`ncl_sys::with_native_context`] pattern as `native_make_closure`
//! (`support.rs`).
//!
//! Calling convention (fixed by `ncl-codegen`'s `lower_runtime_builtin`):
//! register 0 is always the thread pointer; immediate (compile-time-known)
//! arguments come next as raw integers, followed by value arguments as ABI
//! words. See the `OpKind::EnterHandler`/`LeaveHandler` lowering in
//! `target_{aarch64,x86_64}_lowering/ops.rs` for the exact argument order
//! this module's signatures must match.

use std::ptr::NonNull;

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, LambdaList, MultipleValues, ObjectError, Parameter, ParameterType,
    Runtime as ObjectRuntime, ThreadContext, Word,
};
use ncl_sys::Thread;

use crate::support::NativeInvocation;

/// Run `body` with the `ThreadContext` reached through the installed native
/// callback context, returning `Word::NIL` if no context is installed (which
/// should not happen for code compiled through this crate).
#[allow(clippy::option_if_let_else)]
fn with_context(thread: NonNull<Thread>, body: impl FnOnce(&mut ThreadContext) -> Word) -> Word {
    match ncl_sys::with_native_context(thread, |invocation: &mut NativeInvocation<'_>| {
        body(invocation.context)
    }) {
        Some(value) => value,
        None => unreachable!("native context missing"), // check-added-lines: allow(panic) impossible ABI state
    }
}

/// Establish a `catch` frame. See [`ThreadContext::enter_catch`].
///
pub extern "C" fn native_enter_catch(
    thread: NonNull<Thread>,
    _region: u64,
    _depth: u64,
    tag: Word,
) -> Word {
    with_context(thread, |ctx| {
        ctx.enter_catch(tag);
        Word::NIL
    })
}

/// Leave the innermost `catch` frame. See [`ThreadContext::leave_catch`].
///
pub extern "C" fn native_leave_catch(thread: NonNull<Thread>, _region: u64) -> Word {
    with_context(thread, |ctx| {
        if let Err(error) = ctx.leave_catch() {
            ctx.set_pending(error);
        }
        Word::NIL
    })
}

/// Establish an `unwind-protect` frame. See
/// [`ThreadContext::enter_unwind_protect`].
///
pub extern "C" fn native_enter_unwind_protect(
    thread: NonNull<Thread>,
    _region: u64,
    _cleanup_block: u64,
) -> Word {
    with_context(thread, |ctx| {
        ctx.enter_unwind_protect();
        Word::NIL
    })
}

/// Leave the innermost `unwind-protect` frame. See
/// [`ThreadContext::leave_unwind_protect`].
///
pub extern "C" fn native_leave_unwind_protect(thread: NonNull<Thread>, _region: u64) -> Word {
    with_context(thread, |ctx| {
        ctx.leave_unwind_protect();
        Word::NIL
    })
}

/// Establish a `progv` frame. See [`ThreadContext::enter_progv`].
///
/// A malformed binding list records a pending [`ObjectError`] (surfaced at
/// the Rust/native boundary through [`ThreadContext::take_pending`]) rather
/// than panicking.
///
pub extern "C" fn native_enter_progv(
    thread: NonNull<Thread>,
    _region: u64,
    symbols: Word,
    values: Word,
) -> Word {
    with_context(thread, |ctx| {
        if let Err(error) = ctx.enter_progv(symbols, values) {
            ctx.set_pending(error);
        }
        Word::NIL
    })
}

/// Leave the innermost `progv` frame. See [`ThreadContext::leave_progv`].
///
pub extern "C" fn native_leave_progv(thread: NonNull<Thread>, _region: u64) -> Word {
    with_context(thread, |ctx| {
        if let Err(error) = ctx.leave_progv() {
            ctx.set_pending(error);
        }
        Word::NIL
    })
}

/// Native-ABI entry for `COMMON-LISP::throw`, dispatched by
/// `OpKind::Builtin`. See [`ThreadContext::throw`].
///
/// An escaping tag (no enclosing `catch` currently established it) records a
/// pending [`ObjectError::ControlError`], surfaced once compiled code
/// finishes unwinding and control reaches the Rust/native boundary (see
/// [`ThreadContext::take_pending`]). `thread.pending` is also set in this
/// case so generated code keeps propagating (running any `unwind-protect`
/// cleanups it passes through) instead of continuing to execute as though
/// the `throw` had returned a value.
pub extern "C" fn native_throw(thread: NonNull<Thread>, tag: Word, value: Word) -> Word {
    with_context(thread, |ctx| {
        if let Err(error) = ctx.throw(tag, value) {
            ctx.set_pending(error);
            ctx.set_non_local_exit(true);
        }
        Word::NIL
    })
}

/// Safe Rust implementation of `throw`, used when it is invoked through the
/// ordinary [`ncl_object::FunctionCaller`] path (for example `(funcall
/// #'throw 'k 5)`) rather than the `OpKind::Builtin` fast path.
///
/// # Errors
/// Returns [`ObjectError::ControlError`] when `tag` names no enclosing
/// `catch`.
fn throw_builtin(
    ctx: &mut ThreadContext,
    _runtime: &ObjectRuntime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let tag = args.required(0)?;
    let value = args.required(1)?;
    ctx.throw(tag, value)?;
    Ok(value)
}

fn control_builtin_address(
    function: extern "C" fn(NonNull<Thread>, Word, Word) -> Word,
) -> Result<usize, ObjectError> {
    let raw = function as *const (); // check-added-lines: allow(as-cast) function-pointer-to-address conversion, the same pattern `function_address!` uses internally.
    let address = ncl_sys::function_address(raw).map_err(|_| ObjectError::Layout)?;
    usize::try_from(address).map_err(|_| ObjectError::Layout)
}

/// Register `COMMON-LISP::throw` with both the safe Rust callback (used by
/// `funcall`) and the native ABI trampoline (used by `OpKind::Builtin`).
///
/// # Errors
/// Returns the allocation, layout, or storage error reported while
/// registering the builtin.
pub fn register_control_builtins(
    ctx: &mut ThreadContext,
    runtime: &ObjectRuntime,
) -> Result<(), ObjectError> {
    const PARAMETERS: &[Parameter] = &[
        Parameter {
            name: BuiltinName::new("TAG"),
            ty: ParameterType::Any,
        },
        Parameter {
            name: BuiltinName::new("RESULT-FORM"),
            ty: ParameterType::Any,
        },
    ];
    let descriptor = Builtin {
        lambda_list: LambdaList::fixed(PARAMETERS),
        convention: BuiltinConvention::Direct(Arity::exact(2)),
    };
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("throw")),
        BuiltinImplementation::direct(descriptor, throw_builtin)
            .with_entry(control_builtin_address(native_throw)?),
    )?;
    Ok(())
}
