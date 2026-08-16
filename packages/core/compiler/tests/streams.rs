use ncl_compiler::{CompileErrorKind, Compiler, Instruction, Program};
use ncl_syntax::read;

fn compile(source: &str) -> Program {
    let forms = read(source).expect("test source should parse");
    Compiler::compile_forms(&forms).expect("test source should compile")
}

fn compile_error(source: &str) -> CompileErrorKind {
    let forms = read(source).expect("test source should parse");
    Compiler::compile_forms(&forms)
        .expect_err("test source should be rejected")
        .kind
}

fn has_instruction<F>(program: &Program, predicate: F) -> bool
where
    F: Fn(&Instruction) -> bool,
{
    program
        .functions
        .iter()
        .flat_map(|function| &function.instructions)
        .any(predicate)
}

#[test]
fn lowers_with_open_file_and_validates_its_binding() {
    let without_body = compile("(with-open-file (stream \"path\"))");
    assert!(has_instruction(&without_body, |instruction| matches!(
        instruction,
        Instruction::FunctionLoad(name) if name == "OPEN"
    )));
    assert!(has_instruction(&without_body, |instruction| matches!(
        instruction,
        Instruction::FunctionLoad(name) if name == "CLOSE"
    )));
    assert!(has_instruction(&without_body, |instruction| matches!(
        instruction,
        Instruction::UnwindProtect { .. }
    )));

    let with_body = compile("(with-open-file (stream \"path\") (read stream))");
    assert!(has_instruction(&with_body, |instruction| matches!(
        instruction,
        Instruction::FunctionLoad(name) if name == "READ"
    )));

    assert!(matches!(
        compile_error("(with-open-file)"),
        CompileErrorKind::Arity { operator, .. } if operator == "WITH-OPEN-FILE"
    ));
    assert_eq!(
        compile_error("(with-open-file stream)"),
        CompileErrorKind::ExpectedList {
            context: "WITH-OPEN-FILE binding".to_string(),
        }
    );
    assert!(matches!(
        compile_error("(with-open-file (stream))"),
        CompileErrorKind::InvalidForm { message }
            if message == "WITH-OPEN-FILE binding needs a stream variable and pathname"
    ));
    assert_eq!(
        compile_error("(with-open-file (1 \"path\"))"),
        CompileErrorKind::ExpectedSymbol {
            context: "WITH-OPEN-FILE stream variable".to_string(),
        }
    );
}

