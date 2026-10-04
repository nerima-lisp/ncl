#[cfg(test)]
mod runtime_tests {
    use super::super::{Runtime, RuntimeError};
    use std::fs;

    #[test]
    fn pprint_logical_block_runs_a_fill_style_traversal() {
        let mut runtime = match Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => panic!("runtime initialization failed: {error}"),
        };
        let source = r#"
            (progn
              (setf common-lisp::*print-length* 2)
              (with-output-to-string (s)
                (pprint-logical-block (s '(1 2 3) :prefix "(" :suffix ")")
                  (pprint-exit-if-list-exhausted)
                  (prin1 (pprint-pop) s)
                  (pprint-newline :fill)
                  (pprint-exit-if-list-exhausted)
                  (write-string " " s)
                  (prin1 (pprint-pop) s)
                  (pprint-exit-if-list-exhausted)
                  (write-string " " s)
                  (prin1 (pprint-pop) s))))
        "#;
        let value = match runtime.eval(source) {
            Ok(value) => value,
            Err(error) => panic!("pprint logical block evaluation failed: {error}"),
        };
        assert_eq!(runtime.format_result(value), "\"(1 2)\"");
    }

    #[test]
    fn pprint_logical_block_exposes_a_dotted_tail_once() {
        let mut runtime = match Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => panic!("runtime initialization failed: {error}"),
        };
        let value = match runtime.eval(
            r#"(with-output-to-string (s)
                  (pprint-logical-block (s '(1 . 2) :prefix "(" :suffix ")")
                    (prin1 (pprint-pop) s)
                    (pprint-exit-if-list-exhausted)
                    (write-string " . " s)
                    (prin1 (pprint-pop) s)))"#,
        ) {
            Ok(value) => value,
            Err(error) => panic!("dotted logical block evaluation failed: {error}"),
        };
        assert_eq!(runtime.format_result(value), "\"(1 . 2)\"");
    }

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
}
