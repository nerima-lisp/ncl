use core::arch::asm;
use core::ffi::c_void;

/// Invoke a function using the AAPCS64 scalar calling convention.
///
/// # Safety
/// `function` must follow AAPCS64 and match the supplied scalar and pointer arguments
/// and result type.
pub(super) unsafe fn invoke(
    function: *mut c_void,
    general: &[u64; 8],
    floating: &[u64; 8],
    stack: &[u64],
    stack_area_bytes: usize,
) -> (u64, u64) {
    let integer_result: u64;
    let float_result: f64;
    // SAFETY: the caller classifies each scalar into the AAPCS64 GP, SIMD/FP, or
    // eightbyte stack area. The callee preserves x19, and the outgoing stack area
    // remains 16-byte aligned.
    unsafe {
        asm!(
            "str x19, [sp, #-16]!",
            "mov x19, sp",
            "sub sp, sp, x11",
            "mov x14, #0",
            "2:",
            "cmp x14, x12",
            "b.hs 3f",
            "ldr x15, [x10, x14, lsl #3]",
            "str x15, [sp, x14, lsl #3]",
            "add x14, x14, #1",
            "b 2b",
            "3:",
            "ldr d0, [x9]",
            "ldr d1, [x9, #8]",
            "ldr d2, [x9, #16]",
            "ldr d3, [x9, #24]",
            "ldr d4, [x9, #32]",
            "ldr d5, [x9, #40]",
            "ldr d6, [x9, #48]",
            "ldr d7, [x9, #56]",
            "ldr x0, [x17]",
            "ldr x1, [x17, #8]",
            "ldr x2, [x17, #16]",
            "ldr x3, [x17, #24]",
            "ldr x4, [x17, #32]",
            "ldr x5, [x17, #40]",
            "ldr x6, [x17, #48]",
            "ldr x7, [x17, #56]",
            "blr x16",
            "mov sp, x19",
            "ldr x19, [sp]",
            "add sp, sp, #16",
            in("x16") function,
            in("x17") general.as_ptr(),
            in("x9") floating.as_ptr(),
            in("x10") stack.as_ptr(),
            in("x11") stack_area_bytes,
            in("x12") stack.len(),
            lateout("x0") integer_result,
            lateout("v0") float_result,
            clobber_abi("C"),
        );
    }
    (integer_result, float_result.to_bits())
}
