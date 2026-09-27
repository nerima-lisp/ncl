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
    CellError, FunctionObject, LispError, Runtime as ObjectRuntime, ThreadContext, Word,
};
use ncl_sys::{CodePtr, Thread, alloc_code, publish_code, write_code};
use std::ptr::NonNull;

use crate::RuntimeError;
use crate::support::NativeInvocation;

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
    _rest: u64,
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
        dispatch_with_context(invocation, argc, [a0, a1, a2, a3], function_object)
    }) {
        Some(result) => result,
        None => error_result(),
    }
}

fn dispatch_with_context(
    invocation: &mut NativeInvocation<'_>,
    argc: u64,
    registers: [u64; 4],
    function_object: u64,
) -> NativeCallResult {
    let context: &mut ThreadContext = invocation.context;
    let object: &ObjectRuntime = invocation.object;
    let Some(count) = Word::from_bits(argc)
        .as_fixnum()
        .and_then(|value| usize::try_from(value).ok())
        .filter(|count| *count <= registers.len())
    else {
        context.set_pending(ncl_object::ObjectError::Layout);
        return error_result();
    };
    let Ok(function) = FunctionObject::try_from(Word::from_bits(function_object)) else {
        context.set_pending(ncl_object::ObjectError::Unbound);
        return error_result();
    };
    let call_words: Vec<Word> = registers
        .iter()
        .take(count)
        .copied()
        .map(Word::from_bits)
        .collect();
    match object.call_builtin(context, function, &call_words) {
        Ok(value) => ok_result(value, context.values().len()),
        Err(error) => {
            context.set_pending(error);
            error_result()
        }
    }
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
    let builtin_address = dispatch_address()?;
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
