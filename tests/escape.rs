//! End-to-end tests for escaped strings and characters in CLI output.

#[path = "common/mod.rs"]
mod common;

#[path = "common/escape_cases.rs"]
mod escape_cases;

use common::{CliEvalCase, assert_ncl_eval_cases, run_ncl};
use escape_cases::CASES;

fn run_runtime(source: &str, compile: bool) -> Result<String, ncl_runtime::RuntimeError> {
    let mut runtime = ncl_runtime::Runtime::new()?;
    let value = if compile {
        runtime.compile(source)?
    } else {
        runtime.eval(source)?
    };
    Ok(runtime.format_result(value))
}

fn escape_case_requires_process(name: &str) -> bool {
    matches!(
        name,
        "mapcar-go"
            | "lambda-return-from-restores-progv"
            | "branch-return-from-restores-special-binding"
            | "lambda-return-from-restores-repeated-progv-binding"
            | "unwind-protect-cleanup"
            | "flet-body-return-from-runs-unwind-cleanup"
            | "labels-body-return-from-runs-unwind-cleanup"
    )
}

#[test]
fn non_local_escape_values_are_observable_through_the_cli() {
    assert_eq!(CASES.len(), 33);
    let batch = CASES
        .iter()
        .filter(|case| !escape_case_requires_process(case.name))
        .map(|case| CliEvalCase::new(case.name, case.source, case.expected))
        .collect::<Vec<_>>();
    assert_eq!(batch.len(), 26);
    assert_ncl_eval_cases(&batch);

    for case in CASES
        .iter()
        .filter(|case| escape_case_requires_process(case.name))
    {
        let output = run_ncl(case.source);
        assert_eq!(output.status.code(), Some(0), "{}: {:?}", case.name, output);
        assert!(
            output.stderr.is_empty(),
            "{}: {:?}",
            case.name,
            output.stderr
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim_end(),
            case.expected,
            "{}",
            case.name
        );
    }
}

#[test]
fn loops_with_tagbody_and_closures_survive_gc_stress() {
    let mut runtime = ncl_runtime::Runtime::new()
        .unwrap_or_else(|error| panic!("runtime initialization failed: {error:?}"));
    runtime.set_gc_stress(true);
    runtime.set_strict_forwarding(true);
    for (source, expected) in [
        ("(let ((s 0)) (dotimes (i 10) (setq s (+ s i))) s)", "45"),
        (
            "(let ((x 0)) (tagbody top (setq x (1+ x)) (when (< x 5) (go top))) x)",
            "5",
        ),
    ] {
        let value = runtime
            .compile(source)
            .unwrap_or_else(|error| panic!("compile failed for {source}: {error:?}"));
        assert_eq!(runtime.format_result(value), expected, "{source}");
    }
}

#[test]
fn expired_closure_reports_control_error_through_the_cli() {
    let cases = [
        "(progn (setq *escape-closure* nil) (block done (setq *escape-closure* (lambda () (return-from done 17)))) (funcall *escape-closure*))",
        "(let ((f nil)) (block b (setq f (lambda () (return-from b 1)))) (funcall f))",
        "(let ((f nil) (n 0)) (block b (setq n 3) (setq f (lambda () (return-from b 1)))) (funcall f))",
        "(let ((f nil) (n 0) (m 0)) (block b (setq n 3) (setq m 4) (setq f (lambda () (return-from b 1)))) (funcall f))",
        "(let ((f nil)) (block b (flet ((make () (lambda () (return-from b 1)))) (setq f (make)))) (funcall f))",
        "(let ((f nil)) (block b (labels ((make () (lambda () (return-from b 1)))) (setq f (make)))) (funcall f))",
        "(let ((f nil) (n 0)) (tagbody (setq n 3) (setq f (lambda () (go done))) done) (funcall f))",
        "(let ((f nil)) (tagbody (setq f (flet ((jump () (go done))) (lambda () (jump)))) done) (funcall f))",
        "(let ((f nil)) (tagbody (setq f (labels ((jump () (go done))) (lambda () (jump)))) done) (funcall f))",
    ];
    for source in cases {
        let output = run_ncl(source);
        assert_eq!(output.status.code(), Some(1), "{source}: {output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("ControlError"),
            "{source}: stderr={:?}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn if_else_return_from_restores_active_regions_for_following_exit() {
    let source = "(progn (defvar *branch-special-after* :root) (list (block done (let ((*branch-special-after* :inner)) (if t :continue (return-from done :else-exit)) (return-from done *branch-special-after*))) *branch-special-after*))";
    let output = run_ncl(source);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty(), "{:?}", output.stderr);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        "(:INNER :ROOT)"
    );
}

