//! CLOS method-body rewriting must preserve dotted list structure used by LOOP.

use std::process::Command;

#[test]
fn defmethod_loop_preserves_dotted_destructuring_in_its_body() {
    // CLHS LOOP defines dotted destructuring variables as (d-var-spec . d-var-spec).
    let form = "(progn (defparameter *seen-pairs* nil) (defgeneric seen-pair-p (x y)) (defmethod seen-pair-p ((x cons) (y cons)) (loop for (x2 . y2) in *seen-pairs* thereis (and (eq x x2) (eq y y2)))) (let ((x (cons 'a 'b)) (y (cons 'c 'd))) (setf *seen-pairs* (list (cons x y))) (seen-pair-p x y)))";
    let output = match Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", form])
        .output()
    {
        Ok(output) => output,
        Err(error) => panic!("failed to execute NCL: {error}"),
    };
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();

    assert!(output.status.success(), "NCL failed: {stderr}");
    assert_eq!(stdout, "T", "unexpected result (stderr: {stderr})");
}
