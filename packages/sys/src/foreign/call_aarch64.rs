use core::arch::asm;
use core::ffi::c_void;

/// Invoke a function using the AAPCS64 integer/pointer register convention.
///
/// # Safety
/// `function` must be callable with this ABI and these eight register values.
pub(super) unsafe fn invoke(function: *mut c_void, arguments: [u64; 8]) -> u64 {
    let [
        argument_0,
        argument_1,
        argument_2,
        argument_3,
        argument_4,
        argument_5,
        argument_6,
        argument_7,
    ] = arguments;
    let result: u64;
    // SAFETY: the caller validated an AArch64 integer/pointer signature with at most
    // eight register arguments. The ABI requires a 16-byte-aligned stack, which this
    // assembly leaves unchanged.
    unsafe {
        asm!(
            "blr x16",
            inlateout("x16") function => _,
            inlateout("x0") argument_0 => result,
            inlateout("x1") argument_1 => _,
            inlateout("x2") argument_2 => _,
            inlateout("x3") argument_3 => _,
            inlateout("x4") argument_4 => _,
            inlateout("x5") argument_5 => _,
            inlateout("x6") argument_6 => _,
            inlateout("x7") argument_7 => _,
            clobber_abi("C"),
        );
    }
    result
}