#[test]
fn multiple_value_bind_return_from_nil_preserves_multiple_values() {
    let source = "(multiple-value-list (block nil (multiple-value-bind (symbol status) (values 'ncl-b3-symbol :internal) (return (values symbol status)))))";
    let output = run_ncl(source);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty(), "{:?}", output.stderr);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        "(COMMON-LISP-USER:NCL-B3-SYMBOL :INTERNAL)"
    );
}

#[test]
fn multiple_value_bind_return_from_nil_through_lambda_preserves_multiple_values() {
    let source = "(multiple-value-list (block nil (funcall (lambda () (multiple-value-bind (symbol status) (values 'ncl-b3-symbol :internal) (return (values symbol status)))))))";
    let output = run_ncl(source);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty(), "{:?}", output.stderr);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        "(COMMON-LISP-USER:NCL-B3-SYMBOL :INTERNAL)"
    );
}

#[test]
fn nested_closure_return_from_reaches_active_block_through_eval_and_compile() {
    let source = "(block done (funcall (lambda () (funcall (lambda () (return-from done :nested))))) :unreachable)";
    for (path, compile) in [("eval", false), ("compile", true)] {
        let result = run_runtime(source, compile)
            .unwrap_or_else(|error| panic!("{path} failed for {source}: {error:?}"));
        assert_eq!(result, ":NESTED", "{path}: {source}");
    }
}

#[test]
fn lambda_parameter_init_forms_can_return_from_active_block_through_eval_and_compile() {
    let cases = [
        (
            "optional",
            "(block b (funcall (lambda (&optional (x (return-from b 1))) x)))",
        ),
        (
            "key",
            "(block b (funcall (lambda (&key (x (return-from b 1))) x)))",
        ),
        (
            "aux",
            "(block b (funcall (lambda (&aux (x (return-from b 1))) x)))",
        ),
        (
            "labels aux",
            "(block b (labels ((exit-default (&aux (x (return-from b 1))) x) (peer () nil)) (exit-default)))",
        ),
    ];
    for (parameter_kind, source) in cases {
        for (path, compile) in [("eval", false), ("compile", true)] {
            let result = run_runtime(source, compile).unwrap_or_else(|error| {
                panic!("{path} failed for {parameter_kind}: {source}: {error:?}")
            });
            assert_eq!(result, "1", "{path} {parameter_kind}: {source}");
        }
    }
}

#[test]
fn lambda_init_setq_updates_captured_outer_through_eval_and_compile() {
    let cases = [
        (
            "optional",
            "(let ((outer 0)) (funcall (lambda (&optional (x (setq outer 1))) x)) outer)",
        ),
        (
            "key",
            "(let ((outer 0)) (funcall (lambda (&key (x (setq outer 1))) x)) outer)",
        ),
    ];
    for (parameter_kind, source) in cases {
        for (path, compile) in [("eval", false), ("compile", true)] {
            let result = run_runtime(source, compile).unwrap_or_else(|error| {
                panic!("{path} failed for {parameter_kind}: {source}: {error:?}")
            });
            assert_eq!(result, "1", "{path} {parameter_kind}: {source}");
        }
    }
}

#[test]
fn lambda_parameter_init_forms_capture_lexical_variables_through_eval_and_compile() {
    let cases = [
        (
            "optional",
            "(let ((outer 7)) (funcall (lambda (&optional (x outer)) x)))",
        ),
        (
            "key",
            "(let ((outer 7)) (funcall (lambda (&key (x outer)) x)))",
        ),
        (
            "aux",
            "(let ((outer 7)) (funcall (lambda (&aux (x outer)) x)))",
        ),
        (
            "labels function",
            "(labels ((local-init-peer (x) x) (with-default (&optional (x (local-init-peer 7))) x)) (with-default))",
        ),
    ];
    for (parameter_kind, source) in cases {
        for (path, compile) in [("eval", false), ("compile", true)] {
            let result = run_runtime(source, compile).unwrap_or_else(|error| {
                panic!("{path} failed for {parameter_kind}: {source}: {error:?}")
            });
            assert_eq!(result, "7", "{path} {parameter_kind}: {source}");
        }
    }
}

