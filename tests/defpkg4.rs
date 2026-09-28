#![allow(missing_docs)]

use std::process::Command;

fn ncl() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ncl"))
}

#[test]
fn common_lisp_alias_and_defpackage_use_are_unique() {
    let output = ncl()
        .args([
            "--eval",
            "(progn (defpackage \"NCL-DEFPKG4-HASH\" (:use #:cl)) (defpackage \"NCL-DEFPKG4-STRING\" (:use \"CL\")) (if (and (eq (find-package \"CL\") (find-package \"COMMON-LISP\")) (find (find-package \"COMMON-LISP\") (package-use-list (find-package \"NCL-DEFPKG4-HASH\"))) (find (find-package \"COMMON-LISP\") (package-use-list (find-package \"NCL-DEFPKG4-STRING\")))) t nil))",
        ])
        .output()
        .unwrap_or_else(|error| panic!("ncl failed to start: {error}"));
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "T");
}
