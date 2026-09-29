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
//! This module publishes one tiny hand-assembled stub per ISA that re-presents
//! the native call using the ordinary C calling convention (moving the two
//! pinned registers into the two argument slots the Lisp convention leaves
//! free) and tail-calls [`dispatch`], a plain `extern "C"` Rust function. The
//! stub itself carries no logic beyond that register/stack shuffle; it exists
//! only because the two pinned registers sit outside the integer argument
//! registers an `extern "C" fn` can name directly. [`dispatch`] recovers the
//! live `Runtime`/`ThreadContext` pair through the same native-context side
//! channel `native_make_closure` already uses (see
//! `crate::support::NativeInvocation`) and forwards the call to
//! `ncl_object::Runtime::call_builtin`, which already performs arity
//! checking, `&key`/`&rest` adaptation, and pending-condition conversion.
//! Every line in this module compiles under ordinary safe Rust; the thread
//! pointer is only ever dereferenced inside `ncl_sys::with_native_context`,
//! in the one crate whose lint configuration permits that.
use ncl_asm_aarch64::{Assembler as Aarch64Assembler, Inst as Aarch64Inst, Reg as Aarch64Reg};
use ncl_asm_x86_64::{Assembler as X86Assembler, BinOp, Imm, Inst as X86Inst, Mem, Reg as X86Reg};
use ncl_object::{
    BuiltinIdentifier, BuiltinName, BuiltinPackage, CellError, FunctionObject, LispError, Package,
    Runtime as ObjectRuntime, ThreadContext, Word, symbol_function,
};
use ncl_sys::{CodePtr, Thread, alloc_code, publish_code, write_code};
use std::ptr::NonNull;

