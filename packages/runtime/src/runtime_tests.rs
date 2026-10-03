#[cfg(test)]
mod runtime_tests {
    use super::super::{Runtime, RuntimeError};
    use std::fs;

    #[test]
    fn evaluates_a_literal_through_native_code() {
        let mut runtime = match Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => panic!("runtime initialization failed: {error}"),
        };
        let value = match runtime.eval("42") {
            Ok(value) => value,
            Err(error) => panic!("native evaluation failed: {error}"),
        };
        assert_eq!(runtime.format_result(value), "42");
        drop(runtime);
    }

    #[test]
    fn compile_and_load_share_the_native_pipeline() {
        let mut runtime = match Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => panic!("runtime initialization failed: {error}"),
        };
        let compiled = match runtime.compile("41") {
            Ok(value) => value,
            Err(error) => panic!("compile failed: {error}"),
        };
        let loaded = match runtime.load("42") {
            Ok(value) => value,
            Err(error) => panic!("load failed: {error}"),
        };
        assert_eq!(runtime.format_result(compiled), "41");
        assert_eq!(runtime.format_result(loaded), "42");
    }

    #[test]
    fn load_file_executes_source_and_reports_missing_files() {
        let path = std::env::temp_dir().join(format!("ncl-runtime-{}.lisp", std::process::id()));
        if let Err(error) = fs::write(&path, "43") {
            panic!("source file creation failed: {error}");
        }
        let mut runtime = match Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => panic!("runtime initialization failed: {error}"),
        };
        let value = match runtime.load_file(&path) {
            Ok(value) => value,
            Err(error) => panic!("load_file failed: {error}"),
        };
        let compiled = match runtime.compile_file(&path) {
            Ok(value) => value,
            Err(error) => panic!("compile_file failed: {error}"),
        };
        let fasl_path = path.with_extension("fasl");
        assert!(
            fasl_path.is_file(),
            "compile_file did not create FASL output"
        );
        assert_eq!(runtime.format_result(value), "43");
        assert_eq!(runtime.format_result(compiled), "43");
        let missing = runtime.load_file(path.with_extension("missing"));
        assert!(matches!(missing, Err(RuntimeError::Io { .. })));
        if let Err(error) = fs::remove_file(path) {
            panic!("source file cleanup failed: {error}");
        }
        if let Err(error) = fs::remove_file(fasl_path) {
            panic!("FASL file cleanup failed: {error}");
        }
    }

    #[test]
    fn load_file_evaluates_multiple_forms_in_order() {
        let path = std::env::temp_dir().join(format!(
            "ncl-runtime-multiple-forms-{}.lisp",
            std::process::id()
        ));
        if let Err(error) = fs::write(&path, b"41 42") {
            panic!("source file creation failed: {error}");
        }
        let mut runtime = match Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => panic!("runtime initialization failed: {error}"),
        };
        let value = match runtime.load_file(&path) {
            Ok(value) => value,
            Err(error) => panic!("load_file failed: {error}"),
        };
        assert_eq!(runtime.format_result(value), "42");
        if let Err(error) = fs::remove_file(path) {
            panic!("source file cleanup failed: {error}");
        }
    }

    #[test]
    fn load_file_executes_compiled_fasl_without_changing_source() {
        let path =
            std::env::temp_dir().join(format!("ncl-runtime-fasl-{}.lisp", std::process::id()));
        let source = b"44";
        if let Err(error) = fs::write(&path, source) {
            panic!("source file creation failed: {error}");
        }
        let mut runtime = match Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => panic!("runtime initialization failed: {error}"),
        };
        if let Err(error) = runtime.compile_file(&path) {
            panic!("compile_file failed: {error}");
        }
        let loaded = match runtime.load_file(path.with_extension("fasl")) {
            Ok(value) => value,
            Err(error) => panic!("FASL load failed: {error}"),
        };
        let preserved = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) => panic!("source file read failed: {error}"),
        };
        assert_eq!(preserved, source);
        assert_eq!(runtime.format_result(loaded), "44");
        if let Err(error) = fs::remove_file(&path) {
            panic!("source file cleanup failed: {error}");
        }
        if let Err(error) = fs::remove_file(path.with_extension("fasl")) {
            panic!("FASL file cleanup failed: {error}");
        }
    }

    #[test]
    fn load_file_rejects_malformed_and_hash_mismatched_fasl() {
        let path = std::env::temp_dir().join(format!(
            "ncl-runtime-invalid-fasl-{}.lisp",
            std::process::id()
        ));
        if let Err(error) = fs::write(&path, b"45") {
            panic!("source file creation failed: {error}");
        }
        let fasl_path = path.with_extension("fasl");
        if let Err(error) = fs::write(&fasl_path, b"NCLFASL\0") {
            panic!("malformed FASL creation failed: {error}");
        }
        let mut runtime = match Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => panic!("runtime initialization failed: {error}"),
        };
        assert!(runtime.load_file(&fasl_path).is_err());
        if let Err(error) = runtime.compile_file(&path) {
            panic!("compile_file failed: {error}");
        }
        let mut bytes = match fs::read(&fasl_path) {
            Ok(bytes) => bytes,
            Err(error) => panic!("FASL read failed: {error}"),
        };
        let Some(last) = bytes.last_mut() else {
            panic!("compiled FASL was empty");
        };
        *last ^= 1;
        if let Err(error) = fs::write(&fasl_path, bytes) {
            panic!("corrupt FASL write failed: {error}");
        }
        assert!(runtime.load_file(&fasl_path).is_err());
        if let Err(error) = fs::remove_file(path) {
            panic!("source file cleanup failed: {error}");
        }
        if let Err(error) = fs::remove_file(fasl_path) {
            panic!("FASL file cleanup failed: {error}");
        }
    }

    #[test]
    fn runtimes_can_be_created_and_dropped_repeatedly() {
        for _ in 0..3 {
            let mut runtime = match Runtime::new() {
                Ok(runtime) => runtime,
                Err(error) => panic!("runtime initialization failed: {error}"),
            };
            let value = match runtime.eval("7") {
                Ok(value) => value,
                Err(error) => panic!("native evaluation failed: {error}"),
            };
            assert_eq!(runtime.format_result(value), "7");
        }
    }

    #[test]
    fn evaluates_fast_arithmetic_entries_and_comparisons() {
        let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
        for (source, expected) in [("(+ 40 2)", "42"), ("(- 44 2)", "42"), ("(* 6 7)", "42")] {
            let value = runtime
                .eval(source)
                .unwrap_or_else(|error| panic!("{source} failed: {error}"));
            assert_eq!(runtime.format_result(value), expected, "source: {source}");
        }
        for (source, expected) in [("(< 1 2)", "T"), ("(< 2 1)", "NIL")] {
            let value = runtime
                .eval(source)
                .unwrap_or_else(|error| panic!("{source} failed: {error}"));
            assert_eq!(runtime.format_result(value), expected, "source: {source}");
        }
    }

    #[test]
    fn eval_when_selects_execute_and_compile_toplevel_situations() {
        let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
        let executed = runtime
            .eval("(eval-when (:execute) 41 42)")
            .unwrap_or_else(|error| panic!("execute eval-when failed: {error}"));
        assert_eq!(runtime.format_result(executed), "42");

        let skipped = runtime
            .eval("(eval-when (:compile-toplevel) 42)")
            .unwrap_or_else(|error| panic!("skipped eval-when failed: {error}"));
        assert_eq!(runtime.format_result(skipped), "NIL");
    }

    #[test]
    fn compile_file_selects_compile_toplevel_and_rejects_bad_eval_when() {
        let path =
            std::env::temp_dir().join(format!("ncl-runtime-eval-when-{}.lisp", std::process::id()));
        if let Err(error) = fs::write(&path, "(eval-when (:compile-toplevel) 42)") {
            panic!("source file creation failed: {error}");
        }
        let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
        let value = runtime
            .compile_file(&path)
            .unwrap_or_else(|error| panic!("compile_file failed: {error}"));
        assert_eq!(runtime.format_result(value), "42");

        let malformed = runtime.eval("(eval-when (:unknown) 42)");
        assert!(matches!(malformed, Err(RuntimeError::Front(_))));
        if let Err(error) = fs::remove_file(path) {
            panic!("source cleanup failed: {error}");
        }
    }
}
