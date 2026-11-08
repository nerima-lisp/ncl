#[path = "common/mod.rs"]
mod common;

use std::process::Command;

#[test]
fn long_form_method_combination_matches_literal_qualifier_patterns() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ncl"));
    command.args([
        "--eval",
        include_str!("fixtures/long_form_method_combination.lisp"),
    ]);
    let output = common::run_command(&mut command);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "T");
}
