#![allow(missing_docs)]

#[cfg(target_arch = "x86_64")]
mod x86run {
    use std::fs;
    use std::process::Command;

    fn eval(source: &str) -> String {
        let output = Command::new(env!("CARGO_BIN_EXE_ncl"))
            .args(["--eval", source])
            .output()
            .unwrap_or_else(|error| panic!("failed to run ncl: {error}"));
        assert!(output.status.success(), "{source}: {output:?}");
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    #[test]
    fn x86_64_native_execution_handles_calls_closures_quote_and_load() {
        assert_eq!(eval("(+ 1 2)"), "3");
        assert_eq!(
            eval("(progn (defun fib (n) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2))))) (fib 25))"),
            "75025"
        );
        assert_eq!(
            eval("(funcall (let ((y 5)) (lambda (x) (+ x y))) 10)"),
            "15"
        );
        assert_eq!(eval("'(1 2 . 3)"), "(1 2 . 3)");

        let path = std::env::temp_dir().join(format!("ncl-x86run-{}.lisp", std::process::id()));
        fs::write(&path, "43\n")
            .unwrap_or_else(|error| panic!("source file creation failed: {error}"));
        let Some(path) = path.to_str() else {
            panic!("non-UTF-8 temp path");
        };
        assert_eq!(eval(&format!("(load \"{path}\")")), "43");
        let _ = fs::remove_file(path);
    }
}
