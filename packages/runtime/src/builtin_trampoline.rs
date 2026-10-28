//! Generic native trampoline shared by every builtin with no ISA-specific
//! fast path.
//!
//! Compiled Lisp code calls a function object's `ENTRY` slot directly, using
//! the native calling convention that `packages/codegen`'s `lower_call`/
//! `lower_closure_call` emit for every `Call`/`CallClosure` site: a tagged
//! argc in the first argument register, up to four tagged argument words in
//! the next four registers, the callee function object in a pinned "self"
//! register (`x16` on `AArch64`, `r10` on x86-64), and the live thread
//! context pointer in a second pinned register (`x21`/`r15`). That is *not*
//! the calling convention of a safe Rust [`ncl_object::RustBuiltin`]
//! (`fn(&mut ThreadContext, &Runtime, &BuiltinArgs, &mut MultipleValues) ->
//! Result<Word, ObjectError>`), so `Runtime::register_builtin` can never hand
//! out `function as usize` as a usable native `ENTRY`: a `blr`/`call` into it
//! reinterprets unrelated registers as `&mut ThreadContext` and friends and
//! corrupts the native frame instead of raising a Lisp condition.
//!
//! This module publishes one hand-assembled boundary stub per ISA. Each stub
//! preserves the generated caller's frame pointer and continuation PC, then
//! presents the native call using the ordinary C calling convention. The
//! dispatcher recovers the live `Runtime`/`ThreadContext` pair through the
//! same native-context side channel `native_make_closure` already uses (see
//! `crate::support::NativeInvocation`) and forwards the call to
//! `ncl_object::Runtime::call_builtin`, which already performs arity
//! checking, `&key`/`&rest` adaptation, and pending-condition conversion.
use ncl_object::{
    BuiltinIdentifier, BuiltinName, BuiltinPackage, CellError, FunctionObject, LispError, Package,
    Runtime as ObjectRuntime, ThreadContext, Word, symbol_function,
};
use ncl_sys::{CodePtr, Thread, alloc_code, publish_code, write_code};
use std::ptr::NonNull;

#[path = "builtin_trampoline_boundary_stub.rs"]
mod boundary_stub;
#[cfg(test)]
use boundary_stub::build_x86_64_stub;

use crate::RuntimeError;
use crate::support::NativeInvocation;
#[path = "builtin_trampoline_dispatch.rs"]
mod dispatch_impl;
use dispatch_impl::dispatch;
pub use dispatch_impl::record_boundary_error;
#[path = "builtin_trampoline_keywords.rs"]
mod keyword_impl;
use keyword_impl::install_keyword_builtins;
#[path = "arith_fast.rs"]
mod arith_fast;
pub use arith_fast::install as install_arith_fast;

