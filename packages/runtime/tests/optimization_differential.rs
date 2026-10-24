#![allow(missing_docs)]

use ncl_runtime::{OptimizationMode, Runtime, RuntimeError, RuntimeOptions};
use ncl_sys::Word;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

type Outcome = Result<String, String>;

#[derive(Clone, Copy)]
enum EntryPoint {
    Eval,
    Compile,
}

fn runtime(mode: OptimizationMode, stress: bool) -> Runtime {
    let mut runtime = Runtime::with_options(RuntimeOptions::new(mode, false))
        .unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
    if stress {
        runtime.set_gc_stress(true);
        runtime.set_strict_forwarding(true);
    }
    runtime
}

fn format_outcome(runtime: &mut Runtime, result: Result<Word, RuntimeError>) -> Outcome {
    result
        .map(|value| runtime.format_result(value))
        .map_err(|error| error.to_string())
}

fn run_form(mode: OptimizationMode, entry: EntryPoint, source: &str, stress: bool) -> Outcome {
    let mut runtime = runtime(mode, stress);
    let result = match entry {
        EntryPoint::Eval => runtime.eval(source),
        EntryPoint::Compile => runtime.compile(source),
    };
    format_outcome(&mut runtime, result)
}

fn run_scalar_inlining_gvn(mode: OptimizationMode) -> Outcome {
    let mut runtime = runtime(mode, false);
    runtime.set_strict_forwarding(true);
    if let Err(error) = runtime.compile(
        "(progn (defun ncl-opt-diff-scalar-leaf (x) x) (defun ncl-opt-diff-scalar-use () (+ (ncl-opt-diff-scalar-leaf 18) (ncl-opt-diff-scalar-leaf 18))))",
    ) {
        return Err(error.to_string());
    }
    runtime.set_gc_stress(true);
    let result =
        runtime.compile("(let ((keep (cons 29 30))) (+ (ncl-opt-diff-scalar-use) (car keep)))");
    format_outcome(&mut runtime, result)
}

fn run_file(mode: OptimizationMode, path: &Path, operation: FileOperation) -> Outcome {
    let mut runtime = runtime(mode, false);
    let result = match operation {
        FileOperation::Load => runtime.load_file(path),
        FileOperation::Compile => runtime.compile_file(path),
    };
    format_outcome(&mut runtime, result)
}

#[derive(Clone, Copy)]
enum FileOperation {
    Load,
    Compile,
}

fn source_path() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "ncl-optimization-differential-{}-{}.lisp",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn optimized_and_unoptimized_runtime_entrypoints_match() {
    let cases = [
        ("(* 6 (+ 3 4))", Some("42")),
        (
            "(progn (defun ncl-opt-diff-fib (n) (if (< n 2) n (+ (ncl-opt-diff-fib (- n 1)) (ncl-opt-diff-fib (- n 2))))) (ncl-opt-diff-fib 8))",
            Some("21"),
        ),
        (
            "(let ((x (cons 5 6))) (setf (car x) 12) (car x))",
            Some("12"),
        ),
        (
            "(let ((x (cons 7 9))) (funcall (lambda () (car x))))",
            Some("7"),
        ),
        ("(multiple-value-list (floor 7 2))", Some("(3 1)")),
        (
            "(handler-case (error \"optimization differential\") (error () :caught))",
            Some(":CAUGHT"),
        ),
        (
            "(progn (defgeneric ncl-opt-diff-generic (value)) (defmethod ncl-opt-diff-generic ((value integer)) (+ value 1)) (ncl-opt-diff-generic 41))",
            Some("42"),
        ),
        ("(ncl-opt-diff-unbound-function)", None),
    ];
    for entry in [EntryPoint::Eval, EntryPoint::Compile] {
        for (source, expected) in cases {
            let optimized = run_form(OptimizationMode::Default, entry, source, false);
            let unoptimized = run_form(OptimizationMode::Off, entry, source, false);
            assert_eq!(optimized, unoptimized, "{source}");
            if let Some(expected) = expected {
                assert_eq!(optimized.as_deref(), Ok(expected), "{source}");
            } else {
                assert!(optimized.is_err(), "expected an error for {source}");
            }
        }
    }
}

#[test]
fn optimized_and_unoptimized_file_entrypoints_match() {
    let path = source_path();
    let source = "(let ((x (cons 6 9))) (setf (car x) 13) (car x))";
    std::fs::write(&path, source)
        .unwrap_or_else(|error| panic!("source file creation failed: {error}"));
    let fasl = path.with_extension("fasl");

    for mode in [OptimizationMode::Default, OptimizationMode::Off] {
        assert_eq!(
            run_file(mode, &path, FileOperation::Load),
            Ok("13".to_owned())
        );
        assert_eq!(
            run_file(mode, &path, FileOperation::Compile),
            Ok("13".to_owned())
        );
        assert_eq!(
            run_file(mode, &fasl, FileOperation::Load),
            Ok("13".to_owned())
        );
    }

    std::fs::remove_file(&path).unwrap_or_else(|error| panic!("source cleanup failed: {error}"));
    std::fs::remove_file(fasl).unwrap_or_else(|error| panic!("FASL cleanup failed: {error}"));
}

#[test]
fn optimization_differential_gc_stress_strict_forwarding() {
    let source = "(let ((x nil)) (if t (setq x (cons 29 30)) (setq x nil)) (car x))";
    let optimized = run_form(OptimizationMode::Default, EntryPoint::Compile, source, true);
    let unoptimized = run_form(OptimizationMode::Off, EntryPoint::Compile, source, true);
    assert_eq!(optimized, Ok("29".to_owned()));
    assert_eq!(optimized, unoptimized);
}

#[test]
fn scalar_inlining_gvn_matches_under_gc_stress_and_strict_forwarding() {
    let optimized = run_scalar_inlining_gvn(OptimizationMode::Default);
    let unoptimized = run_scalar_inlining_gvn(OptimizationMode::Off);
    assert_eq!(optimized, Ok("65".to_owned()));
    assert_eq!(optimized, unoptimized);
}
