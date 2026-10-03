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

#[test]
fn defpackage_imports_and_interns_symbols_through_the_cli() {
    let output = ncl()
        .args([
            "--eval",
            "(progn (defpackage \"NCL-DEFPKG4-SOURCE\" (:use #:cl)) (intern \"IMPORTED\" \"NCL-DEFPKG4-SOURCE\") (intern \"SHADOWED\" \"NCL-DEFPKG4-SOURCE\") (defpackage \"NCL-DEFPKG4-TARGET\" (:use #:cl) (:import-from \"NCL-DEFPKG4-SOURCE\" \"IMPORTED\") (:shadowing-import-from \"NCL-DEFPKG4-SOURCE\" \"SHADOWED\") (:intern \"LOCAL\") (:documentation \"target\") (:size 32)) (if (and (eq (find-symbol \"IMPORTED\" \"NCL-DEFPKG4-SOURCE\") (find-symbol \"IMPORTED\" \"NCL-DEFPKG4-TARGET\")) (eq (find-symbol \"SHADOWED\" \"NCL-DEFPKG4-SOURCE\") (find-symbol \"SHADOWED\" \"NCL-DEFPKG4-TARGET\")) (find-symbol \"LOCAL\" \"NCL-DEFPKG4-TARGET\")) t nil))",
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

#[test]
fn package_builtins_accept_lisp_designators_and_mutate_symbols() {
    let output = ncl()
        .args([
            "--eval",
            "(progn (defpackage \"NCL-DEFPKG4-BUILTINS\" (:use #:cl)) (let* ((package (find-package 'NCL-DEFPKG4-BUILTINS)) (symbol (intern \"VALUE\" package))) (set symbol 9) (export (list symbol) package) (if (and (packagep package) (eq (find-symbol \"VALUE\" 'NCL-DEFPKG4-BUILTINS) symbol) (eq (symbol-value symbol) 9) (eq (find-symbol \"MISSING\" package) nil)) t nil)))",
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
