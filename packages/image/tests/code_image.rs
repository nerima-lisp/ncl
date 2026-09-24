// The lint allow and crate doc must precede the architecture gate: on other
// targets the `#![cfg]` empties the crate and would strip them with it.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests assert on code-space failures"
)]

//! Acceptance test: a published code block executes after an image load.

#![cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]

use ncl_image::{CodeImage, load, save};
use ncl_object::{Runtime, ThreadContext};
use ncl_sys::{alloc_code, invoke_entry, publish_code, write_code};

/// The constant returned by the fixture program.
const EXPECTED: u64 = 42;

#[test]
fn published_code_executes_after_a_load() {
    let program = constant_program();
    let mut code = alloc_code(program.len()).unwrap();
    write_code(&mut code, 0, &program).unwrap();
    publish_code(&mut code).unwrap();
    let image = CodeImage::capture(&code, 0, 0, "constant").unwrap();

    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let bytes = save(&runtime, &mut ctx, &[], &[image]).unwrap();

    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let loaded = load(&bytes, &runtime, &mut ctx).unwrap();
    assert_eq!(loaded.code.len(), 1);
    assert_eq!(loaded.code[0].entry(), loaded.code[0].address());

    let (value, count) = invoke_entry(&loaded.code[0], 0, ctx.thread_mut(), 0, [0; 4], 0);
    assert_eq!(value, EXPECTED);
    assert_eq!(count, 1);
}

/// Machine code that returns [`EXPECTED`] as one value.
#[cfg(target_arch = "aarch64")]
fn constant_program() -> Vec<u8> {
    // mov x0, #42 ; mov x1, #1 ; ret
    let instructions = [0xD280_0540_u32, 0xD280_0021, 0xD65F_03C0];
    instructions
        .iter()
        .flat_map(|instruction| instruction.to_le_bytes())
        .collect()
}

/// Machine code that returns [`EXPECTED`] as one value.
#[cfg(target_arch = "x86_64")]
fn constant_program() -> Vec<u8> {
    // mov rax, 42 ; mov rdx, 1 ; ret
    let mut bytes = vec![0x48, 0xB8];
    bytes.extend_from_slice(&EXPECTED.to_le_bytes());
    bytes.extend_from_slice(&[0x48, 0xC7, 0xC2, 0x01, 0x00, 0x00, 0x00, 0xC3]);
    bytes
}
