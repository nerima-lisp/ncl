#![allow(missing_docs)]

use std::process::{Command, Output, Stdio};

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
