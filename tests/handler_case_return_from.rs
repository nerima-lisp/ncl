//! Non-local exits from HANDLER-CASE clauses must reach their lexical BLOCK target.

use std::process::Command;

#[test]
fn handler_case_return_from_exits_the_enclosing_block() {
    // CLHS RETURN-FROM and HANDLER-CASE: values return from the enclosing block.
    for (form, expected) in [
        (
            "(block failed (handler-case (values) (error () (return-from failed :not-run))))",
            "NIL",
        ),
        (
            "(block failed (handler-case (error \"trigger\") (error () (return-from failed :handled))))",
            ":HANDLED",
        ),
    ] {
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
        assert_eq!(
            stdout, expected,
            "unexpected result for {form} (stderr: {stderr})"
        );
    }
}