#[test]
fn lowers_with_output_to_string_with_or_without_a_destination() {
    let without_destination = compile(
        "(with-output-to-string (stream)
           (write-string \"hello\" stream))",
    );
    for name in [
        "MAKE-STRING-OUTPUT-STREAM",
        "GET-OUTPUT-STREAM-STRING",
        "CLOSE",
    ] {
        assert!(has_instruction(
            &without_destination,
            |instruction| matches!(
                instruction,
                Instruction::FunctionLoad(found) if found == name
            )
        ));
    }
    assert!(has_instruction(
        &without_destination,
        |instruction| matches!(instruction, Instruction::UnwindProtect { .. })
    ));

    let without_body = compile("(with-output-to-string (stream))");
    assert!(has_instruction(&without_body, |instruction| matches!(
        instruction,
        Instruction::FunctionLoad(name) if name == "GET-OUTPUT-STREAM-STRING"
    )));

    let with_destination = compile(
        "(with-output-to-string (stream result)
           (write-string \"hello\" stream))",
    );
    assert!(has_instruction(&with_destination, |instruction| matches!(
        instruction,
        Instruction::FunctionLoad(name) if name == "__NCL_APPEND_OUTPUT_TO_STRING"
    )));
    assert!(has_instruction(&with_destination, |instruction| matches!(
        instruction,
        Instruction::Setf(_)
    )));

    assert!(matches!(
        compile_error("(with-output-to-string)"),
        CompileErrorKind::Arity { operator, .. } if operator == "WITH-OUTPUT-TO-STRING"
    ));
    assert_eq!(
        compile_error("(with-output-to-string stream)"),
        CompileErrorKind::ExpectedList {
            context: "WITH-OUTPUT-TO-STRING binding".to_string(),
        }
    );
    for source in [
        "(with-output-to-string ())",
        "(with-output-to-string (stream result extra))",
    ] {
        assert!(matches!(
            compile_error(source),
            CompileErrorKind::InvalidForm { message }
                if message == "WITH-OUTPUT-TO-STRING binding needs a stream variable and optional string place"
        ));
    }
    assert_eq!(
        compile_error("(with-output-to-string (1))"),
        CompileErrorKind::ExpectedSymbol {
            context: "WITH-OUTPUT-TO-STRING stream variable".to_string(),
        }
    );
}

#[test]
fn lowers_with_input_from_string_options_and_index_place() {
    let no_options = compile("(with-input-from-string (stream \"hello\"))");
    assert!(has_instruction(&no_options, |instruction| matches!(
        instruction,
        Instruction::FunctionLoad(name) if name == "MAKE-STRING-INPUT-STREAM"
    )));

    let start_only = compile("(with-input-from-string (stream \"hello\" :start 1))");
    assert!(has_instruction(&start_only, |instruction| matches!(
        instruction,
        Instruction::Constant(ncl_compiler::Constant::Integer(1))
    )));

    let end_only = compile("(with-input-from-string (stream \"hello\" :end 3))");
    assert!(has_instruction(&end_only, |instruction| matches!(
        instruction,
        Instruction::Constant(ncl_compiler::Constant::Integer(0))
    )));

    let all_options = compile(
        "(with-input-from-string (stream \"hello\" :start 1 :end 3 :index index)
           (read-char stream))",
    );
    assert!(has_instruction(&all_options, |instruction| matches!(
        instruction,
        Instruction::FunctionLoad(name) if name == "%STREAM-INPUT-POSITION"
    )));
    assert!(has_instruction(&all_options, |instruction| matches!(
        instruction,
        Instruction::Setf(_)
    )));
    assert!(has_instruction(&all_options, |instruction| matches!(
        instruction,
        Instruction::UnwindProtect { .. }
    )));

    assert!(matches!(
        compile_error("(with-input-from-string)"),
        CompileErrorKind::Arity { operator, .. } if operator == "WITH-INPUT-FROM-STRING"
    ));
    assert_eq!(
        compile_error("(with-input-from-string stream)"),
        CompileErrorKind::ExpectedList {
            context: "WITH-INPUT-FROM-STRING binding".to_string(),
        }
    );
    assert!(matches!(
        compile_error("(with-input-from-string (stream))"),
        CompileErrorKind::InvalidForm { message }
            if message == "WITH-INPUT-FROM-STRING binding needs a stream variable and string"
    ));
    assert_eq!(
        compile_error("(with-input-from-string (1 \"hello\"))"),
        CompileErrorKind::ExpectedSymbol {
            context: "WITH-INPUT-FROM-STRING stream variable".to_string(),
        }
    );
    assert!(matches!(
        compile_error("(with-input-from-string (stream \"hello\" :start))"),
        CompileErrorKind::InvalidForm { message }
            if message == "WITH-INPUT-FROM-STRING options need keyword/value pairs"
    ));
    assert!(matches!(
        compile_error("(with-input-from-string (stream \"hello\" start 1))"),
        CompileErrorKind::InvalidForm { message }
            if message == "WITH-INPUT-FROM-STRING option must be a keyword"
    ));
    assert!(matches!(
        compile_error("(with-input-from-string (stream \"hello\" :unknown 1))"),
        CompileErrorKind::InvalidForm { message }
            if message == "WITH-INPUT-FROM-STRING option is not supported"
    ));
}

#[test]
fn rejects_duplicate_with_input_from_string_options() {
    for (source, message) in [
        (
            "(with-input-from-string (stream \"hello\" :start 1 :start 2))",
            "WITH-INPUT-FROM-STRING :start may appear only once",
        ),
        (
            "(with-input-from-string (stream \"hello\" :end 1 :end 2))",
            "WITH-INPUT-FROM-STRING :end may appear only once",
        ),
        (
            "(with-input-from-string (stream \"hello\" :index i :index j))",
            "WITH-INPUT-FROM-STRING :index may appear only once",
        ),
    ] {
        assert_eq!(
            compile_error(source),
            CompileErrorKind::InvalidForm {
                message: message.to_string(),
            }
        );
    }
}
