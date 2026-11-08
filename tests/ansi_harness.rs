//! End-to-end regression test for ANSI fixture loading and custom method combinations.

#[path = "common/mod.rs"]
mod common;

use std::path::Path;
use std::process::Command;

// CLHS 7.6.6.3: a predicate selects which applicable methods enter its method group.
#[test]
fn ansi_harness_fixture_loads_and_dispatches_custom_method_combination() {
    let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ansi_harness");
    let source = r#"
        (progn
          (load "gclload1.lsp")
          (load "gclload2.lsp")
          (format t "BEGIN ansi-harness-smoke~%")
          (unless (and (get 'randomized 'ncl::%clos-method-combination-definition)
                       (positive-integer-qualifier-p '(1))
                       (not (positive-integer-qualifier-p '(0))))
            (error "ANSI harness fixture definitions were not registered"))
          (let ((result (ansi-randomized-method :fixture)))
            (unless (or (eq result :low) (eq result :high))
              (error "RANDOMIZED selected no qualified method"))
            (format t "RANDOMIZED-RESULT ~S~%" result))
          (format t "PASS ansi-harness-smoke~%")
          t)
    "#;
    let mut command = Command::new(env!("CARGO_BIN_EXE_ncl"));
    command.current_dir(fixture_dir).args(["--eval", source]);
    let result = common::run_command(&mut command);
    assert!(result.status.success(), "{result:?}");
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(stdout.contains("BEGIN ansi-harness-smoke"), "{stdout}");
    assert!(stdout.contains("PASS ansi-harness-smoke"), "{stdout}");
    assert!(
        stdout.contains("RANDOMIZED-RESULT :LOW") || stdout.contains("RANDOMIZED-RESULT :HIGH"),
        "{stdout}"
    );
}
