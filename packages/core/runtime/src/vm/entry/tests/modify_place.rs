use super::{Instruction, Program, Rc, Runtime, RuntimeError, function, run_entry};

fn raw_modify_result(runtime: &Runtime, source: &str) -> String {
    raw_modify_execute(runtime, source)
        .unwrap_or_else(|error| panic!("raw bytecode should execute: {error:?}"))
        .primary_value()
        .to_string()
}

fn raw_modify_execute(runtime: &Runtime, source: &str) -> Result<crate::Value, RuntimeError> {
    let forms =
        ncl_syntax::read(source).unwrap_or_else(|error| panic!("source should parse: {error:?}"));
    assert_eq!(forms.len(), 1);
    let form = forms
        .first()
        .unwrap_or_else(|| panic!("expected one form, got none"));
    let program = Rc::new(
        ncl_compiler::Compiler::compile_form(form)
            .unwrap_or_else(|error| panic!("source should compile: {error:?}")),
    );
    run_entry(
        runtime,
        &program,
        program.entry,
        &runtime.global_environment(),
        form.span,
    )
}

#[test]
fn raw_modify_place_default_and_multiple_value_delta() {
    for (operation, default, multiple) in [("incf", "11", "12"), ("decf", "9", "8")] {
        for (delta, expected) in [("", default), ("(values 2 99)", multiple)] {
            let runtime = Runtime::new();
            assert_eq!(
                raw_modify_result(
                    &runtime,
                    &format!("(let ((xs (list 10))) ({operation} (car xs) {delta}))")
                ),
                expected
            );
        }
    }
}

#[test]
fn raw_modify_place_rejects_invalid_delta_id_before_expansion() {
    let runtime = Runtime::new();
    runtime
        .eval_source(
            "(defparameter *raw-expanded* 0)
         (define-setf-expander raw-place ()
           (incf *raw-expanded*)
           (values nil nil '(new) 'new '10))",
        )
        .unwrap_or_else(|error| panic!("place setup should evaluate: {error:?}"));
    let invocation = ncl_syntax::read("(incf (raw-place))")
        .unwrap_or_else(|error| panic!("source should parse: {error:?}"))
        .remove(0);
    let span = invocation.span;
    let program = Rc::new(Program {
        entry: 0,
        functions: vec![function(vec![Instruction::ModifyPlace {
            invocation,
            delta: 1,
            arithmetic: "+".to_string(),
        }])],
    });
    assert!(
        matches!(run_entry(&runtime, &program, 0, &runtime.global_environment(), span),
        Err(RuntimeError::InvalidForm { message, .. })
            if message == "invalid modifying delta function")
    );
    assert_eq!(raw_modify_result(&runtime, "*raw-expanded*"), "0");
}

#[test]
fn raw_modify_place_evaluates_index_once() {
    for (operation, expected) in [("incf", "(11 (11 20) 1)"), ("decf", "(9 (9 20) 1)")] {
        let runtime = Runtime::new();
        let source = format!(
            "(let ((xs (list 10 20)) (calls 0))
               (list ({operation} (nth (progn (incf calls) 0) xs) 1) xs calls))"
        );
        assert_eq!(raw_modify_result(&runtime, &source), expected);
    }
}

#[test]
fn raw_modify_place_reads_after_delta() {
    for (operation, expected) in [("incf", "(21 21)"), ("decf", "(19 19)")] {
        let runtime = Runtime::new();
        runtime
            .eval_source(
                "(defparameter *raw-cell* 10)
             (defun raw-place () *raw-cell*)
             (define-setf-expander raw-place ()
               (values nil nil '(new-value)
                 '(setq *raw-cell* new-value) '*raw-cell*))",
            )
            .unwrap_or_else(|error| panic!("place setup should evaluate: {error:?}"));
        let source =
            format!("(list ({operation} (raw-place) (progn (setq *raw-cell* 20) 1)) *raw-cell*)");
        assert_eq!(raw_modify_result(&runtime, &source), expected);
    }
}

#[test]
fn raw_modify_place_delta_exit_does_not_store() {
    for operation in ["incf", "decf"] {
        let runtime = Runtime::new();
        let source = format!(
            "(let ((xs (list 10)) (calls 0))
               (list (block done
                 ({operation} (nth (progn (incf calls) 0) xs) (return-from done 77)))
                 xs calls))"
        );
        assert_eq!(raw_modify_result(&runtime, &source), "(77 (10) 1)");
    }
}

#[test]
fn raw_modify_place_delta_uses_caller_lexical_bindings() {
    let runtime = Runtime::new();
    runtime
        .eval_source(
            "(defparameter *raw-cell* 10)
         (define-setf-expander raw-place ()
           (values '(ncl-setf-temp-0) '(100) '(ncl-setf-temp-1)
             '(setq *raw-cell* ncl-setf-temp-1) '*raw-cell*))",
        )
        .unwrap_or_else(|error| panic!("place setup should evaluate: {error:?}"));
    assert_eq!(
        raw_modify_result(
            &runtime,
            "(let ((ncl-setf-temp-0 2) (ncl-setf-temp-1 3))
           (list (incf (raw-place) (+ ncl-setf-temp-0 ncl-setf-temp-1))
             ncl-setf-temp-0 ncl-setf-temp-1))"
        ),
        "(15 2 3)"
    );
}

#[test]
fn raw_modify_place_restores_dynamic_bindings_on_every_exit() {
    for delta in ["1", "(return-from done 77)", "(/ 1 0)"] {
        let runtime = Runtime::new();
        runtime
            .eval_source(
                "(defparameter *raw-temp* 9)
             (defparameter *raw-cell* 10)
             (define-setf-expander raw-place ()
               (values '(*raw-temp*) '(100) '(new-value)
                 '(setq *raw-cell* new-value) '*raw-cell*))",
            )
            .unwrap_or_else(|error| panic!("place setup should evaluate: {error:?}"));
        let depth = runtime.dynamic_depth();
        let exact_depth = runtime.exact_dynamic_depth();
        let result = raw_modify_execute(
            &runtime,
            &format!("(block done (incf (raw-place) {delta}))"),
        );
        if delta == "(/ 1 0)" {
            assert!(result.is_err());
        } else {
            assert_eq!(
                result
                    .unwrap_or_else(|error| panic!("normal or block exit: {error:?}"))
                    .to_string(),
                if delta == "1" { "11" } else { "77" }
            );
        }
        assert_eq!(runtime.dynamic_depth(), depth);
        assert_eq!(runtime.exact_dynamic_depth(), exact_depth);
        assert_eq!(raw_modify_result(&runtime, "*raw-temp*"), "9");
        assert_eq!(
            raw_modify_result(&runtime, "*raw-cell*"),
            if delta == "1" { "11" } else { "10" }
        );
    }
}

#[test]
fn raw_modify_place_rejects_malformed_expansion_before_delta() {
    for expansion in [
        "(values '(tmp) nil '(new) 'new '10)",
        "(values '(42) '(1) '(new) 'new '10)",
        "(values nil nil '(42) '42 '10)",
    ] {
        let runtime = Runtime::new();
        runtime
            .eval_source(&format!(
                "(defparameter *raw-delta-ran* 0)
             (define-setf-expander raw-place () {expansion})"
            ))
            .unwrap_or_else(|error| panic!("place setup should evaluate: {error:?}"));
        assert!(
            raw_modify_execute(&runtime, "(incf (raw-place) (setq *raw-delta-ran* 1))").is_err()
        );
        assert_eq!(raw_modify_result(&runtime, "*raw-delta-ran*"), "0");
    }
}
