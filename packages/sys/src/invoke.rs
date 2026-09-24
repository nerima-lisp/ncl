use crate::{CodePtr, Thread};

/// Invoke published native code using the NCL `AArch64` entry convention.
#[cfg(target_arch = "aarch64")]
pub fn invoke_entry(
    code: &CodePtr,
    entry_offset: usize,
    ctx: *mut Thread,
    argc: u64,
    arguments: [u64; 4],
    rest: u64,
) -> (u64, u64) {
    invoke_entry_with_function(code, entry_offset, ctx, 0, argc, arguments, rest)
}

/// Invoke published native code with an explicit callee function object in `x16`.
#[cfg(target_arch = "aarch64")]
pub fn invoke_entry_with_function(
    code: &CodePtr,
    entry_offset: usize,
    ctx: *mut Thread,
    function_object: u64,
    argc: u64,
    arguments: [u64; 4],
    rest: u64,
) -> (u64, u64) {
    let entry = code.address().saturating_add(entry_offset);
    let mut value = 0_u64;
    let mut count = 0_u64;
    // SAFETY: the caller supplies published code generated for AArch64, a live
    // Thread, and an entry offset within that allocation.
    unsafe {
        core::arch::asm!(
            "str x21, [sp, #-16]!",
            "blr x17",
            "ldr x21, [sp], #16",
            in("x16") function_object,
            in("x21") ctx,
            in("x17") entry,
            in("x0") argc,
            in("x1") arguments[0],
            in("x2") arguments[1],
            in("x3") arguments[2],
            in("x4") arguments[3],
            in("x5") rest,
            lateout("x0") value,
            lateout("x1") count,
            clobber_abi("C"),
        );
    }
    (value, count)
}

/// Invoke published native code using the NCL x86-64 `SysV` entry convention.
#[cfg(target_arch = "x86_64")]
pub fn invoke_entry(
    code: &CodePtr,
    entry_offset: usize,
    ctx: *mut Thread,
    argc: u64,
    arguments: [u64; 4],
    rest: u64,
) -> (u64, u64) {
    invoke_entry_with_function(code, entry_offset, ctx, 0, argc, arguments, rest)
}

/// Invoke published native code with an explicit callee function object in `r10`.
///
/// Generated code reads the context from `r15` and may clobber it, so the shim
/// saves and restores `r15` around the call.
#[cfg(target_arch = "x86_64")]
pub fn invoke_entry_with_function(
    code: &CodePtr,
    entry_offset: usize,
    ctx: *mut Thread,
    function_object: u64,
    argc: u64,
    arguments: [u64; 4],
    rest: u64,
) -> (u64, u64) {
    let entry = code.address().saturating_add(entry_offset);
    let mut value = 0_u64;
    let mut count = 0_u64;
    // SAFETY: the caller supplies published code generated for x86-64 SysV, a live
    // Thread, and an entry offset within that allocation. The 32-byte stack slot
    // keeps the call 16-byte aligned, leaves the generated frame header's
    // 16-byte reservation untouched, and preserves the caller's `r15`.
    unsafe {
        core::arch::asm!(
            "sub rsp, 32",
            "mov [rsp + 16], r15",
            "call r11",
            "mov r15, [rsp + 16]",
            "add rsp, 32",
            in("r10") function_object,
            in("r11") entry,
            in("r15") ctx,
            in("rdi") argc,
            in("rsi") arguments[0],
            in("rdx") arguments[1],
            in("rcx") arguments[2],
            in("r8") arguments[3],
            in("r9") rest,
            lateout("rax") value,
            lateout("rdx") count,
            clobber_abi("C"),
        );
    }
    (value, count)
}

/// Invoke published native code using the NCL entry convention.
///
/// This target has no native transition yet, so the call is a placeholder.
#[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
pub const fn invoke_entry(
    _code: &CodePtr,
    _entry_offset: usize,
    _ctx: *mut Thread,
    _argc: u64,
    _arguments: [u64; 4],
    _rest: u64,
) -> (u64, u64) {
    (0, 0)
}

/// Invoke published native code with an explicit callee function object.
///
/// This target has no native transition yet, so the call is a placeholder.
#[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
pub const fn invoke_entry_with_function(
    _code: &CodePtr,
    _entry_offset: usize,
    _ctx: *mut Thread,
    _function_object: u64,
    _argc: u64,
    _arguments: [u64; 4],
    _rest: u64,
) -> (u64, u64) {
    (0, 0)
}
