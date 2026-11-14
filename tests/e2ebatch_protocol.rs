//! Tests for the batched CLI E2E output protocol.

#[path = "common/mod.rs"]
mod common;

use common::{CliEvalCase, assert_case_outputs};

#[test]
fn reads_length_prefixed_unicode_with_delimiter_text() {
    let printed = "λS5:🧪";
    let case = CliEvalCase::new("framed-value", "", printed);
    let output = format!("S{}:{}NIL\n", printed.chars().count(), printed);

    assert_case_outputs(&output, &[case], 0, 0);
}

#[test]
#[should_panic(expected = "wrong-value#0")]
fn rejects_a_printed_value_that_differs_from_expected() {
    let case = CliEvalCase::new("wrong-value", "", "right");

    assert_case_outputs("S5:wrongNIL\n", &[case], 0, 0);
}

#[test]
#[should_panic(expected = "condition#3: NCL evaluation signaled a condition")]
fn identifies_a_case_that_signaled_a_condition() {
    let case = CliEvalCase::new("condition", "", "unused");

    assert_case_outputs("E0:NIL\n", &[case], 3, 0);
}
