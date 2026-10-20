use core::arch::asm;
use core::ffi::c_void;

/// Invoke a function using the System V x86-64 scalar calling convention.
///
/// # Safety
/// `function` must follow the System V AMD64 ABI and match the supplied scalar and
/// pointer arguments and result type.
pub(super) unsafe fn invoke(
    function: *mut c_void,
    general: &[u64; 8],
    floating: &[u64; 8],
    stack: &[u64],
    stack_area_bytes: usize,
) -> (u64, u64) {
    let integer_result: u64;
    let float_result: f64;
    // SAFETY: the caller classifies each scalar into the System V GP, SSE, or
    // eightbyte stack area. The stack is aligned before the nested call, and r12
    // retains the saved stack position because the callee preserves it.
    unsafe {
        asm!(
            "push r12",
            "mov r12, rsp",
            "and rsp, -16",
            "sub rsp, r9",
            "mov rsi, r8",
            "mov rdi, rsp",
            "rep movsq",
            "movq xmm0, [rdx]",
            "movq xmm1, [rdx + 8]",
            "movq xmm2, [rdx + 16]",
            "movq xmm3, [rdx + 24]",
            "movq xmm4, [rdx + 32]",
            "movq xmm5, [rdx + 40]",
            "movq xmm6, [rdx + 48]",
            "movq xmm7, [rdx + 56]",
            "mov rdi, [r10]",
            "mov rsi, [r10 + 8]",
            "mov rdx, [r10 + 16]",
            "mov rcx, [r10 + 24]",
            "mov r8, [r10 + 32]",
            "mov r9, [r10 + 40]",
            "call r11",
            "mov rsp, r12",
            "pop r12",
            in("r11") function,
            in("r10") general.as_ptr(),
            in("rdx") floating.as_ptr(),
            in("r8") stack.as_ptr(),
            in("r9") stack_area_bytes,
            in("rcx") stack.len(),
            lateout("rax") integer_result,
            lateout("xmm0") float_result,
            clobber_abi("C"),
        );
    }
    (integer_result, float_result.to_bits())
}
