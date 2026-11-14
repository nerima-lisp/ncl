//! End-to-end tests for macro expansion and LOOP forms.

#[path = "common/mod.rs"]
mod common;

use common::{CliEvalCase, assert_ncl_eval_cases, run_ncl};

#[path = "common/e2emacro_probes.rs"]
mod e2emacro_probes;

use e2emacro_probes::{PROBES, Probe};

struct XFail {
    name: &'static str,
    source: &'static str,
    stderr: &'static str,
    exit_code: i32,
}

/// A form that must fail: unlike [`XFail`], this is the *correct*, permanent
/// outcome (an unhandled condition), not a gap to eventually close.
struct ErrorCase {
    name: &'static str,
    source: &'static str,
    stderr: &'static str,
    exit_code: i32,
}

const XFAILS: &[XFail] = &[XFail {
    name: "loop-with-finally",
    source: "(loop with x = 2 finally (return x))",
    stderr: "UndefinedValue",
    exit_code: 1,
}];

const ERROR_CASES: &[ErrorCase] = &[
    // Unsupported LOOP package iteration clauses name the rejected clause.
    ErrorCase {
        name: "loop-unsupported-being-clause-reports-it",
        source: "(loop for symbol being the widgets)",
        stderr: "Unsupported LOOP clause: BEING THE WIDGETS",
        exit_code: 1,
    },
    // B7: ECASE, unlike CASE, signals when nothing matches.
    ErrorCase {
        name: "ecase-no-match-signals",
        source: "(ecase 5 (1 'a) (2 'b))",
        stderr: "Unsupported",
        exit_code: 1,
    },
    // B7: CCASE gets the same treatment as ECASE.
    ErrorCase {
        name: "ccase-no-match-signals",
        source: "(ccase 5 (1 'a) (2 'b))",
        stderr: "Unsupported",
        exit_code: 1,
    },
    // B2: a shape mismatch signals instead of binding garbage (was
    // `((1 2) 48)` for the two-argument case; see `destructuring-bind-*`
    // probes above for the value-correct cases).
    ErrorCase {
        name: "destructuring-bind-too-few-signals",
        source: "(destructuring-bind (a b) (list 1) (list a b))",
        stderr: "too few elements",
        exit_code: 1,
    },
    ErrorCase {
        name: "destructuring-bind-too-many-signals",
        source: "(destructuring-bind (a) (list 1 2) a)",
        stderr: "too many elements",
        exit_code: 1,
    },
];

fn macro_probe_requires_process(name: &str) -> bool {
    matches!(
        name,
        "defun-fdefinition-lookup"
            | "defmacro-macro-function-lookup"
            | "backquote-in-defmacro-body"
            | "defmacro-body-keyword"
            | "defmacro-nested-pattern"
            | "defmacro-whole"
            | "defmacro-optional-key-defaults"
            | "define-condition-initform-accessor"
            | "macrolet-shadows-global"
            // Their macro expansions fail when multiple probes share one compiled --eval.
            | "destructuring-bind-simple"
            | "destructuring-bind-single"
            | "destructuring-bind-nested-optional-rest"
            | "destructuring-bind-nested-key"
            | "destructuring-bind-dotted"
            | "destructuring-bind-key-default-supplied-p"
            | "destructuring-bind-optional-supplied-p"
            | "destructuring-bind-explicit-key-supplied-p"
    )
}

#[test]
fn standard_macro_probes_assert_compiled_results() {
    assert_eq!(PROBES.len(), 70);
    let batch = PROBES
        .iter()
        .filter(|probe| !macro_probe_requires_process(probe.name))
        .map(|probe| CliEvalCase::new(probe.name, probe.source, probe.expected))
        .collect::<Vec<_>>();
    assert_eq!(batch.len(), 53);
    assert_ncl_eval_cases(&batch);

    for probe in PROBES
        .iter()
        .filter(|probe| macro_probe_requires_process(probe.name))
    {
        let output = run_ncl(probe.source);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}: {:?}",
            probe.name,
            output
        );
        assert!(output.stderr.is_empty(), "{}: {:?}", probe.name, output);
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            probe.expected,
            "{}",
            probe.name
        );
    }
}

#[test]
fn cl_bench_hash_strings_typed_loop_evaluates_end_to_end() {
    let source = r#"(let ((table (make-hash-table :test 'equal)))
                    (loop for i fixnum from 2 downto 0
                          with q fixnum = 17 and r fixnum = 0
                          do (multiple-value-setq (q r) (floor q 4))
                             (setf (gethash (if (evenp i) "even" "odd") table)
                                   (list i q r)))
                    (list (gethash "even" table) (gethash "odd" table)))"#;
    let output = run_ncl(source);
    assert_eq!(output.status.code(), Some(0), "{source}: {output:?}");
    assert!(output.stderr.is_empty(), "{source}: {output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "((0 0 1) (1 1 0))"
    );
}

