#![allow(missing_docs)]

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn ncl() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ncl"))
}

fn path_str(path: &Path) -> &str {
    path.to_str()
        .unwrap_or_else(|| panic!("non-UTF-8 temp path"))
}

fn output(command: &mut Command) -> Output {
    match command.output() {
        Ok(output) => output,
        Err(error) => panic!("failed to run ncl: {error}"),
    }
}

#[test]
fn eval_load_script_and_repl_use_runtime() {
    let eval = output(ncl().args(["--eval", "42"]));
    assert!(eval.status.success());
    assert_eq!(String::from_utf8_lossy(&eval.stdout).trim(), "42");

    let path = std::env::temp_dir().join(format!("ncl-cli-{}.lisp", std::process::id()));
    if let Err(error) = fs::write(&path, "43") {
        panic!("source file creation failed: {error}");
    }

    let load = output(ncl().args(["--load", path_str(&path)]));
    assert!(load.status.success());
    assert_eq!(String::from_utf8_lossy(&load.stdout).trim(), "43");

    let script = output(ncl().args(["--script", path_str(&path)]));
    assert!(script.status.success());
    assert!(script.stdout.is_empty());

    let repl = match ncl().stdin(Stdio::piped()).stdout(Stdio::piped()).spawn() {
        Ok(repl) => repl,
        Err(error) => panic!("failed to start ncl REPL: {error}"),
    };
    let mut repl = repl;
    let Some(mut stdin) = repl.stdin.take() else {
        panic!("REPL stdin unavailable");
    };
    if let Err(error) = stdin.write_all(b"44\n") {
        panic!("failed to write REPL input: {error}");
    }
    drop(stdin);
    let output = match repl.wait_with_output() {
        Ok(output) => output,
        Err(error) => panic!("failed to collect REPL output: {error}"),
    };
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("44"));

    if let Err(error) = fs::remove_file(path) {
        panic!("source file cleanup failed: {error}");
    }
}