#[test]
fn lambda_key_initializer_sees_rest_parameter_through_eval_and_compile() {
    let source =
        "(funcall (lambda (&rest rest &key (x (car rest)) &allow-other-keys) x) :unknown 7)";
    for (path, compile) in [("eval", false), ("compile", true)] {
        let result = run_runtime(source, compile)
            .unwrap_or_else(|error| panic!("{path} failed for {source}: {error:?}"));
        assert_eq!(result, ":UNKNOWN", "{path}: {source}");
    }
}

#[test]
fn nested_return_from_wrapper_forms_reaches_active_block_through_eval_and_compile() {
    let cases = [
        (
            "THROW around closure call",
            "(block done (catch 'tag (throw 'tag (funcall (lambda () (return-from done :x))))) :after)",
        ),
        (
            "THROW around direct return inside closure",
            "(block done (funcall (lambda () (catch 'tag (throw 'tag (return-from done :x))))))",
        ),
        (
            "MULTIPLE-VALUE-PROG1 around closure call",
            "(block done (multiple-value-prog1 (funcall (lambda () (return-from done :x))) :ignored) :after)",
        ),
        (
            "MULTIPLE-VALUE-PROG1 around direct return inside closure",
            "(block done (funcall (lambda () (multiple-value-prog1 (return-from done :x) :ignored))))",
        ),
        (
            "THE around closure call",
            "(block done (the t (funcall (lambda () (return-from done :x)))) :after)",
        ),
        (
            "THE around direct return inside closure",
            "(block done (funcall (lambda () (the t (return-from done :x)))))",
        ),
        (
            "SETQ value around direct return inside closure",
            "(let ((x 0)) (block done (funcall (lambda () (setq x (return-from done :x)))) :after))",
        ),
        (
            "FLET body around direct return inside closure",
            "(block done (funcall (lambda () (flet ((noop () nil)) (return-from done :x)))) :after)",
        ),
        (
            "MULTIPLE-VALUE-CALL argument around closure call",
            "(block done (multiple-value-call #'list (funcall (lambda () (return-from done :x)))) :after)",
        ),
        (
            "EVAL-WHEN around direct return inside closure",
            "(block done (funcall (lambda () (eval-when (:execute) (return-from done :x)))))",
        ),
        (
            "EVAL-WHEN around nested closure return",
            "(block done (funcall (funcall (lambda () (eval-when (:execute) (lambda () (return-from done :x)))))))",
        ),
    ];

    for (wrapper, source) in cases {
        for (path, compile) in [("eval", false), ("compile", true)] {
            let result = run_runtime(source, compile)
                .unwrap_or_else(|error| panic!("{wrapper}; {path} failed for {source}: {error:?}"));
            assert_eq!(result, ":X", "{wrapper}; {path}: {source}");
        }
    }
}

#[test]
fn nested_closure_return_from_after_block_extent_reports_control_error_through_eval_and_compile() {
    let source = "(let ((f nil)) (block done (setq f (funcall (lambda () (lambda () (return-from done :late)))))) (funcall f))";
    for (path, compile) in [("eval", false), ("compile", true)] {
        let error = match run_runtime(source, compile) {
            Err(error) => error,
            Ok(value) => panic!("{path} unexpectedly returned {value} for {source}"),
        };
        assert!(
            format!("{error:?}").contains("ControlError"),
            "{path}: expected CONTROL-ERROR for {source}, got {error:?}"
        );
    }
}

#[test]
fn nested_closure_return_from_runs_unwind_protect_cleanup_through_eval_and_compile() {
    let source = "(let ((cleanup nil)) (list (block done (unwind-protect (funcall (lambda () (funcall (lambda () (return-from done :nested))))) (setq cleanup :ran))) cleanup))";
    for (path, compile) in [("eval", false), ("compile", true)] {
        let result = run_runtime(source, compile)
            .unwrap_or_else(|error| panic!("{path} failed for {source}: {error:?}"));
        assert_eq!(result, "(:NESTED :RAN)", "{path}: {source}");
    }
}
