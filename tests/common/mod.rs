#![allow(missing_docs)]

use std::process::{Command, Output, Stdio};

#[allow(dead_code)]
pub fn run_ncl(source: &str) -> Output {
    match Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", source])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    {
        Ok(output) => output,
        Err(error) => panic!("failed to run ncl: {error}"),
    }
}

#[allow(dead_code)]
pub fn assert_eval_with_stress(source: &str, expected: &str) {
    let mut runtime = ncl_runtime::Runtime::new()
        .unwrap_or_else(|error| panic!("runtime initialization failed: {error:?}"));
    assert_eval_with_stress_on(&mut runtime, source, expected);
}

pub fn assert_eval_with_stress_on(
    runtime: &mut ncl_runtime::Runtime,
    source: &str,
    expected: &str,
) {
    runtime.set_gc_stress(true);
    runtime.set_strict_forwarding(true);
    let value = runtime
        .compile(source)
        .unwrap_or_else(|error| panic!("compile failed for {source}: {error:?}"));
    assert_eq!(runtime.format_result(value), expected, "{source}");
}
