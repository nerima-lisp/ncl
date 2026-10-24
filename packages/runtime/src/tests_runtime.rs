use super::{Runtime, RuntimeError};
use ncl_object::{
    FunctionArguments, FunctionCaller, MultipleValues, ObjectError, Package, symbol_is_constant,
    symbol_value,
};
use std::fs;

#[cfg(test)]
mod tests_write_barrier;

fn failing_lisp_error_converter(
    context: &mut ncl_object::ThreadContext,
    _runtime: &ncl_object::Runtime,
    _error: ncl_object::LispError,
) -> Result<ncl_object::Word, ObjectError> {
    context.set_pending(ObjectError::ControlError);
    Err(ObjectError::Layout)
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
fn deftype_expands_through_type_queries_and_declarations() {
    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
    runtime
        .eval("(deftype r6-b1-range () '(integer 0 10))")
        .unwrap_or_else(|error| panic!("DEFTYPE definition failed: {error}"));

    let in_range = runtime
        .eval("(typep 5 'r6-b1-range)")
        .unwrap_or_else(|error| panic!("TYPEP with DEFTYPE failed: {error}"));
    assert_eq!(runtime.format_result(in_range), "T");
    let outside_range = runtime
        .eval("(typep 11 '(or r6-b1-range string))")
        .unwrap_or_else(|error| panic!("nested TYPEP with DEFTYPE failed: {error}"));
    assert_eq!(runtime.format_result(outside_range), "NIL");

    let subtype = runtime
        .eval("(subtypep 'r6-b1-range 'integer)")
        .unwrap_or_else(|error| panic!("SUBTYPEP with DEFTYPE failed: {error}"));
    assert_eq!(runtime.format_result(subtype), "T");

    let checked = runtime
        .eval("(let ((r6-b1-value 5)) (check-type r6-b1-value r6-b1-range) r6-b1-value)")
        .unwrap_or_else(|error| panic!("CHECK-TYPE with DEFTYPE failed: {error}"));
    assert_eq!(runtime.format_result(checked), "5");

    runtime
        .eval("(defun r6-b1-declared (value) (declare (type r6-b1-range value)) value)")
        .unwrap_or_else(|error| panic!("DECLARE with DEFTYPE failed: {error}"));
    let declared = runtime
        .eval("(r6-b1-declared 5)")
        .unwrap_or_else(|error| panic!("declared function call failed: {error}"));
    assert_eq!(runtime.format_result(declared), "5");

    runtime
        .eval("(defparameter *r6-b1-deftype-expanded-in-declare* nil)")
        .unwrap_or_else(|error| panic!("DEFTYPE declaration marker setup failed: {error}"));
    runtime
        .eval("(deftype r6-b1-declared-type () (setq *r6-b1-deftype-expanded-in-declare* t) 'integer)")
        .unwrap_or_else(|error| panic!("DEFTYPE declaration marker definition failed: {error}"));
    runtime
        .eval("(defun r6-b1-declared-expansion (value) (declare (type r6-b1-declared-type value)) value)")
        .unwrap_or_else(|error| panic!("DECLARE did not expand the DEFTYPE: {error}"));
    let declaration_expanded = runtime
        .eval("*r6-b1-deftype-expanded-in-declare*")
        .unwrap_or_else(|error| panic!("DEFTYPE declaration marker read failed: {error}"));
    assert_eq!(runtime.format_result(declaration_expanded), "T");

    runtime
        .eval("(deftype r6-b1-optional (&optional lower) `(integer ,lower *))")
        .unwrap_or_else(|error| panic!("optional DEFTYPE definition failed: {error}"));
    let defaulted = runtime
        .eval("(typep -1 'r6-b1-optional)")
        .unwrap_or_else(|error| panic!("DEFTYPE default argument failed: {error}"));
    assert_eq!(runtime.format_result(defaulted), "T");
    let supplied = runtime
        .eval("(typep -1 '(r6-b1-optional 0))")
        .unwrap_or_else(|error| panic!("DEFTYPE supplied argument failed: {error}"));
    assert_eq!(runtime.format_result(supplied), "NIL");

    runtime
        .eval("(deftype r6-b1-optional-init (&optional (lower 0)) `(integer ,lower *))")
        .unwrap_or_else(|error| panic!("optional-init DEFTYPE definition failed: {error}"));
    let optional_init = runtime
        .eval("(typep -1 'r6-b1-optional-init)")
        .unwrap_or_else(|error| panic!("DEFTYPE optional init form failed: {error}"));
    assert_eq!(runtime.format_result(optional_init), "NIL");

    runtime
        .eval("(deftype r6-b1-key (&key (lower 0)) `(integer ,lower *))")
        .unwrap_or_else(|error| panic!("key DEFTYPE definition failed: {error}"));
    let key_default = runtime
        .eval("(typep 5 'r6-b1-key)")
        .unwrap_or_else(|error| panic!("DEFTYPE key default failed: {error}"));
    assert_eq!(runtime.format_result(key_default), "T");
    let key_supplied = runtime
        .eval("(typep 1 '(r6-b1-key :lower 2))")
        .unwrap_or_else(|error| panic!("DEFTYPE key argument failed: {error}"));
    assert_eq!(runtime.format_result(key_supplied), "NIL");

    runtime
        .eval("(deftype r6-b1-rest (&rest bounds) `(integer ,(first bounds) ,(second bounds)))")
        .unwrap_or_else(|error| panic!("rest DEFTYPE definition failed: {error}"));
    let rest_arguments = runtime
        .eval("(typep 5 '(r6-b1-rest 0 10))")
        .unwrap_or_else(|error| panic!("DEFTYPE rest arguments failed: {error}"));
    assert_eq!(runtime.format_result(rest_arguments), "T");
}

#[test]
fn internal_time_functions_and_constant_are_registered() {
    let mut runtime = match Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => panic!("runtime initialization failed: {error}"),
    };

    let units = match runtime.eval("INTERNAL-TIME-UNITS-PER-SECOND") {
        Ok(value) => value,
        Err(error) => panic!("internal time constant evaluation failed: {error}"),
    };
    assert_eq!(units.as_fixnum(), Some(1_000_000));

    let real_time = match runtime.eval("(GET-INTERNAL-REAL-TIME)") {
        Ok(value) => value,
        Err(error) => panic!("GET-INTERNAL-REAL-TIME failed: {error}"),
    };
    assert!(real_time.as_fixnum().is_some_and(|ticks| ticks >= 0));

    let run_ticks = match runtime.eval("(GET-INTERNAL-RUN-TIME)") {
        Ok(value) => value,
        Err(error) => panic!("GET-INTERNAL-RUN-TIME failed: {error}"),
    };
    assert!(run_ticks.as_fixnum().is_some_and(|ticks| ticks >= 0));

    let common_lisp = runtime
        .object
        .find_package(&runtime.context, "COMMON-LISP")
        .unwrap_or_else(|| panic!("COMMON-LISP package missing"));
    let (constant, _) = Package::from_word(common_lisp)
        .intern(
            &mut runtime.context,
            &runtime.object,
            "INTERNAL-TIME-UNITS-PER-SECOND",
        )
        .unwrap_or_else(|error| panic!("constant symbol lookup failed: {error}"));
    assert!(
        symbol_is_constant(&runtime.context, constant)
            .unwrap_or_else(|error| panic!("constant flag lookup failed: {error}"))
    );
    assert_eq!(
        symbol_value(&runtime.context, constant)
            .unwrap_or_else(|error| panic!("constant value lookup failed: {error}")),
        units
    );
}

