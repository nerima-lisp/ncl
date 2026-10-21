#[cfg(test)]
mod runtime_tests {
    use super::super::{Runtime, RuntimeError};
    use ncl_object::{
        FunctionArguments, FunctionCaller, MultipleValues, ObjectError, Package,
        symbol_is_constant, symbol_value,
    };
    use std::fs;

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
        assert!(matches!(
            result,
            Err(RuntimeError::Object(ObjectError::ControlError))
        ));
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
    fn jit_storefield_keeps_a_young_value_in_a_weak_only_old_cell() {
        let mut runtime =
            Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
        runtime.set_gc_stress(false);
        runtime.set_strict_forwarding(true);

        let pair_symbol = rooted_user_symbol(&mut runtime, "*U03-STOREFIELD-PAIR*");
        let pair_recovery_symbol = rooted_user_symbol(&mut runtime, "*U03-STOREFIELD-RECOVERED*");
        let pair_weak_symbol = rooted_user_symbol(&mut runtime, "*U03-STOREFIELD-WEAK-PAIR*");
        let cell_weak_symbol = rooted_user_symbol(&mut runtime, "*U03-STOREFIELD-WEAK-CELL*");
        prepare_old_storefield_cell(
            &mut runtime,
            &pair_symbol,
            &pair_weak_symbol,
            &cell_weak_symbol,
        );

        let young = runtime
            .eval("(cons 701 702)")
            .unwrap_or_else(|error| panic!("JIT argument cons creation failed: {error}"));
        let pair = symbol_value(&runtime.context, pair_symbol.get()).unwrap_or_else(|error| {
            panic!("captured cell pair lookup before call failed: {error}")
        });
        let setter = storefield_setter(&runtime, pair, "captured");
        let setter = ncl_object::FunctionObject::try_from(setter)
            .unwrap_or_else(|error| panic!("captured setter conversion failed: {error}"));
        let arguments = [young];
        let mut values = MultipleValues::new();
        let mut caller = runtime.function_caller();
        // Exercise FunctionCaller after eval returns to its prior native context.
        let jit_store = caller
            .call_function(
                &mut runtime.context,
                &runtime.object,
                ncl_object::typed::FunctionDesignator::Function(setter),
                FunctionArguments::new(&arguments),
                &mut values,
            )
            .map_err(|error| error.to_string());
        assert_eq!(jit_store, Ok(ncl_sys::Word::fixnum(701)));
        ncl_object::set_symbol_value(&mut runtime.context, pair_symbol.get(), ncl_sys::Word::NIL)
            .unwrap_or_else(|error| panic!("captured cell pair unrooting failed: {error}"));
        runtime
            .eval("NIL")
            .unwrap_or_else(|error| panic!("clearing the setter return value failed: {error}"));
        collect_minor_twice(&mut runtime, "post-store");

        let weak_cell = weak_target(&runtime, cell_weak_symbol.get())
            .unwrap_or_else(|error| panic!("weak cell lookup failed: {error}"));
        let pair = restore_weak_target(
            &mut runtime,
            pair_weak_symbol.get(),
            pair_recovery_symbol.get(),
        )
        .unwrap_or_else(|error| panic!("weak pair recovery failed: {error}"));
        let captured_cell = storefield_setter_cell(&runtime, pair, "recovered");
        assert_eq!(
            captured_cell, weak_cell,
            "weak link must name the StoreField owner"
        );
        let jit_read = read_recovered_cell_car(&mut runtime, "*U03-STOREFIELD-RECOVERED*");

        let api_read = read_after_barriered_api_store(
            &mut runtime,
            weak_cell,
            &pair_weak_symbol,
            &pair_recovery_symbol,
        )
        .unwrap_or_else(|error| panic!("positive-control failed: {error}"));

        assert_eq!(api_read, ncl_sys::Word::fixnum(801));
        assert_eq!(jit_read, Ok(ncl_sys::Word::fixnum(701)));
    }

    fn prepare_old_storefield_cell(
        runtime: &mut Runtime,
        pair_symbol: &ncl_object::HeapRootedWord,
        pair_weak_symbol: &ncl_object::HeapRootedWord,
        cell_weak_symbol: &ncl_object::HeapRootedWord,
    ) {
        let pair = runtime
            .eval(
                "(let ((cell nil)) \
                   (list (lambda () cell) \
                         (lambda (value) (progn (setq cell value) (car value)))))",
            )
            .unwrap_or_else(|error| panic!("captured cell pair creation failed: {error}"));
        ncl_object::set_symbol_value(&mut runtime.context, pair_symbol.get(), pair)
            .unwrap_or_else(|error| panic!("captured cell pair rooting failed: {error}"));
        runtime
            .eval("NIL")
            .unwrap_or_else(|error| panic!("clearing the pair return value failed: {error}"));
        collect_minor_twice(runtime, "promotion");

        let pair = symbol_value(&runtime.context, pair_symbol.get())
            .unwrap_or_else(|error| panic!("captured cell pair lookup failed: {error}"));
        let cell = storefield_setter_cell(runtime, pair, "captured");
        set_weak_link(runtime, pair_weak_symbol.get(), pair);
        set_weak_link(runtime, cell_weak_symbol.get(), cell);
    }

