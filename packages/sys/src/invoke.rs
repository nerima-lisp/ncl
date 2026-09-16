use crate::{CodePtr, Thread};

/// Invoke published native code using the NCL `AArch64` entry convention.
///
/// On other targets this is a documented placeholder until that target's
/// native transition is implemented.
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

/// Invoke published native code using the NCL entry convention.
#[cfg(not(target_arch = "aarch64"))]
pub fn invoke_entry(
    _code: &CodePtr,
    _entry_offset: usize,
    _ctx: *mut Thread,
    _argc: u64,
    _arguments: [u64; 4],
    _rest: u64,
) -> (u64, u64) {
    (0, 0)
}

#[cfg(not(target_arch = "aarch64"))]
pub fn invoke_entry_with_function(
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