use crate::RuntimeError;
use crate::support::NativeInvocation;
#[path = "builtin_trampoline_dispatch.rs"]
mod dispatch_impl;
use dispatch_impl::{dispatch_with_context, record_boundary_error};
#[path = "builtin_trampoline_keywords.rs"]
mod keyword_impl;
use keyword_impl::install_keyword_builtins;
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
fn ok_result(value: Word, value_count: usize) -> NativeCallResult {
    NativeCallResult {
        value: value.bits(),
        count: u64::try_from(value_count).unwrap_or(1).max(1),
    }
}
/// Registers available to the generic trampoline stub, in the order the
/// shared `extern "C"` dispatcher receives them. `argc`/`a0..a3`/`rest` mirror
/// the native Lisp calling convention's own argument-count/argument-list
/// registers; `function_object`/`thread_ptr` are the two pinned registers the
/// ISA stub moves into ordinary argument slots before tail-calling this
/// function.
#[allow(clippy::similar_names, clippy::too_many_arguments)]
extern "C" fn dispatch(
    argc: u64,
    a0: u64,
    a1: u64,
    a2: u64,
    a3: u64,
    rest: u64,
    function_object: u64,
    thread_ptr: u64,
) -> NativeCallResult {
    let Ok(address) = usize::try_from(thread_ptr) else {
        return error_result();
    };
    let Some(thread) = NonNull::new(std::ptr::without_provenance_mut::<Thread>(address)) else {
        return error_result();
    };
    // `unwrap_or_else` is banned project-wide by `check_added_lines.py`
    // (its one exemption is the `PoisonError::into_inner` idiom), so this
    // stays a `match` rather than the `Option::map_or_else`/`unwrap_or_else`
    // form clippy would otherwise prefer.
    #[allow(clippy::option_if_let_else)]
    match ncl_sys::with_native_context(thread, |invocation: &mut NativeInvocation<'_>| {
        dispatch_with_context(invocation, argc, [a0, a1, a2, a3], rest, function_object)
    }) {
        Some(result) => result,
        None => error_result(),
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
                let result = FunctionObject::try_from(rooted_function)
                    .and_then(|function| object.call_builtin(context, function, &arguments));
                let function_popped = ncl_object::pop_heap_root(object, function_token);
                if !function_popped {
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
            let arguments_popped = argument_tokens
                .into_iter()
                .rev()
                .all(|token| ncl_object::pop_heap_root(object, token));
            if !arguments_popped {
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
                let result = FunctionObject::try_from(rooted_function)
                    .and_then(|function| object.call_builtin(context, function, &arguments));
                let function_popped = ncl_object::pop_heap_root(object, function_token);
                if !function_popped {
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
            let arguments_popped = argument_tokens
                .into_iter()
                .rev()
                .all(|token| ncl_object::pop_heap_root(object, token));
            if !arguments_popped {
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
) -> NativeCallResult {
    let Ok(address) = usize::try_from(thread_ptr) else {
        return error_result();
    };
    let Some(thread) = NonNull::new(std::ptr::without_provenance_mut::<Thread>(address)) else {
        return error_result();
    };
    #[allow(clippy::option_if_let_else)]
    match ncl_sys::with_native_context(thread, |invocation: &mut NativeInvocation<'_>| {
        invocation
            .context
            .set_pending_lisp_error(LispError::CellError(CellError::UndefinedFunction {
                name: Word::from_bits(symbol),
            }));
        invocation
            .context
            .set_pending(ncl_object::ObjectError::UndefinedFunction);
        error_result()
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
/// `mov x6, x16` / `mov x7, x21` followed by an absolute branch into
/// [`dispatch`]. `x6`/`x7` are the two integer argument registers `AArch64`'s
/// `Call`/`CallClosure` lowering never writes (it only uses `x0..x5`), so
/// moving the pinned function-object (`x16`) and thread-context (`x21`)
/// registers there lets `dispatch`'s eight named parameters land exactly on
/// `x0..x7` per the standard `AArch64` C calling convention.
fn build_aarch64_stub(address: u64) -> Result<Vec<u8>, RuntimeError> {
    let mut assembler = Aarch64Assembler::new();
    let emit = |assembler: &mut Aarch64Assembler, inst: Aarch64Inst| {
        assembler
            .emit(&inst)
            .map_err(|error| RuntimeError::Native(error.to_string()))
    };
    emit(
        &mut assembler,
        Aarch64Inst::Mov {
            rd: ncl_asm_aarch64::RegOrSp::Reg(Aarch64Reg(6)),
            rn: ncl_asm_aarch64::RegOrSp::Reg(Aarch64Reg(16)),
        },
    )?;
    emit(
        &mut assembler,
        Aarch64Inst::Mov {
            rd: ncl_asm_aarch64::RegOrSp::Reg(Aarch64Reg(7)),
            rn: ncl_asm_aarch64::RegOrSp::Reg(Aarch64Reg(21)),
        },
    )?;
    for instruction in ncl_asm_aarch64::mov_imm64(Aarch64Reg(9), address) {
        emit(&mut assembler, instruction)?;
    }
    emit(&mut assembler, Aarch64Inst::Br { rn: Aarch64Reg(9) })?;
    assembler
        .finish()
        .map(|blob| blob.bytes)
        .map_err(|error| RuntimeError::Native(error.to_string()))
}
/// Reserves two stack argument slots for `dispatch`'s seventh and eighth
/// parameters (the pinned function-object/thread-context registers `r10`/
/// `r15`, which sit outside the six `SysV` integer argument registers), stores
/// them, calls [`dispatch`], releases the reservation, and returns. x86-64's
/// `Call`/`CallClosure` lowering (`emit_call`) already reserves 16 bytes below
/// `rsp` before its own indirect call, so `rsp` is `8 mod 16` on stub entry;
/// subtracting 24 (also `8 mod 16`) restores the required `0 mod 16` alignment
/// at the nested `call`.
fn build_x86_64_stub(address: u64) -> Result<Vec<u8>, RuntimeError> {
    const RESERVED: i32 = 24;
    let address = i64::try_from(address).map_err(|_| {
        RuntimeError::Native("generic builtin dispatcher address does not fit in i64".to_owned())
    })?;
    let mut assembler = X86Assembler::new();
    let emit = |assembler: &mut X86Assembler, inst: X86Inst| {
        assembler
            .emit(&inst)
            .map_err(|error| RuntimeError::Native(error.to_string()))
    };
    emit(
        &mut assembler,
        X86Inst::BinRI(BinOp::Sub, X86Reg::Rsp, RESERVED),
    )?;
    emit(
        &mut assembler,
        X86Inst::MovMR(Mem::base(X86Reg::Rsp, 0), X86Reg::R10),
    )?;
    emit(
        &mut assembler,
        X86Inst::MovMR(Mem::base(X86Reg::Rsp, 8), X86Reg::R15),
    )?;
    emit(
        &mut assembler,
        X86Inst::MovRI(X86Reg::Rax, Imm::I64(address)),
    )?;
    emit(&mut assembler, X86Inst::CallReg(X86Reg::Rax))?;
    emit(
        &mut assembler,
        X86Inst::BinRI(BinOp::Add, X86Reg::Rsp, RESERVED),
    )?;
    emit(&mut assembler, X86Inst::Ret)?;
    assembler
        .finish()
        .map(|blob| blob.bytes)
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
            build_aarch64_stub(address)
        } else {
            build_x86_64_stub(address)
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