    fn storefield_setter_cell(
        runtime: &Runtime,
        pair: ncl_sys::Word,
        phase: &str,
    ) -> ncl_sys::Word {
        let setter = storefield_setter(runtime, pair, phase);
        ncl_object::closure_ref(&runtime.context, ncl_object::Function::from_word(setter), 0)
            .unwrap_or_else(|error| panic!("{phase} value cell lookup failed: {error}"))
    }

    fn storefield_setter(runtime: &Runtime, pair: ncl_sys::Word, phase: &str) -> ncl_sys::Word {
        let setter_list = ncl_object::cdr(&runtime.context, pair)
            .unwrap_or_else(|error| panic!("{phase} setter list lookup failed: {error}"));
        ncl_object::car(&runtime.context, setter_list)
            .unwrap_or_else(|error| panic!("{phase} setter lookup failed: {error}"))
    }

    fn collect_minor_twice(runtime: &mut Runtime, phase: &str) {
        for collection in [1, 2] {
            runtime.context.collect(false).unwrap_or_else(|error| {
                panic!("{phase} minor collection {collection} failed: {error}")
            });
        }
    }

    fn read_after_barriered_api_store(
        runtime: &mut Runtime,
        weak_cell: ncl_sys::Word,
        pair_weak_symbol: &ncl_object::HeapRootedWord,
        pair_recovery_symbol: &ncl_object::HeapRootedWord,
    ) -> Result<ncl_sys::Word, String> {
        let api_value = runtime
            .eval("(cons 801 802)")
            .map_err(|error| format!("positive-control cons creation failed: {error}"))?;
        runtime
            .context
            .write_object_slot(weak_cell, 0, api_value)
            .map_err(|error| format!("positive-control cell store failed: {error}"))?;
        runtime
            .eval("NIL")
            .map_err(|error| format!("clearing the positive-control value failed: {error}"))?;
        ncl_object::set_symbol_value(
            &mut runtime.context,
            pair_recovery_symbol.get(),
            ncl_sys::Word::NIL,
        )
        .map_err(|error| format!("recovered pair unrooting failed: {error}"))?;
        collect_minor_twice(runtime, "positive-control");
        restore_weak_target(runtime, pair_weak_symbol.get(), pair_recovery_symbol.get())
            .map_err(|error| format!("positive-control pair recovery failed: {error}"))?;
        read_recovered_cell_car(runtime, "*U03-STOREFIELD-RECOVERED*")
    }

    fn intern_user_symbol(runtime: &mut Runtime, name: &str) -> ncl_sys::Word {
        let package = runtime
            .object
            .find_package(&runtime.context, "COMMON-LISP-USER")
            .unwrap_or_else(|| panic!("COMMON-LISP-USER package missing"));
        Package::from_word(package)
            .intern(&mut runtime.context, &runtime.object, name)
            .unwrap_or_else(|error| panic!("symbol {name} interning failed: {error}"))
            .0
    }

    fn rooted_user_symbol(runtime: &mut Runtime, name: &str) -> ncl_object::HeapRootedWord {
        let symbol = intern_user_symbol(runtime, name);
        ncl_object::HeapRootedWord::new(&runtime.context, symbol)
            .unwrap_or_else(|error| panic!("symbol {name} rooting failed: {error}"))
    }

    fn set_weak_link(runtime: &mut Runtime, symbol: ncl_sys::Word, owner: ncl_sys::Word) {
        let link = ncl_object::make_cons(
            &mut runtime.context,
            &runtime.object,
            ncl_sys::Word::NIL,
            owner,
        )
        .unwrap_or_else(|error| panic!("weak link allocation failed: {error}"));
        let weak = runtime.context.make_weak(link, ncl_sys::Weakness::Value);
        ncl_object::set_symbol_value(&mut runtime.context, symbol, weak)
            .unwrap_or_else(|error| panic!("weak link rooting failed: {error}"));
    }

    fn weak_target(runtime: &Runtime, symbol: ncl_sys::Word) -> Result<ncl_sys::Word, String> {
        let link = symbol_value(&runtime.context, symbol).map_err(|error| error.to_string())?;
        let target = runtime.context.weak_value(link);
        (target != ncl_sys::Word::NIL)
            .then_some(target)
            .ok_or_else(|| "weak target was cleared".to_owned())
    }

    fn restore_weak_target(
        runtime: &mut Runtime,
        weak_symbol: ncl_sys::Word,
        recovery_symbol: ncl_sys::Word,
    ) -> Result<ncl_sys::Word, String> {
        let target = weak_target(runtime, weak_symbol)?;
        ncl_object::set_symbol_value(&mut runtime.context, recovery_symbol, target)
            .map_err(|error| error.to_string())?;
        Ok(target)
    }

    fn read_recovered_cell_car(
        runtime: &mut Runtime,
        recovery_symbol: &str,
    ) -> Result<ncl_sys::Word, String> {
        let value = runtime
            .eval(&format!("(funcall (car {recovery_symbol}))"))
            .map_err(|error| error.to_string())?;
        ncl_object::car(&runtime.context, value).map_err(|error| error.to_string())
    }
}
