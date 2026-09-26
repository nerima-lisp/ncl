#![allow(missing_docs)]

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const CHILD_TIMEOUT: Duration = Duration::from_secs(30);

fn ncl() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ncl"))
}

fn path_str(path: &Path) -> &str {
    path.to_str()
        .unwrap_or_else(|| panic!("non-UTF-8 temp path"))
}

fn output(command: &mut Command) -> Output {
    let child = match command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => panic!("failed to run ncl: {error}"),
    };
    wait_with_timeout(child)
}

fn wait_with_timeout(mut child: Child) -> Output {
    let deadline = Instant::now() + CHILD_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return match child.wait_with_output() {
                    Ok(output) => output,
                    Err(error) => panic!("failed to collect ncl output: {error}"),
                };
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                if let Err(error) = child.kill() {
                    panic!("ncl timed out and could not be killed: {error}");
                }
                let _ = child.wait();
                panic!("ncl did not exit within {CHILD_TIMEOUT:?}");
            }
            Err(error) => panic!("failed to poll ncl: {error}"),
        }
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
    let output = wait_with_timeout(repl);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("44"));

    if let Err(error) = fs::remove_file(path) {
        panic!("source file cleanup failed: {error}");
    }
}

#[test]
fn evals_core_forms_through_native_builtin_entries() {
    for (source, expected) in [
        ("(+ 1 2)", "3"),
        ("(* 6 7)", "42"),
        ("(car (cons 1 2))", "1"),
    ] {
        let result = output(ncl().args(["--eval", source]));
        assert!(result.status.success(), "{source}: {result:?}");
        assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), expected);
    }
}

#[test]
fn evals_native_functions_constants_and_closures() {
    for (source, expected) in [
        ("(progn (defun f (x) (+ x 1)) (f 41))", "42"),
        ("(funcall (let ((y 5)) (lambda (x) (+ x y))) 10)", "15"),
    ] {
        let result = output(ncl().args(["--eval", source]));
        assert!(result.status.success(), "{source}: {result:?}");
        assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), expected);
    }
}
