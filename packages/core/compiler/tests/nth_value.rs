//! Native NTH-VALUE compilation and operand diagnostics.

use ncl_compiler::{CompileError, CompileErrorKind, Compiler, Instruction, Program};
use ncl_syntax::read;

fn compile(source: &str) -> Result<Program, CompileError> {
    let forms =
        read(source).unwrap_or_else(|error| panic!("reader failed for {source}: {error:?}"));
    Compiler::compile_forms(&forms)
}

#[test]
fn pure_nth_value_never_emits_eval_in_any_function() {
    for source in [
        "(nth-value 1 (values 10 20 30))",
        "(nth-value (values 0 1) (values))",
        "(nth-value 0 (nth-value 1 (values 10 20)))",
        "(let ((x 10)) (funcall (lambda () (nth-value 1 (values 0 x)))))",
    ] {
        let program = compile(source)
            .unwrap_or_else(|error| panic!("compilation failed for {source}: {error:?}"));
        assert!(!program.functions.is_empty(), "{source}");
        for (id, function) in program.functions.iter().enumerate() {
            assert!(!function.instructions.is_empty(), "function {id}: {source}");
            assert!(
                function.instructions.iter().all(|instruction| !matches!(
                    instruction,
                    Instruction::Eval(_) | Instruction::EvalForm(_)
                )),
                "function {id} contains Eval: {source}"
            );
        }
    }
}

#[test]
fn nth_value_requires_exactly_two_arguments() {
    for (source, actual) in [
        ("(nth-value)", 0),
        ("(nth-value 0)", 1),
        ("(nth-value 0 1 2)", 3),
    ] {
        let Err(error) = compile(source) else {
            panic!("invalid arity compiled: {source}");
        };
        assert_eq!(
            error.kind,
            CompileErrorKind::Arity {
                operator: "NTH-VALUE".into(),
                expected: "two".into(),
                actual,
            },
            "{source}"
        );
    }
    assert!(compile("(nth-value 0 1)").is_ok());
}

#[test]
fn nth_value_propagates_syntax_errors_from_both_operands() {
    for source in ["(nth-value (if) 1)", "(nth-value 0 (if))"] {
        let Err(error) = compile(source) else {
            panic!("malformed operand compiled: {source}");
        };
        assert_eq!(
            error.kind,
            CompileErrorKind::Arity {
                operator: "IF".into(),
                expected: "two or three".into(),
                actual: 0,
            },
            "{source}"
        );
        assert_eq!(&source[error.span.start..error.span.end], "(if)");
    }
}
