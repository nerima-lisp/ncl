#![allow(missing_docs)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn ncl() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ncl"))
}

fn path_str(path: &Path) -> &str {
    path.to_str().unwrap_or("<non-UTF-8 temp path>")
}

fn temp_source(name: &str, source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("ncl-{name}-{}.lisp", std::process::id()));
    fs::write(&path, source).unwrap_or_else(|error| panic!("source creation failed: {error}"));
    path
}

#[test]
fn execute_situation_runs_and_compile_only_does_not() {
    let execute = ncl()
        .args(["--eval", "(eval-when (:execute) 2)"])
        .output()
        .unwrap_or_else(|error| panic!("ncl failed to start: {error}"));
    assert!(execute.status.success());
    assert_eq!(String::from_utf8_lossy(&execute.stdout).trim(), "2");

    let path = temp_source("evalwhen-load", "(eval-when (:compile-toplevel) (print 1))");
    let loaded = ncl()
        .args(["--load", path_str(&path)])
        .output()
        .unwrap_or_else(|error| panic!("ncl failed to start: {error}"));
    assert!(loaded.status.success());
    assert!(!String::from_utf8_lossy(&loaded.stdout).contains('1'));
    fs::remove_file(path).unwrap_or_else(|error| panic!("source cleanup failed: {error}"));
}

#[test]
fn load_toplevel_situation_runs() {
    let path = temp_source(
        "evalwhen-load-toplevel",
        "(eval-when (:load-toplevel :execute) 3)",
    );
    let loaded = ncl()
        .args(["--load", path_str(&path)])
        .output()
        .unwrap_or_else(|error| panic!("ncl failed to start: {error}"));
    assert!(loaded.status.success());
    assert_eq!(String::from_utf8_lossy(&loaded.stdout).trim(), "3");
    fs::remove_file(path).unwrap_or_else(|error| panic!("source cleanup failed: {error}"));
}

#[test]
fn compile_file_runs_compile_toplevel_and_later_macro() {
    let path = temp_source(
        "evalwhen-compile",
        "(eval-when (:compile-toplevel :load-toplevel :execute) (defmacro m () 1)) (m)",
    );
    let compiled = ncl()
        .args(["--compile-file", path_str(&path)])
        .output()
        .unwrap_or_else(|error| panic!("ncl failed to start: {error}"));
    assert!(compiled.status.success());
    assert_eq!(String::from_utf8_lossy(&compiled.stdout).trim(), "1");
    let fasl = path.with_extension("fasl");
    fs::remove_file(path).unwrap_or_else(|error| panic!("source cleanup failed: {error}"));
    fs::remove_file(fasl).unwrap_or_else(|error| panic!("fasl cleanup failed: {error}"));
}

#[test]
fn compile_file_evaluates_compile_toplevel_side_effects() {
    let path = temp_source(
        "evalwhen-side-effect",
        "(eval-when (:compile-toplevel) (setq *x* 1)) *x*",
    );
    let compiled = ncl()
        .args(["--compile-file", path_str(&path)])
        .output()
        .unwrap_or_else(|error| panic!("ncl failed to start: {error}"));
    assert!(compiled.status.success());
    assert_eq!(String::from_utf8_lossy(&compiled.stdout).trim(), "1");
    let fasl = path.with_extension("fasl");
    fs::remove_file(path).unwrap_or_else(|error| panic!("source cleanup failed: {error}"));
    fs::remove_file(fasl).unwrap_or_else(|error| panic!("fasl cleanup failed: {error}"));
}

#[test]
fn invalid_situation_is_rejected() {
    let result = ncl()
        .args(["--eval", "(eval-when (:not-a-situation) 1)"])
        .output()
        .unwrap_or_else(|error| panic!("ncl failed to start: {error}"));
    assert!(!result.status.success());
}
