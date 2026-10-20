use core::arch::asm;
use core::ffi::c_void;

/// Invoke a function using the System V x86-64 integer/pointer register convention.
///
/// # Safety
/// `function` must be callable with this ABI and these six register values.
pub(super) unsafe fn invoke(function: *mut c_void, arguments: [u64; 6]) -> u64 {
    let [
        argument_0,
        argument_1,
        argument_2,
        argument_3,
        argument_4,
        argument_5,
    ] = arguments;
    let result: u64;
    // SAFETY: the caller validated a System V integer/pointer signature with at most
    // six register arguments. The stack is realigned for the nested call and restored.
    unsafe {
        asm!(
            "mov r10, rsp",
            "and rsp, -16",
            "sub rsp, 144",
            "mov [rsp], r10",
            "call r11",
            "mov rsp, [rsp]",
            inlateout("r11") function => _,
            inlateout("rdi") argument_0 => _,
            inlateout("rsi") argument_1 => _,
            inlateout("rdx") argument_2 => _,
            inlateout("rcx") argument_3 => _,
            inlateout("r8") argument_4 => _,
            inlateout("r9") argument_5 => _,
            lateout("rax") result,
            clobber_abi("C"),
        );
    }
    result
}