pub fn unregister_heap_roots(
    object: &ObjectRuntime,
    tokens: impl IntoIterator<Item = ncl_sys::RootToken>,
) -> Result<(), ncl_object::ObjectError> {
    let mut first_error = None;
    for token in tokens {
        if let Err(error) = ncl_object::unregister_heap_root(object, token)
            && first_error.is_none()
        {
            first_error = Some(error);
        }
    }
    first_error.map_or(Ok(()), Err)
}
/// `(primary value, value count)`, the pair a native entry already returns to
/// `invoke_entry_with_function_address` (value in the first return register,
/// count in the second): `#[repr(C)]` with two eight-byte integer fields is
/// classified identically on `AArch64` (`x0`/`x1`) and x86-64 (`rax`/`rdx`).
#[repr(C)]
struct NativeCallResult {
    value: u64,
    count: u64,
}
const fn error_result() -> NativeCallResult {
    NativeCallResult {
        value: Word::NIL.bits(),
        count: 0,
    }
}
#[allow(clippy::too_many_arguments)]
extern "C" fn make_rest_list_native(
    thread_ptr: *mut Thread,
    a0: u64,
    a1: u64,
    a2: u64,
    a3: u64,
    rest: u64,
    argc: u64,
    start: u64,
) -> u64 {
    let Some(thread) = NonNull::new(thread_ptr) else {
        return Word::NIL.bits();
    };
    let Some(result) =
        ncl_sys::with_native_context(thread, |invocation: &mut NativeInvocation<'_>| {
            let context = &mut *invocation.context;
            let object = invocation.object;
            let Some(count) = Word::from_bits(argc)
                .as_fixnum()
                .and_then(|value| usize::try_from(value).ok())
                .filter(|count| *count <= ncl_sys::CALL_ARGUMENTS_LIMIT)
            else {
                context.set_pending(ncl_object::ObjectError::Layout);
                return Word::NIL.bits();
            };
            let mut arguments = vec![
                Word::from_bits(argc),
                Word::from_bits(start),
                Word::from_bits(a0),
                Word::from_bits(a1),
                Word::from_bits(a2),
                Word::from_bits(a3),
            ];
            arguments.truncate(2 + count.min(4));
            if count > 4 {
                let Ok(rest_words) = ncl_sys::copy_native_words(rest, count - 4) else {
                    context.set_pending(ncl_object::ObjectError::Layout);
                    return Word::NIL.bits();
                };
                arguments.extend(rest_words);
            }
            let argument_tokens = arguments
                .iter_mut()
                .map(|word| ncl_object::push_heap_root(object, word))
                .collect::<Vec<_>>();
            let result = (|| {
                let Some(package) = object.find_package(context, "NCL-EXT") else {
                    context.set_pending(ncl_object::ObjectError::Layout);
                    return Word::NIL.bits();
                };
                let Ok((name, _)) =
                    Package::from_word(package).intern(context, object, "MAKE-REST-LIST")
                else {
                    context.set_pending(ncl_object::ObjectError::Layout);
                    return Word::NIL.bits();
                };
                let Ok(function_word) = symbol_function(context, name) else {
                    context.set_pending(ncl_object::ObjectError::Unbound);
                    return Word::NIL.bits();
                };
                let Ok(function) = FunctionObject::try_from(function_word) else {
                    context.set_pending(ncl_object::ObjectError::Unbound);
                    return Word::NIL.bits();
                };
                let mut rooted_function = function.as_word();
                let function_token = ncl_object::push_heap_root(object, &mut rooted_function);
                let result = FunctionObject::try_from(rooted_function).and_then(|function| {
                    object.call_builtin_from_generated_code(context, function, &arguments)
                });
                let function_popped = ncl_object::unregister_heap_root(object, function_token);
                if function_popped.is_err() {
                    context.set_pending(ncl_object::ObjectError::RootStackCorrupted);
                    return Word::NIL.bits();
                }
                match result {
                    Ok(value) => value.bits(),
                    Err(error) => {
                        record_boundary_error(context, error);
                        Word::NIL.bits()
                    }
                }
            })();
            let arguments_popped = unregister_heap_roots(object, argument_tokens.into_iter().rev());
            if arguments_popped.is_err() {
                context.set_pending(ncl_object::ObjectError::RootStackCorrupted);
                return Word::NIL.bits();
            }
            result
        })
    else {
        return Word::NIL.bits();
    };
    result
}
extern "C" fn keyword_check_native(
    thread_ptr: *mut Thread,
    a0: u64,
    a1: u64,
    a2: u64,
    _a3: u64,
    _rest: u64,
    _argc: u64,
    _start: u64,
) -> u64 {
    call_keyword_builtin(thread_ptr, "CHECK-KEYWORDS", &[a0, a1, a2])
}
extern "C" fn keyword_value_native(
    thread_ptr: *mut Thread,
    a0: u64,
    a1: u64,
    _a2: u64,
    _a3: u64,
    _rest: u64,
    _argc: u64,
    _start: u64,
) -> u64 {
    call_keyword_builtin(thread_ptr, "KEYWORD-VALUE", &[a0, a1])
}
extern "C" fn keyword_supplied_native(
    thread_ptr: *mut Thread,
    a0: u64,
    a1: u64,
    _a2: u64,
    _a3: u64,
    _rest: u64,
    _argc: u64,
    _start: u64,
) -> u64 {
    call_keyword_builtin(thread_ptr, "KEYWORD-SUPPLIED-P", &[a0, a1])
}
fn call_keyword_builtin(thread_ptr: *mut Thread, name: &str, words: &[u64]) -> u64 {
    let Some(thread) = NonNull::new(thread_ptr) else {
        return Word::NIL.bits();
    };
    let Some(result) =
        ncl_sys::with_native_context(thread, |invocation: &mut NativeInvocation<'_>| {
            let context = &mut *invocation.context;
            let object = invocation.object;
            let mut arguments = words
                .iter()
                .copied()
                .map(Word::from_bits)
                .collect::<Vec<_>>();
            let argument_tokens = arguments
                .iter_mut()
                .map(|word| ncl_object::push_heap_root(object, word))
                .collect::<Vec<_>>();
            let result = (|| {
                let Some(package) = object.find_package(context, "NCL-EXT") else {
                    context.set_pending(ncl_object::ObjectError::Layout);
                    return Word::NIL.bits();
                };
                let Ok((symbol, _)) = Package::from_word(package).intern(context, object, name)
                else {
                    context.set_pending(ncl_object::ObjectError::Layout);
                    return Word::NIL.bits();
                };
                let Ok(function_word) = symbol_function(context, symbol) else {
                    context.set_pending(ncl_object::ObjectError::Unbound);
                    return Word::NIL.bits();
                };
                let Ok(function) = FunctionObject::try_from(function_word) else {
                    context.set_pending(ncl_object::ObjectError::Unbound);
                    return Word::NIL.bits();
                };
                let mut rooted_function = function.as_word();
                let function_token = ncl_object::push_heap_root(object, &mut rooted_function);
                let result = FunctionObject::try_from(rooted_function).and_then(|function| {
                    object.call_builtin_from_generated_code(context, function, &arguments)
                });
                let function_popped = ncl_object::unregister_heap_root(object, function_token);
                if function_popped.is_err() {
                    context.set_pending(ncl_object::ObjectError::RootStackCorrupted);
                    return Word::NIL.bits();
                }
                match result {
                    Ok(value) => value.bits(),
                    Err(error) => {
                        record_boundary_error(context, error);
                        Word::NIL.bits()
                    }
                }
            })();
            let arguments_popped = unregister_heap_roots(object, argument_tokens.into_iter().rev());
            if arguments_popped.is_err() {
                context.set_pending(ncl_object::ObjectError::RootStackCorrupted);
                return Word::NIL.bits();
            }
            result
        })
    else {
        return Word::NIL.bits();
    };
    result
}
extern "C" fn undefined_function_dispatch(
    _argc: u64,
    _a0: u64,
    _a1: u64,
    _a2: u64,
    _a3: u64,
    _rest: u64,
    symbol: u64,
    thread_ptr: u64,
    frame_fp: u64,
    return_pc: u64,
) -> NativeCallResult {
    let Ok(address) = usize::try_from(thread_ptr) else {
        return error_result();
    };
    let Some(thread) = NonNull::new(std::ptr::without_provenance_mut::<Thread>(address)) else {
        return error_result();
    };
    let Ok(frame_fp) = usize::try_from(frame_fp) else {
        return error_result();
    };
    let Ok(return_pc) = usize::try_from(return_pc) else {
        return error_result();
    };
    #[allow(clippy::option_if_let_else)]
    match ncl_sys::with_native_context(thread, |invocation: &mut NativeInvocation<'_>| {
        boundary_stub::with_generated_frame(invocation, frame_fp, return_pc, |invocation| {
            let error = LispError::CellError(CellError::UndefinedFunction {
                name: Word::from_bits(symbol),
            });
            let condition = match ncl_conditions::condition_from_lisp_error(
                invocation.context,
                invocation.object,
                error,
            ) {
                Ok(condition) => condition,
                Err(error) => {
                    record_boundary_error(invocation.context, error);
                    return error_result();
                }
            };
            dispatch_impl::signal_condition_error(
                invocation.context,
                condition,
                ncl_object::ObjectError::UndefinedFunction,
            )
        })
    }) {
        Some(result) => result,
        None => error_result(),
    }
}
fn dispatch_address() -> Result<u64, RuntimeError> {
    ncl_sys::function_address!(dispatch).map_err(|error| RuntimeError::Native(error.to_string()))
}
fn undefined_function_dispatch_address() -> Result<u64, RuntimeError> {
    ncl_sys::function_address!(undefined_function_dispatch)
        .map_err(|error| RuntimeError::Native(error.to_string()))
}
/// Publish the generic builtin trampoline and install it on `object` as the
/// default `ENTRY` for every builtin with no ISA-specific fast path.
///
/// The returned [`CodePtr`] must be retained for the runtime's lifetime: it
/// owns the executable mapping every such builtin's function object jumps
/// into.
///
/// # Errors
/// Returns a native error if assembling, publishing, or installing the
/// trampoline fails.
pub fn install(
    object: &ObjectRuntime,
    ctx: &mut ThreadContext,
) -> Result<(CodePtr, CodePtr), RuntimeError> {
    let make_rest_list = ncl_sys::function_address!(make_rest_list_native)
        .map_err(|error| RuntimeError::Native(error.to_string()))?;
    object.register_builtin_address(
        BuiltinIdentifier::new(
            BuiltinPackage::CommonLisp,
            BuiltinName::new("make-rest-list"),
        ),
        usize::try_from(make_rest_list)
            .map_err(|_| RuntimeError::Native("make-rest-list address overflow".to_owned()))?,
    );
    let builtin_address = dispatch_address()?;
    install_keyword_builtins(object)?;
    let undefined_address = undefined_function_dispatch_address()?;
    let build = |address| {
        if cfg!(target_arch = "aarch64") {
            boundary_stub::build_aarch64_stub(address)
        } else {
            boundary_stub::build_x86_64_stub(address)
        }
    };
    let bytes = build(builtin_address)?;
    let undefined_bytes = build(undefined_address)?;
    let mut code =
        alloc_code(bytes.len()).map_err(|error| RuntimeError::Native(format!("{error:?}")))?;
    write_code(&mut code, 0, &bytes).map_err(|error| RuntimeError::Native(format!("{error:?}")))?;
    publish_code(&mut code).map_err(|error| RuntimeError::Native(format!("{error:?}")))?;
    object.install_generic_builtin_entry(ctx, code.address())?;
    let mut undefined_code = alloc_code(undefined_bytes.len())
        .map_err(|error| RuntimeError::Native(format!("{error:?}")))?;
    write_code(&mut undefined_code, 0, &undefined_bytes)
        .map_err(|error| RuntimeError::Native(format!("{error:?}")))?;
    publish_code(&mut undefined_code)
        .map_err(|error| RuntimeError::Native(format!("{error:?}")))?;
    Ok((code, undefined_code))
}

#[cfg(test)]
#[path = "builtin_trampoline_test.rs"]
mod tests;