#[test]
fn escaping_throw_runs_cleanup_and_restores_progv_bindings() {
    let mut runtime = match Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => panic!("runtime initialization failed: {error}"),
    };
    runtime
        .eval("(defparameter *r2-progv-value* :outer)")
        .unwrap_or_else(|error| panic!("outer binding setup failed: {error}"));
    runtime
        .eval("(defparameter *r2-cleanup-value* :not-run)")
        .unwrap_or_else(|error| panic!("cleanup setup failed: {error}"));

    let result = runtime.eval(
        "(progv '(*r2-progv-value*) '(:inner) \
               (unwind-protect \
                 (throw 'r2-unmatched-tag :payload) \
                 (setq *r2-cleanup-value* *r2-progv-value*)))",
    );
    assert!(
        matches!(result, Err(RuntimeError::Object(ObjectError::ControlError))),
        "unexpected result: {result:?}"
    );
    let _ = runtime.context.take_non_local_exit();

    let cleanup_ran = runtime
        .eval("(eq *r2-cleanup-value* :inner)")
        .unwrap_or_else(|error| panic!("cleanup observation failed: {error}"));
    assert_eq!(runtime.format_result(cleanup_ran), "T");
    let binding_restored = runtime
        .eval("(eq *r2-progv-value* :outer)")
        .unwrap_or_else(|error| panic!("progv restoration check failed: {error}"));
    assert_eq!(runtime.format_result(binding_restored), "T");
    let thread = runtime.context.thread_access();
    assert_eq!(thread.cleanup_depth(), 0);
    assert_eq!(thread.handler_depth(), 0);
}

#[test]
fn failed_condition_conversion_does_not_poison_the_next_compiled_eval() {
    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
    runtime
        .object
        .register_lisp_error_converter(failing_lisp_error_converter);

    assert!(matches!(
        runtime.eval("(car)"),
        Err(RuntimeError::Object(ObjectError::Layout))
    ));
    assert!(!runtime.context.thread_access().runtime_error_pending());

    let value = runtime
        .eval("42")
        .unwrap_or_else(|error| panic!("follow-up evaluation failed: {error}"));
    assert_eq!(runtime.format_result(value), "42");
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
    let path = std::env::temp_dir().join(format!("ncl-runtime-fasl-{}.lisp", std::process::id()));
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
fn jit_storefield_keeps_a_young_value_in_a_weak_only_old_cell() {
    tests_write_barrier::run();
}