#[test]
fn do_symbols_probes_assert_clhs_results() {
    let probes = [
        Probe {
            name: "do-symbols-includes-inherited-and-declares",
            source: r#"(let* ((pkg (make-package "NCL-B4-DS-STRING" :use nil))
                            (local (intern "LOCAL" pkg))
                            (used (make-package "NCL-B4-DS-USED" :use nil))
                            (inherited (intern "INHERITED" used)))
                       (export (list inherited) used)
                       (use-package used pkg)
                       (let ((local-seen nil) (inherited-seen nil) (count 0))
                         (do-symbols (symbol "NCL-B4-DS-STRING"
                                             (list local-seen inherited-seen (= count 2) (null symbol)))
                           (declare (special symbol))
                           (setq count (1+ count))
                           (when (eq symbol local) (setq local-seen t))
                           (when (eq symbol inherited) (setq inherited-seen t)))))"#,
            expected: "(T T T T)",
        },
        Probe {
            name: "do-symbols-accepts-symbol-package-designator",
            source: r#"(let* ((pkg (make-package "NCL-B4-DS-SYMBOL" :use nil))
                            (target (intern "TARGET" pkg))
                            (seen nil))
                       (do-symbols (symbol 'NCL-B4-DS-SYMBOL
                                           (and (eq seen target) (null symbol)))
                         (setq seen symbol)))"#,
            expected: "T",
        },
        Probe {
            name: "do-external-symbols-accepts-package-object",
            source: r#"(let* ((pkg (make-package "NCL-B4-DO-EXTERNAL" :use nil))
                            (internal (intern "INTERNAL" pkg))
                            (external (intern "EXTERNAL" pkg))
                            (seen nil)
                            (count 0))
                       (export (list external) pkg)
                       (do-external-symbols (symbol pkg
                                               (list (eq seen external) (= count 1) (null symbol)))
                         (setq seen symbol)
                         (setq count (1+ count))))"#,
            expected: "(T T T)",
        },
        Probe {
            name: "do-all-symbols-counts-a-local-symbol",
            source: r#"(let* ((pkg (make-package "NCL-B4-DO-ALL" :use nil))
                            (target (intern "ONLY-HERE" pkg))
                            (found nil)
                            (count 0))
                       (do-all-symbols (symbol (list found (= count 1) (null symbol)))
                         (when (eq symbol target)
                           (setq found t)
                           (setq count (1+ count)))))"#,
            expected: "(T T T)",
        },
        Probe {
            name: "do-symbols-return-preserves-values",
            source: r#"(let* ((pkg (make-package "NCL-B4-DO-RETURN" :use nil))
                            (entry (intern "ENTRY" pkg)))
                       (multiple-value-list
                         (do-symbols (symbol pkg)
                           (return (values :EARLY :EXIT)))))"#,
            expected: "(:EARLY :EXIT)",
        },
    ];

    for probe in probes {
        let output = run_ncl(probe.source);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}: {:?}",
            probe.name,
            output
        );
        assert!(output.stderr.is_empty(), "{}: {:?}", probe.name, output);
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            probe.expected,
            "{}",
            probe.name
        );
    }
}

#[test]
fn loop_package_iteration_updates_following_for_clause() {
    let source = r#"(let* ((pkg (make-package "NCL-B5-LOOP-SYMBOLS" :use nil))
                           (target (intern "TARGET" pkg))
                           (external (intern "EXTERNAL" pkg))
                           (ignored (export (list external) pkg))
                           (count 0)
                           (evaluations 0)
                           (initial-values-ready nil)
                           (status-ok t))
                      (loop for sym being the symbols in pkg
                            for status = (if sym
                                             (progn
                                               (setq evaluations (1+ evaluations))
                                               (nth-value 1 (find-symbol (symbol-name sym) pkg)))
                                             nil)
                            do (when (= count 0)
                                 (setq initial-values-ready (= evaluations 1)))
                               (setq count (1+ count))
                               (unless (eq status (nth-value 1 (find-symbol (symbol-name sym) pkg)))
                                 (setq status-ok nil)))
                      (list (= count 2)
                            (= evaluations 2)
                            initial-values-ready
                            status-ok))"#;
    let output = run_ncl(source);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "(T T T T)");
}

#[test]
fn macro_forms_that_must_error_do_error() {
    for case in ERROR_CASES {
        let output = run_ncl(case.source);
        assert_eq!(output.status.code(), Some(case.exit_code), "{}", case.name);
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(case.stderr),
            "{}: {:?}",
            case.name,
            output
        );
    }
}

#[test]
fn unsupported_standard_macro_paths_remain_explicit_xfails() {
    for case in XFAILS {
        let output = run_ncl(case.source);
        assert_eq!(output.status.code(), Some(case.exit_code), "{}", case.name);
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(case.stderr),
            "{}: {:?}",
            case.name,
            output
        );
    }
}
