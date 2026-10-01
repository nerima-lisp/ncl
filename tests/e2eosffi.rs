#![allow(missing_docs)]

use std::process::{Command, Output, Stdio};

fn run(source: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", source])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap_or_else(|error| panic!("{source}: {error}"))
}

#[test]
fn compiled_os_and_ffi_packages_are_visible() {
    let source = "(and (find-package \"NCL-OS\") (find-symbol \"CALL-FOREIGN\" \"NCL-FFI\"))";
    let output = run(source);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty(), "{:?}", output.stderr);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "NCL-FFI:CALL-FOREIGN\n"
    );
}

#[test]
fn compiled_os_primitives_return_values() {
    let source = "(and (stringp (ncl-os::current-directory)) (integerp (ncl-os::get-time)) (integerp (ncl-os::random-u64)))";
    let output = run(source);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty(), "{:?}", output.stderr);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "T\n");
}
