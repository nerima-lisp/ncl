#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

use ncl_compiler::{CompileErrorKind, Compiler, Constant, Instruction};
use ncl_syntax::{Form, FormKind, Span, read};

fn compile(source: &str) -> ncl_compiler::Program {
    let forms = read(source).expect("test source should parse");
    Compiler::compile_forms(&forms).expect("test source should compile")
}

#[test]
fn compiles_entry_point_shapes_from_table_cases() {
    let cases = [("empty sequence", ""), ("single form", "42")];

    for (name, source) in cases {
        let forms = read(source).expect("test source should parse");
        let sequence = Compiler::compile_forms(&forms).expect(name);
        assert_eq!(sequence.entry, 0, "{name}");
        assert_eq!(sequence.functions.len(), 1, "{name}");
        assert_eq!(
            sequence.functions[0].instructions.last(),
            Some(&Instruction::Return),
            "{name}"
        );

        if let Some(form) = forms.first() {
            let single = Compiler::compile_form(form).expect(name);
            assert_eq!(single.functions, sequence.functions, "{name}");
        }
    }
}

#[test]
fn rejects_incomplete_compilation_forms_from_table_cases() {
    let cases = [
        ("lambda", "(lambda)"),
        ("function", "(function)"),
        ("define", "(define name)"),
        ("defun", "(defun name)"),
        ("setq", "(setq name)"),
        ("psetq", "(psetq name)"),
        ("multiple-value-setq", "(multiple-value-setq)"),
        ("setf", "(setf place)"),
        ("defvar", "(defvar)"),
        ("funcall", "(funcall)"),
        ("eval", "(eval)"),
        ("apply", "(apply function)"),
        ("mapcar", "(mapcar function)"),
        ("map-into", "(map-into sequence)"),
    ];

    for (name, source) in cases {
        let forms = read(source).expect("test source should parse");
        let error = Compiler::compile_forms(&forms).expect_err(name);
        assert!(!error.to_string().is_empty(), "{name}: {error:?}");
    }
}

#[test]
fn compiles_arithmetic_shaped_calls_and_normalizes_names() {
    let program = compile("(+ 1 2)");

    assert_eq!(program.entry, 0);
    assert_eq!(program.functions.len(), 1);
    assert_eq!(
        program.functions[0].instructions,
        vec![
            Instruction::FunctionLoad("+".to_string()),
            Instruction::Constant(Constant::Integer(1)),
            Instruction::Constant(Constant::Integer(2)),
            Instruction::Call(2),
            Instruction::Return,
        ]
    );
}

#[test]
fn compiles_lexical_bindings_and_local_functions_from_table_cases() {
    let cases = [
        ("parallel let", "(let ((x 1) (y 2)) (+ x y))", false),
        ("sequential let", "(let* ((x 1) (y (+ x 1))) y)", false),
        (
            "local function",
            "(flet ((double (x) (+ x x))) (double 3))",
            true,
        ),
        (
            "recursive local function",
            "(labels ((count (n) (if (= n 0) 0 (count (- n 1))))) (count 2))",
            true,
        ),
    ];

    for (name, source, has_nested_function) in cases {
        let program = compile(source);
        assert_eq!(program.entry, 0, "{name}");
        assert_eq!(
            program.functions[0].instructions.last(),
            Some(&Instruction::Return),
            "{name}"
        );
        assert_eq!(program.functions.len() > 1, has_nested_function, "{name}");
    }
}

#[test]
fn compiles_definition_and_call_forms_from_table_cases() {
    let cases = [
        ("define", "(define answer 42)", 1),
        ("defun", "(defun answer () 42)", 2),
        ("setq", "(progn (setq answer 1) (setq answer 2))", 1),
        ("funcall", "(funcall #'+ 1 2)", 1),
        ("apply", "(apply #'+ '(1 2))", 1),
    ];

    for (name, source, function_count) in cases {
        let program = compile(source);
        assert_eq!(program.functions.len(), function_count, "{name}");
        assert_eq!(
            program.functions[program.entry].instructions.last(),
            Some(&Instruction::Return),
            "{name}"
        );
    }
}

#[test]
fn preserves_escaped_symbol_identity_in_compilation() {
    let cases = [
        (
            "function",
            "(#'|MiXeD|)",
            Instruction::FunctionLoadExact("MiXeD".to_string()),
        ),
        (
            "define",
            "(define |MiXeD| 1)",
            Instruction::DefineExact("MiXeD".to_string()),
        ),
        (
            "setq",
            "(setq |MiXeD| 1)",
            Instruction::SetExact("MiXeD".to_string()),
        ),
    ];

    for (name, source, expected) in cases {
        let program = compile(source);
        assert!(
            program.functions[0].instructions.contains(&expected),
            "{name}: {:?}",
            program.functions[0].instructions
        );
    }
}

#[test]
fn compiles_assignment_and_higher_order_forms_from_table_cases() {
    let cases = [
        ("multiple setq", "(setq first 1 second 2)"),
        ("parallel setq", "(psetq first 1 second 2)"),
        (
            "multiple values",
            "(multiple-value-setq (first second) (values 1 2))",
        ),
        ("mapcar", "(mapcar #'+ '(1 2) '(3 4))"),
        ("map-into", "(map-into destination #'+ '(1 2) '(3 4))"),
        ("defvar without initializer", "(defvar answer)"),
        ("defparameter without initializer", "(defparameter answer)"),
    ];

    for (name, source) in cases {
        let program = compile(source);
        assert_eq!(program.entry, 0, "{name}");
        assert_eq!(
            program.functions[0].instructions.last(),
            Some(&Instruction::Return),
            "{name}"
        );
    }
}

#[test]
fn compiles_runtime_definition_forms_from_table_cases() {
    let cases = [
        ("defvar", "(defvar answer 42)"),
        ("defparameter", "(defparameter answer 42)"),
        ("defconstant", "(defconstant answer 42)"),
        ("defstruct", "(defstruct point x y)"),
        ("eval-when", "(eval-when (:compile-toplevel) 42)"),
    ];

    for (name, source) in cases {
        let program = compile(source);
        assert_eq!(program.entry, 0, "{name}");
        assert_eq!(
            program.functions[0].instructions.last(),
            Some(&Instruction::Return),
            "{name}"
        );
    }
}

#[test]
fn patches_if_jump_targets() {
    let program = compile("(if #t 1 2)");

    assert_eq!(
        program.functions[0].instructions,
        vec![
            Instruction::Constant(Constant::Boolean(true)),
            Instruction::JumpIfFalse(4),
            Instruction::Constant(Constant::Integer(1)),
            Instruction::Jump(5),
            Instruction::Constant(Constant::Integer(2)),
            Instruction::Return,
        ]
    );
}

#[test]
fn compiles_lambda_as_a_closure_function() {
    let program = compile("(lambda (x) (+ x 1))");

    assert_eq!(
        program.functions[0].instructions,
        vec![Instruction::MakeClosure(1), Instruction::Return]
    );
    assert_eq!(program.functions[1].name, None);
    assert_eq!(program.functions[1].parameters, vec!["X"]);
    assert_eq!(
        program.functions[1].instructions,
        vec![
            Instruction::FunctionLoad("+".to_string()),
            Instruction::Load("X".to_string()),
            Instruction::Constant(Constant::Integer(1)),
            Instruction::Call(2),
            Instruction::Return,
        ]
    );
}

#[test]
fn compiles_rest_parameter_metadata_for_lambda_and_defun() {
    let defun = compile("(defun collect (first &rest rest) rest)");
    assert_eq!(defun.functions[1].parameters, vec!["FIRST"]);
    assert!(defun.functions[1].optional.is_empty());
    assert_eq!(defun.functions[1].rest, Some("REST".to_string()));

    let lambda = compile("(lambda (&rest values) values)");
    assert!(lambda.functions[1].parameters.is_empty());
    assert!(lambda.functions[1].optional.is_empty());
    assert_eq!(lambda.functions[1].rest, Some("VALUES".to_string()));
}

#[test]
fn compiles_optional_parameter_metadata_and_default_functions() {
    let program = compile(
        "(defun describe (required &optional (optional (+ required 1) supplied-p) &rest rest)
           (list required optional supplied-p rest))",
    );
    let function = &program.functions[1];

    assert_eq!(function.parameters, vec!["REQUIRED"]);
    assert_eq!(function.optional.len(), 1);
    assert_eq!(function.optional[0].name, "OPTIONAL");
    assert_eq!(
        function.optional[0].supplied_p.as_deref(),
        Some("SUPPLIED-P")
    );
    assert_eq!(function.rest, Some("REST".to_string()));

    let default_function = &program.functions[function.optional[0].default_function];
    assert_eq!(default_function.parameters, Vec::<String>::new());
    assert_eq!(default_function.optional.len(), 0);
    assert_eq!(default_function.rest, None);
    assert_eq!(
        default_function.instructions.last(),
        Some(&Instruction::Return)
    );
}

#[test]
fn rejects_malformed_ordinary_lambda_parameters() {
    for source in [
        "(lambda (x x) x)",
        "(lambda (x X) x)",
        "(lambda (x &rest) x)",
        "(lambda (x &rest rest extra) x)",
        "(lambda (x &rest 1) x)",
        "(defun bad (x &rest rest extra) x)",
    ] {
        let forms = read(source).expect("test source should parse");
        let error = Compiler::compile_forms(&forms).unwrap_err();

        assert!(
            matches!(
                &error.kind,
                CompileErrorKind::ExpectedSymbol { .. } | CompileErrorKind::InvalidForm { .. }
            ),
            "{source}: {error:?}"
        );
    }
}

#[test]
fn rejects_malformed_destructuring_lambda_lists() {
    for source in [
        "(destructuring-bind 1 1 1)",
        "(destructuring-bind (&whole value &whole other) 1 1)",
        "(destructuring-bind (value &whole whole) 1 1)",
        "(destructuring-bind (value &optional &optional) 1 1)",
        "(destructuring-bind (value &optional (item 1 supplied extra)) 1 1)",
        "(destructuring-bind (value &optional 1) 1 1)",
        "(destructuring-bind (value &rest) 1 1)",
        "(destructuring-bind (value &rest rest item) 1 1)",
        "(destructuring-bind (value &key (:item)) 1 1)",
        "(destructuring-bind (value &key ((item value)) 1 1) 1 1)",
        "(destructuring-bind (value &key (:item (value other))) 1 1)",
        "(destructuring-bind (value &key (: value)) 1 1)",
        "(destructuring-bind (value &key (()) 1 1) 1 1)",
        "(destructuring-bind (value &key ()) 1 1)",
        "(destructuring-bind (value &key ((:item item) 1 supplied extra)) 1 1)",
        "(destructuring-bind (value &key (:item item) (:item other)) 1 1)",
        "(destructuring-bind (value &allow-other-keys) 1 1)",
        "(destructuring-bind (value &key item &allow-other-keys other) 1 1)",
        "(destructuring-bind (value &environment environment) 1 1)",
        "(destructuring-bind (value &unsupported item) 1 1)",
        "(destructuring-bind (value &aux (count 1 extra)) 1 1)",
        "(destructuring-bind (value &aux 1) 1 1)",
        "(destructuring-bind (value &aux (count 1) &aux other) 1 1)",
        "(destructuring-bind (value value) 1 1)",
        "(destructuring-bind (value &optional (item 1 item-p) &optional other) 1 1)",
        "(destructuring-bind (value &rest rest item) 1 1)",
        "(destructuring-bind (value &key item &key other) 1 1)",
        "(destructuring-bind (value &key item &allow-other-keys &allow-other-keys) 1 1)",
        "(destructuring-bind (value &key item &aux other &key extra) 1 1)",
        "(destructuring-bind (value &key item &environment environment) 1 1)",
        "(destructuring-bind (value &key item &rest rest) 1 1)",
    ] {
        let forms = read(source).expect("test source should parse");
        assert!(Compiler::compile_forms(&forms).is_err(), "{source}");
    }
}

#[test]
fn compiles_destructuring_lambda_list_sections_from_table_cases() {
    let cases = [
        (
            "required pattern",
            "(destructuring-bind ((left right)) value left)",
        ),
        (
            "nested dotted pattern",
            "(destructuring-bind ((head . tail)) value tail)",
        ),
        (
            "dotted pattern and optional atom",
            "(destructuring-bind (head . tail) value (list head tail))",
        ),
        (
            "whole optional rest",
            "(destructuring-bind (&whole whole value &optional (maybe 42 supplied) &rest rest) value whole)",
        ),
        (
            "keywords and auxiliary",
            "(destructuring-bind (value &key item (:other alternate 7 other-supplied) &allow-other-keys &aux (local 9)) value local)",
        ),
        (
            "explicit keyword pair and auxiliary atom",
            "(destructuring-bind (value &key ((:item alternate)) &aux local) value local)",
        ),
        (
            "nested optional and keyword patterns",
            "(destructuring-bind ((left right) &optional ((maybe-left maybe-right) nil) &key ((:item (item-left item-right)))) value left)",
        ),
        (
            "body alias",
            "(destructuring-bind (value &body rest) value rest)",
        ),
    ];

    for (name, source) in cases {
        let program = compile(source);
        assert_eq!(program.entry, 0, "{name}");
        assert!(
            program.functions[0]
                .instructions
                .contains(&Instruction::Return),
            "{name}"
        );
    }
}

#[test]
fn compiles_ignore_errors_into_a_catch_boundary() {
    let program = compile("(ignore-errors (+ 1 2))");

    assert_eq!(
        program.functions[0].instructions,
        vec![Instruction::IgnoreErrors(1), Instruction::Return]
    );
    assert_eq!(
        program.functions[1].instructions,
        vec![
            Instruction::FunctionLoad("+".to_string()),
            Instruction::Constant(Constant::Integer(1)),
            Instruction::Constant(Constant::Integer(2)),
            Instruction::Call(2),
            Instruction::Return,
        ]
    );
}

#[test]
fn compiles_block_and_return_from_into_a_named_control_boundary() {
    let program = compile("(block done (return-from done 7))");

    assert_eq!(
        program.functions[0].instructions,
        vec![
            Instruction::Block {
                function: 1,
                name: "DONE".to_string(),
            },
            Instruction::Return,
        ]
    );
    assert_eq!(
        program.functions[1].instructions,
        vec![
            Instruction::Constant(Constant::Integer(7)),
            Instruction::ReturnFrom {
                name: "DONE".to_string(),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn compiles_unwind_protect_into_protected_and_cleanup_functions() {
    let program = compile("(unwind-protect 7 8)");

    assert_eq!(
        program.functions[0].instructions,
        vec![
            Instruction::UnwindProtect {
                protected: 1,
                cleanup: 2,
            },
            Instruction::Return,
        ]
    );
    assert_eq!(
        program.functions[1].instructions,
        vec![
            Instruction::Constant(Constant::Integer(7)),
            Instruction::Return
        ]
    );
    assert_eq!(
        program.functions[2].instructions,
        vec![
            Instruction::Constant(Constant::Integer(8)),
            Instruction::Return
        ]
    );
}

#[test]
fn preserves_parallel_and_sequential_let_initializer_rules() {
    let parallel = compile("(let ((x 1) (y x)) y)");
    assert_eq!(
        parallel.functions[0].instructions,
        vec![
            Instruction::EnterScope,
            Instruction::Constant(Constant::Integer(1)),
            Instruction::Load("X".to_string()),
            Instruction::Define("Y".to_string()),
            Instruction::Pop,
            Instruction::Define("X".to_string()),
            Instruction::Pop,
            Instruction::Load("Y".to_string()),
            Instruction::ExitScope,
            Instruction::Return,
        ]
    );

    let sequential = compile("(let* ((x 1) (y x)) y)");
    assert_eq!(
        sequential.functions[0].instructions,
        vec![
            Instruction::EnterScope,
            Instruction::Constant(Constant::Integer(1)),
            Instruction::Define("X".to_string()),
            Instruction::Pop,
            Instruction::Load("X".to_string()),
            Instruction::Define("Y".to_string()),
            Instruction::Pop,
            Instruction::Load("Y".to_string()),
            Instruction::ExitScope,
            Instruction::Return,
        ]
    );
}

#[test]
fn reports_malformed_forms_with_the_offending_span() {
    let form = read("(if 1)").expect("test source should parse")[0].clone();
    let error = Compiler::compile_form(&form).expect_err("malformed if should fail");

    assert_eq!(error.span, form.span);
    assert!(matches!(error.kind, CompileErrorKind::Arity { .. }));
}

#[test]
fn validates_let_binding_shapes_from_table_cases() {
    let cases = [
        ("missing bindings", "(let)", "at least one"),
        ("bindings are a list", "(let x 1)", "must be a list"),
        ("each binding is a list", "(let (x) 1)", "must be a list"),
        (
            "binding has too many forms",
            "(let ((x 1 2)) 1)",
            "needs a name",
        ),
        (
            "parallel names are unique",
            "(let ((x) (x)) 1)",
            "distinct names",
        ),
    ];

    for (name, source, expected) in cases {
        let forms = read(source).expect(name);
        let error = Compiler::compile_forms(&forms).expect_err(name);
        let kind = format!("{}", error.kind).to_ascii_lowercase();
        assert!(kind.contains(expected), "{name}: {kind}");
    }
}

#[test]
fn validates_local_function_binding_shapes_from_table_cases() {
    let cases = [
        ("missing bindings", "(flet)", "at least one"),
        ("bindings are a list", "(flet name 1)", "must be a list"),
        (
            "each binding is a list",
            "(flet (name) 1)",
            "must be a list",
        ),
        (
            "binding has too few forms",
            "(flet ((name)) 1)",
            "needs a name",
        ),
        (
            "local names are unique",
            "(flet ((name () 1) (name () 2)) 1)",
            "must be unique",
        ),
    ];

    for (name, source, expected) in cases {
        let forms = read(source).expect(name);
        let error = Compiler::compile_forms(&forms).expect_err(name);
        let message = format!("{}", error.kind).to_ascii_lowercase();
        assert!(message.contains(expected), "{name}: {message}");
    }
}

#[test]
fn compiles_let_bindings_without_values_as_nil() {
    let program = compile("(let ((value)) value)");

    assert!(
        program.functions[0]
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Constant(Constant::Nil)))
    );
}

#[test]
fn lowers_prog1_with_retained_value_and_ordered_tail_effects() {
    let program = compile("(prog1 (setq marker 1) (setq marker 2) (setq marker 3))");

    assert_eq!(
        program.functions[0].instructions,
        vec![
            Instruction::EnterScope,
            Instruction::Constant(Constant::Integer(1)),
            Instruction::Set("MARKER".to_string()),
            Instruction::Define("__NCL_PROG1_VALUE_0".to_string()),
            Instruction::Pop,
            Instruction::Constant(Constant::Integer(2)),
            Instruction::Set("MARKER".to_string()),
            Instruction::Pop,
            Instruction::Constant(Constant::Integer(3)),
            Instruction::Set("MARKER".to_string()),
            Instruction::Pop,
            Instruction::Load("__NCL_PROG1_VALUE_0".to_string()),
            Instruction::ExitScope,
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_prog2_with_discarded_first_value_and_empty_tail() {
    let program = compile("(prog2 1 2)");

    assert_eq!(
        program.functions[0].instructions,
        vec![
            Instruction::EnterScope,
            Instruction::Constant(Constant::Integer(1)),
            Instruction::Pop,
            Instruction::Constant(Constant::Integer(2)),
            Instruction::Define("__NCL_PROG2_VALUE_0".to_string()),
            Instruction::Pop,
            Instruction::Load("__NCL_PROG2_VALUE_0".to_string()),
            Instruction::ExitScope,
            Instruction::Return,
        ]
    );
}

#[test]
fn prog1_and_prog2_empty_tails_return_the_retained_form() {
    let prog1 = compile("(prog1 7)");
    let prog2 = compile("(prog2 7 8)");

    assert!(prog1.functions[0].instructions.iter().any(|instruction| {
        matches!(
            instruction,
            Instruction::Load(name) if name == "__NCL_PROG1_VALUE_0"
        )
    }));
    assert!(prog2.functions[0].instructions.iter().any(|instruction| {
        matches!(
            instruction,
            Instruction::Load(name) if name == "__NCL_PROG2_VALUE_0"
        )
    }));
    assert_eq!(
        prog1.functions[0]
            .instructions
            .iter()
            .filter(|instruction| matches!(instruction, Instruction::Constant(Constant::Nil)))
            .count(),
        0
    );
    assert_eq!(
        prog2.functions[0]
            .instructions
            .iter()
            .filter(|instruction| matches!(instruction, Instruction::Constant(Constant::Nil)))
            .count(),
        0
    );
}

#[test]
fn prog_temporary_names_avoid_source_collisions() {
    let program = compile("(let ((__ncl_prog1_value_0 7)) (prog1 1 __ncl_prog1_value_0))");
    let instructions = &program.functions[0].instructions;

    assert!(instructions.iter().any(|instruction| {
        matches!(
            instruction,
            Instruction::Define(name) if name == "__NCL_PROG1_VALUE_0"
        )
    }));
    assert!(instructions.iter().any(|instruction| {
        matches!(
            instruction,
            Instruction::Define(name) if name == "__NCL_PROG1_VALUE_1"
        )
    }));
    assert!(instructions.iter().any(|instruction| {
        matches!(
            instruction,
            Instruction::Load(name) if name == "__NCL_PROG1_VALUE_0"
        )
    }));
    assert!(instructions.iter().any(|instruction| {
        matches!(
            instruction,
            Instruction::Load(name) if name == "__NCL_PROG1_VALUE_1"
        )
    }));
}

#[test]
fn validates_prog1_and_prog2_arity_with_source_spans() {
    let prog1_form = read("(prog1)").expect("test source should parse")[0].clone();
    let prog1_error = Compiler::compile_forms(std::slice::from_ref(&prog1_form))
        .expect_err("PROG1 without a form should fail");
    assert_eq!(prog1_error.span, prog1_form.span);
    assert_eq!(
        prog1_error.kind,
        CompileErrorKind::Arity {
            operator: "PROG1".to_string(),
            expected: "at least one".to_string(),
            actual: 0,
        }
    );

    let prog2_form = read("(prog2 1)").expect("test source should parse")[0].clone();
    let prog2_error = Compiler::compile_forms(std::slice::from_ref(&prog2_form))
        .expect_err("PROG2 without a second form should fail");
    assert_eq!(prog2_error.span, prog2_form.span);
    assert_eq!(
        prog2_error.kind,
        CompileErrorKind::Arity {
            operator: "PROG2".to_string(),
            expected: "at least two".to_string(),
            actual: 1,
        }
    );
}

#[test]
fn rejects_non_symbol_bindings_without_panicking() {
    let form = Form::list(
        vec![
            Form::atom("let", Span::new(0, 3)),
            Form::list(
                vec![Form::list(
                    vec![Form::new(
                        FormKind::String("not-a-name".to_string()),
                        Span::new(7, 18),
                    )],
                    Span::new(6, 19),
                )],
                Span::new(5, 20),
            ),
        ],
        Span::new(0, 21),
    );

    let error = Compiler::compile_form(&form).expect_err("invalid binding should fail");
    assert_eq!(error.span, Span::new(7, 18));
    assert!(matches!(
        error.kind,
        CompileErrorKind::ExpectedSymbol { .. }
    ));
}

#[test]
fn emits_quoted_vectors_as_data() {
    let program = compile("#(a 1)");
    let forms = read("#(a 1)").expect("test source should parse");

    assert_eq!(
        program.functions[0].instructions,
        vec![Instruction::Quote(forms[0].clone()), Instruction::Return]
    );
}

#[test]
fn emits_control_flow_and_dynamic_binding_instructions() {
    let program = compile(
        "(defvar answer 1)
         (when t (setq answer 2))
         (unless nil (setq answer 3))
         (cond ((= answer 3)))",
    );
    let instructions = &program.functions[0].instructions;

    assert!(instructions.iter().any(|instruction| {
        matches!(instruction, Instruction::IsBound(name) if name == "ANSWER")
    }));
    assert!(
        instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Set(name) if name == "ANSWER"))
    );
    assert!(
        instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::JumpIfFalse(_)))
    );
}

#[test]
fn lowers_case_to_eql_comparisons_and_jumps() {
    let program = compile("(case value ((1 2) :one) (otherwise :other))");
    let instructions = &program.functions[0].instructions;

    assert!(instructions.iter().any(|instruction| {
        matches!(instruction, Instruction::FunctionLoad(name) if name == "EQL")
    }));
    assert!(instructions.iter().any(|instruction| {
        matches!(instruction, Instruction::Define(name) if name.starts_with("__NCL_CASE_KEY_"))
    }));
    assert!(instructions.iter().any(|instruction| {
        matches!(instruction, Instruction::Quote(form) if matches!(&form.kind, FormKind::Atom(atom) if atom == "1"))
    }));
    assert!(
        instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::JumpIfFalse(_)))
    );
    assert!(
        instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::ExitScope))
    );
}

#[test]
fn lowers_typecase_to_typep_comparisons_and_jumps() {
    let program = compile("(TYPECASE value (INTEGER :integer) (otherwise :other))");
    let instructions = &program.functions[0].instructions;

    assert!(instructions.iter().any(|instruction| {
        matches!(instruction, Instruction::FunctionLoad(name) if name == "TYPEP")
    }));
    assert!(instructions.iter().any(|instruction| {
        matches!(instruction, Instruction::Define(name) if name.starts_with("__NCL_TYPECASE_KEY_"))
    }));
    assert!(instructions.iter().any(|instruction| {
        matches!(instruction, Instruction::Quote(form) if matches!(&form.kind, FormKind::Atom(atom) if atom == "INTEGER"))
    }));
    assert!(
        instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::JumpIfFalse(_)))
    );
    assert!(
        instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::ExitScope))
    );
}

#[test]
fn lowers_error_case_variants_without_default_clauses() {
    let ecase = compile("(ecase value (INTEGER :integer))");
    assert!(ecase.functions[0].instructions.iter().any(|instruction| {
        matches!(instruction, Instruction::FunctionLoad(name) if name == "__NCL_ECASE_ERROR")
    }));

    let etypecase = compile("(etypecase value (INTEGER :integer))");
    assert!(etypecase.functions[0].instructions.iter().any(|instruction| {
        matches!(instruction, Instruction::FunctionLoad(name) if name == "__NCL_ETYPECASE_ERROR")
    }));
}

#[test]
fn rejects_malformed_case_and_typecase_clauses_from_table_cases() {
    let cases = [
        ("case without key", "(case)"),
        ("case non-list clause", "(case value 1)"),
        ("case empty clause", "(case value ())"),
        ("typecase without value", "(typecase)"),
        ("typecase non-list clause", "(typecase value integer)"),
        ("typecase empty clause", "(typecase value ())"),
    ];

    for (name, source) in cases {
        let forms = read(source).expect(name);
        let result = Compiler::compile_forms(&forms);
        assert!(result.is_err(), "{name}: {source}");
    }
}

#[test]
fn lowers_multiple_value_list_to_a_value_carrier_conversion() {
    let program = compile("(multiple-value-list (values 1 2))");

    assert_eq!(
        program.functions[0].instructions,
        vec![
            Instruction::Constant(Constant::Integer(1)),
            Instruction::Primary,
            Instruction::Constant(Constant::Integer(2)),
            Instruction::Primary,
            Instruction::Values(2),
            Instruction::MultipleValueList,
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_nth_value_to_a_native_value_selection() {
    let program = compile("(nth-value 1 (values 10 20))");

    assert_eq!(
        program.functions[0].instructions,
        vec![
            Instruction::Constant(Constant::Integer(1)),
            Instruction::Constant(Constant::Integer(10)),
            Instruction::Primary,
            Instruction::Constant(Constant::Integer(20)),
            Instruction::Primary,
            Instruction::Values(2),
            Instruction::NthValue,
            Instruction::Return,
        ]
    );
}

#[test]
fn compiles_control_form_matrix() {
    let cases = [
        "(catch 'tag (throw 'tag 1))",
        "(unwind-protect 1 2)",
        "(block done (return-from done 1))",
        "(tagbody start (go end) end)",
        "(multiple-value-bind (a b) (values 1 2) (+ a b))",
        "(multiple-value-call #'+ (values 1) (values 2))",
        "(multiple-value-prog1 1 2)",
        "(progv '(name) '(1) name)",
        "(handler-case 1 (error (condition) condition))",
        "(ignore-errors (error 'error))",
    ];

    for source in cases {
        let program = compile(source);
        assert!(
            !program.functions.is_empty(),
            "compiled no functions: {source}"
        );
    }
}

#[test]
fn emits_quasiquote_and_apply_instructions() {
    let quasiquote = compile("`(item ,value)");
    assert!(matches!(
        quasiquote.functions[0].instructions.first(),
        Some(Instruction::QuasiQuote(_))
    ));

    let apply = compile("(apply + 1 '(2))");
    assert!(
        apply.functions[0]
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Apply(2)))
    );
}

#[test]
fn emits_eval_and_mapcar_instructions() {
    let eval = compile("(eval '(+ 1 2))");
    assert!(matches!(
        eval.functions[0].instructions.as_slice(),
        [
            Instruction::Quote(_),
            Instruction::Eval(_),
            Instruction::Return
        ]
    ));

    for operation in ["MAPCAR", "MAPC", "MAPL", "MAPLIST", "MAPCAN", "MAPCON"] {
        let source = format!("({operation} + '(1 2) '(10 20))");
        let program = compile(&source);
        assert!(
            program.functions[0]
                .instructions
                .iter()
                .any(|instruction| {
                    matches!(instruction, Instruction::ListMapping { operation: emitted, sequence_count: 2 } if emitted == operation)
                }),
            "missing native instruction for {operation}"
        );
    }

    for operation in ["EVERY", "SOME", "NOTANY", "NOTEVERY"] {
        let source = format!("({operation} #'numberp '(1 2) '(3 4))");
        let program = compile(&source);
        assert!(
            program.functions[0].instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::SequenceQuantifier { operation: emitted, sequence_count: 2 }
                    if emitted == operation
            )),
            "missing native instruction for {operation}"
        );
    }

    let map = compile("(map 'list #'numberp '(1 2) '(3 4))");
    assert!(map.functions[0].instructions.iter().any(|instruction| {
        matches!(instruction, Instruction::SequenceMapping { sequence_count: 2 })
    }));

    let map_into = compile("(map-into result #'1+ '(1 2))");
    assert!(map_into.functions[0].instructions.iter().any(|instruction| {
        matches!(instruction, Instruction::SequenceMapInto { sequence_count: 1 })
    }));
    let reduce = compile("(reduce #'+ '(1 2 3) :initial-value 10)");
    assert!(reduce.functions[0].instructions.iter().any(|instruction| {
        matches!(instruction, Instruction::SequenceReduce { option_count: 2 })
    }));
    let merge = compile("(merge 'list '(1 3) '(2 4) #'< :key #'identity)");
    assert!(merge.functions[0].instructions.iter().any(|instruction| {
        matches!(instruction, Instruction::SequenceMerge { option_count: 2 })
    }));
    assert!(
        map_into.functions[0]
            .instructions
            .iter()
            .any(|instruction| {
                matches!(
                    instruction,
                    Instruction::MapIntoSetfSymbol { name, escaped: false } if name == "RESULT"
                )
            })
    );

    let symbol_value = compile("(setf (symbol-value symbol) 1)");
    assert!(symbol_value.functions[0].instructions.iter().any(|instruction| {
        matches!(instruction, Instruction::SetfSymbolCellDynamic { operator } if operator == "SYMBOL-VALUE")
    }));
}

#[test]
fn lowers_dotimes_with_a_single_count_evaluation_and_result() {
    let program = compile("(dotimes (i (+ 1 1) (+ i 10)) (+ i 1))");

    assert_eq!(
        program.functions[0].instructions,
        vec![
            Instruction::EnterScope,
            Instruction::FunctionLoad("+".to_string()),
            Instruction::Constant(Constant::Integer(1)),
            Instruction::Constant(Constant::Integer(1)),
            Instruction::Call(2),
            Instruction::Define("__NCL_DOTIMES_LIMIT_0".to_string()),
            Instruction::Pop,
            Instruction::Constant(Constant::Integer(0)),
            Instruction::Define("I".to_string()),
            Instruction::Pop,
            Instruction::FunctionLoad("<".to_string()),
            Instruction::Load("I".to_string()),
            Instruction::Load("__NCL_DOTIMES_LIMIT_0".to_string()),
            Instruction::Call(2),
            Instruction::JumpIfFalse(27),
            Instruction::FunctionLoad("+".to_string()),
            Instruction::Load("I".to_string()),
            Instruction::Constant(Constant::Integer(1)),
            Instruction::Call(2),
            Instruction::Pop,
            Instruction::FunctionLoad("+".to_string()),
            Instruction::Load("I".to_string()),
            Instruction::Constant(Constant::Integer(1)),
            Instruction::Call(2),
            Instruction::Set("I".to_string()),
            Instruction::Pop,
            Instruction::Jump(10),
            Instruction::FunctionLoad("+".to_string()),
            Instruction::Load("I".to_string()),
            Instruction::Constant(Constant::Integer(10)),
            Instruction::Call(2),
            Instruction::ExitScope,
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_empty_dotimes_body_with_a_default_result() {
    let program = compile("(dotimes (i 0))");

    assert_eq!(
        program.functions[0].instructions,
        vec![
            Instruction::EnterScope,
            Instruction::Constant(Constant::Integer(0)),
            Instruction::Define("__NCL_DOTIMES_LIMIT_0".to_string()),
            Instruction::Pop,
            Instruction::Constant(Constant::Integer(0)),
            Instruction::Define("I".to_string()),
            Instruction::Pop,
            Instruction::FunctionLoad("<".to_string()),
            Instruction::Load("I".to_string()),
            Instruction::Load("__NCL_DOTIMES_LIMIT_0".to_string()),
            Instruction::Call(2),
            Instruction::JumpIfFalse(21),
            Instruction::Constant(Constant::Nil),
            Instruction::Pop,
            Instruction::FunctionLoad("+".to_string()),
            Instruction::Load("I".to_string()),
            Instruction::Constant(Constant::Integer(1)),
            Instruction::Call(2),
            Instruction::Set("I".to_string()),
            Instruction::Pop,
            Instruction::Jump(7),
            Instruction::Constant(Constant::Nil),
            Instruction::ExitScope,
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_dolist_with_endp_car_cdr_and_multiple_elements() {
    let program = compile("(dolist (item (list 1 2) item) (+ item 1))");

    assert_eq!(
        program.functions[0].instructions,
        vec![
            Instruction::EnterScope,
            Instruction::FunctionLoad("LIST".to_string()),
            Instruction::Constant(Constant::Integer(1)),
            Instruction::Constant(Constant::Integer(2)),
            Instruction::Call(2),
            Instruction::Define("__NCL_DOLIST_TAIL_0".to_string()),
            Instruction::Pop,
            Instruction::Constant(Constant::Nil),
            Instruction::Define("ITEM".to_string()),
            Instruction::Pop,
            Instruction::FunctionLoad("ENDP".to_string()),
            Instruction::Load("__NCL_DOLIST_TAIL_0".to_string()),
            Instruction::Call(1),
            Instruction::JumpIfFalse(15),
            Instruction::Jump(31),
            Instruction::FunctionLoad("CAR".to_string()),
            Instruction::Load("__NCL_DOLIST_TAIL_0".to_string()),
            Instruction::Call(1),
            Instruction::Set("ITEM".to_string()),
            Instruction::Pop,
            Instruction::FunctionLoad("+".to_string()),
            Instruction::Load("ITEM".to_string()),
            Instruction::Constant(Constant::Integer(1)),
            Instruction::Call(2),
            Instruction::Pop,
            Instruction::FunctionLoad("CDR".to_string()),
            Instruction::Load("__NCL_DOLIST_TAIL_0".to_string()),
            Instruction::Call(1),
            Instruction::Set("__NCL_DOLIST_TAIL_0".to_string()),
            Instruction::Pop,
            Instruction::Jump(10),
            Instruction::Constant(Constant::Nil),
            Instruction::Set("ITEM".to_string()),
            Instruction::Pop,
            Instruction::Load("ITEM".to_string()),
            Instruction::ExitScope,
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_do_with_implicit_block_parallel_steps_and_tagbody() {
    let program = compile(
        "(do ((i 0 (1+ i)) (j 0 i))
             ((= i 2) j)
           (go done)
           done)",
    );

    assert!(program.functions[0].instructions.iter().any(|instruction| {
        matches!(
            instruction,
            Instruction::Block { name, .. } if name == "NIL"
        )
    }));
    assert!(program.functions.iter().any(|function| {
        function
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::TagBody { .. }))
    }));
    assert!(program.functions.iter().any(|function| {
        function
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::JumpIfFalse(_)))
    }));
    assert!(program.functions.iter().any(|function| {
        function
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Set(name) if name == "I"))
    }));
}

#[test]
fn lowers_prog_with_implicit_block_and_tagbody() {
    let program = compile("(prog ((i 0)) start (go done) done)");

    assert!(program.functions[0].instructions.iter().any(|instruction| {
        matches!(
            instruction,
            Instruction::Block { name, .. } if name == "NIL"
        )
    }));
    assert!(program.functions.iter().any(|function| {
        function
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::TagBody { .. }))
    }));
    assert!(program.functions.iter().any(|function| {
        function
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Go { tag } if tag == "DONE"))
    }));
    assert!(program.functions.iter().any(|function| {
        function
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Define(name) if name == "I"))
    }));
}

#[test]
fn lowers_sequential_prog_bindings_without_losing_initializer_scope() {
    let program = compile("(prog* ((first 1) (second first)) done (go done))");

    assert!(program.functions.iter().any(|function| {
        function
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Load(name) if name == "FIRST"))
    }));
    assert!(program.functions.iter().any(|function| {
        function
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Define(name) if name == "SECOND"))
    }));
    assert!(program.functions.iter().any(|function| {
        function
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Go { tag } if tag == "DONE"))
    }));
}

#[test]
fn lowers_prog_bindings_without_initializers_to_nil() {
    let program = compile("(prog (value) done)");

    assert!(program.functions.iter().any(|function| {
        function
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Constant(Constant::Nil)))
    }));
    assert!(program.functions.iter().any(|function| {
        function
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Define(name) if name == "VALUE"))
    }));
}

#[test]
fn rejects_invalid_prog_binding_shapes() {
    for source in [
        "(prog ((value 1 2)) done)",
        "(prog ((1 2)) done)",
        "(prog ((value 1) (value 2)) done)",
    ] {
        let forms = read(source).expect("test source should parse");
        assert!(Compiler::compile_forms(&forms).is_err(), "{source}");
    }
}

#[test]
fn avoids_loop_temporary_collisions_and_keeps_outer_bindings() {
    let program = compile(
        "(let ((__ncl_dotimes_limit_0 9))
           (progn (dotimes (i 0)) __ncl_dotimes_limit_0))",
    );
    let instructions = &program.functions[0].instructions;

    assert!(instructions.iter().any(|instruction| {
        matches!(
            instruction,
            Instruction::Define(name) if name == "__NCL_DOTIMES_LIMIT_0"
        )
    }));
    assert!(instructions.iter().any(|instruction| {
        matches!(
            instruction,
            Instruction::Define(name) if name == "__NCL_DOTIMES_LIMIT_1"
        )
    }));
    assert_eq!(
        &instructions[instructions.len() - 3..],
        [
            Instruction::Load("__NCL_DOTIMES_LIMIT_0".to_string()),
            Instruction::ExitScope,
            Instruction::Return,
        ]
    );
}

#[test]
fn compiles_tagbody_and_go_with_label_positions() {
    let program = compile("(tagbody start (go done) done)");

    assert_eq!(
        program.functions[0].instructions,
        vec![
            Instruction::TagBody {
                function: 1,
                tags: vec![("START".to_owned(), 0), ("DONE".to_owned(), 2)],
            },
            Instruction::Return,
        ]
    );
    assert_eq!(
        program.functions[1].instructions,
        vec![
            Instruction::Go {
                tag: "DONE".to_owned(),
            },
            Instruction::Pop,
            Instruction::Constant(Constant::Nil),
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_multiple_value_control_forms() {
    let program = compile(
        "(multiple-value-bind (first second) (values 1 2) (+ first second))
         (multiple-value-call + (values 1 2) (values 3 4))
         (multiple-value-prog1 (values 1 2) (values 3 4))",
    );
    let instructions = &program.functions[0].instructions;

    assert!(instructions
        .iter()
        .any(|instruction| matches!(instruction, Instruction::BindValues(names) if names == &["FIRST".to_owned(), "SECOND".to_owned()])));
    assert!(
        instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::MultipleValueCall(2)))
    );
    assert!(instructions
        .iter()
        .any(|instruction| matches!(instruction, Instruction::DefineValues(name) if name.starts_with("__NCL_MULTIPLE_VALUE_PROG1_VALUE"))));
}

#[test]
fn lowers_short_circuit_boolean_forms() {
    let program = compile("(and a b c) (or x y z) (and) (or)");
    let instructions = &program.functions[0].instructions;

    assert!(
        instructions
            .iter()
            .filter(|instruction| matches!(instruction, Instruction::Dup))
            .count()
            >= 3
    );
    assert!(
        instructions.iter().any(|instruction| matches!(
            instruction,
            Instruction::Constant(Constant::Boolean(true))
        ))
    );
    assert!(
        instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Constant(Constant::Nil)))
    );
}

#[test]
fn lowers_conditional_control_forms_from_table_cases() {
    let cases = [
        ("(when flag value)", "WHEN"),
        ("(unless flag value)", "UNLESS"),
        ("(cond (flag value) (t fallback))", "COND"),
        ("(case key ((1 2) value) (otherwise fallback))", "CASE"),
        ("(ecase key (1 value))", "ECASE"),
        (
            "(typecase value (integer result) (otherwise fallback))",
            "TYPECASE",
        ),
        ("(etypecase value (integer result))", "ETYPECASE"),
    ];

    for (source, operator) in cases {
        let program = compile(source);
        assert!(
            program.functions[0]
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::Jump(_))),
            "{operator} should lower its branch exit: {source}"
        );
    }
}

#[test]
fn lowers_conditional_edge_cases_from_table_cases() {
    let cases = [
        ("(cond (flag))", "COND"),
        ("(cond (flag value))", "COND"),
        ("(case key (1) (otherwise))", "CASE"),
        ("(case key ((1 2) value))", "CASE"),
        ("(typecase value (integer))", "TYPECASE"),
        ("(etypecase value (integer))", "ETYPECASE"),
    ];

    for (source, operator) in cases {
        let program = compile(source);
        assert!(
            !program.functions[0].instructions.is_empty(),
            "{operator}: {source}"
        );
    }
}

#[test]
fn rejects_malformed_conditional_control_forms_from_table_cases() {
    for source in [
        "(when)",
        "(unless)",
        "(cond 1)",
        "(cond ())",
        "(case)",
        "(case 1 2)",
        "(case 1 ())",
        "(ecase)",
        "(ecase 1 2)",
        "(ecase 1 ())",
        "(typecase)",
        "(typecase 1 2)",
        "(typecase 1 ())",
        "(etypecase)",
        "(etypecase 1 2)",
        "(etypecase 1 ())",
    ] {
        let forms = read(source).expect("test source should parse");
        assert!(Compiler::compile_forms(&forms).is_err(), "{source}");
    }
}

#[test]
fn rejects_runtime_definition_forms_without_a_name() {
    for source in [
        "(defstruct)",
        "(defvar)",
        "(defparameter)",
        "(defconstant)",
        "(eval-when)",
        "(define)",
        "(defun)",
    ] {
        let forms = read(source).expect("test source should parse");
        assert!(Compiler::compile_forms(&forms).is_err(), "{source}");
    }
}

#[test]
fn rejects_non_symbol_names_across_definition_and_control_forms() {
    for source in ["(defvar :keyword 2)", "(defun 1 () nil)", "(block 1 nil)"] {
        let forms = read(source).expect("test source should parse");
        assert!(Compiler::compile_forms(&forms).is_err(), "{source}");
    }
}

#[test]
fn rejects_malformed_prog_and_condition_handler_forms() {
    for source in [
        "(prog)",
        "(prog 1 nil)",
        "(prog ((value 1 2)) nil)",
        "(prog ((value 1) (VALUE 2)) nil)",
        "(prog (1) nil)",
        "(handler-case 1)",
        "(handler-case 1 2)",
        "(handler-case 1 error)",
        "(handler-case 1 (error 1))",
        "(handler-case 1 (error (first second) nil))",
        "(handler-bind 1)",
        "(handler-bind (error) nil)",
        "(handler-bind ((error)) nil)",
        "(handler-bind ((error handler extra)) nil)",
        "(restart-bind)",
        "(restart-bind 1 nil)",
        "(restart-bind (error) nil)",
        "(restart-bind ((abort)) nil)",
        "(restart-bind ((abort handler extra)) nil)",
        "(restart-bind ((1 (lambda () nil))) nil)",
        "(catch)",
        "(with-simple-restart)",
        "(with-simple-restart abort nil)",
        "(with-simple-restart (abort) nil)",
        "(with-simple-restart (1 \"abort\") nil)",
        "(restart-case 1)",
        "(restart-case 1 (abort))",
        "(restart-case 1 abort)",
        "(restart-case 1 (1 () nil))",
        "(with-condition-restarts nil nil)",
        "(with-open-file)",
        "(with-open-file 1 nil)",
        "(with-open-file (stream))",
        "(with-open-file (1 \"path\") nil)",
        "(throw 1)",
        "(throw 1 2 3)",
        "(progv)",
        "(progv nil)",
        "(multiple-value-list)",
        "(multiple-value-list 1 2)",
    ] {
        let forms = read(source).expect("test source should parse");
        assert!(Compiler::compile_forms(&forms).is_err(), "{source}");
    }
}

#[test]
fn rejects_malformed_iteration_specs_from_table_cases() {
    for source in [
        "(dotimes item)",
        "(dotimes ())",
        "(dotimes (index))",
        "(dotimes (index 1 2 3))",
        "(dotimes (1 2))",
        "(dolist item)",
        "(dolist ())",
        "(dolist (item))",
        "(dolist (item values extra result))",
        "(dolist (1 values))",
        "(do bindings)",
        "(do (1) ((= 1 1)))",
        "(do () termination)",
        "(do () ())",
    ] {
        let forms = read(source).expect("test source should parse");
        assert!(Compiler::compile_forms(&forms).is_err(), "{source}");
    }
}

#[test]
fn lowers_dynamic_condition_restart_and_catch_forms() {
    let program = compile(
        "(handler-case (error \"boom\") (error (condition) nil))
         (handler-bind ((error (lambda (condition) condition))) nil)
         (restart-bind ((abort (lambda () nil))) (invoke-restart 'abort))
         (catch 'tag (throw 'tag 42))
         (with-simple-restart (abort \"abort\") nil)
         (with-condition-restarts nil nil nil)
         (with-open-file (stream \"/tmp/ncl-compiler-test\") stream)
         (restart-case (invoke-restart 'abort) (abort () nil))
         (progv '(name) '(value) name)",
    );
    let instructions = &program.functions[0].instructions;

    assert!(instructions
        .iter()
        .any(|instruction| matches!(instruction, Instruction::HandlerCase { clauses, .. } if clauses.len() == 1)));
    assert!(instructions
        .iter()
        .any(|instruction| matches!(instruction, Instruction::HandlerBind { handlers, .. } if handlers.len() == 1)));
    assert!(instructions
        .iter()
        .any(|instruction| matches!(instruction, Instruction::RestartBind { bindings, .. } if bindings.len() == 1)));
    assert!(
        instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Catch { .. }))
    );
    assert!(instructions
        .iter()
        .any(|instruction| matches!(instruction, Instruction::WithSimpleRestart { name, .. } if name == "ABORT")));
    assert!(
        instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::WithConditionRestarts { .. }))
    );
    assert!(instructions
        .iter()
        .any(|instruction| matches!(instruction, Instruction::RestartCase { clauses, .. } if clauses.len() == 1)));
    assert!(
        instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Progv { .. }))
    );
}

#[test]
fn lowers_do_star_with_omitted_init_and_escaped_stepped_variable() {
    let program = compile("(do* ((i) (|Mixed| 1 (1+ |Mixed|))) ((> |Mixed| 0) |Mixed|) nil)");

    assert!(program.functions.iter().any(|function| {
        function
            .instructions
            .windows(2)
            .any(|window| matches!(window, [Instruction::Constant(Constant::Nil), Instruction::Define(name)] if name == "I"))
    }), "missing DO* variable should default its init to NIL");
    assert!(program.functions.iter().any(|function| {
        function
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::DefineExact(name) if name == "Mixed"))
    }), "escaped DO* variable should bind with DefineExact");
    assert!(
        program.functions.iter().any(|function| {
            function.instructions.iter().any(
                |instruction| matches!(instruction, Instruction::SetExact(name) if name == "Mixed"),
            )
        }),
        "escaped DO* stepped variable should update with SetExact"
    );
}

#[test]
fn lowers_do_with_omitted_init_and_escaped_stepped_variable() {
    let program = compile("(do ((i) (|Mixed| 1 (1+ |Mixed|))) ((> |Mixed| 0) |Mixed|) nil)");

    assert!(
        program.functions.iter().any(|function| {
            function.instructions.windows(2).any(|window| {
                matches!(
                    window,
                    [Instruction::Constant(Constant::Nil), Instruction::Define(name)]
                        if name.starts_with("__NCL_DO_INIT_")
                )
            })
        }),
        "missing DO variable should default its init temporary to NIL"
    );
    assert!(program.functions.iter().any(|function| {
        function
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::DefineExact(name) if name == "Mixed"))
    }), "escaped DO variable should bind with DefineExact");
    assert!(
        program.functions.iter().any(|function| {
            function.instructions.iter().any(
                |instruction| matches!(instruction, Instruction::SetExact(name) if name == "Mixed"),
            )
        }),
        "escaped DO stepped variable should update with SetExact"
    );
}

#[test]
fn lowers_dolist_without_a_result_form_to_nil() {
    let program = compile("(dolist (item (list 1)) item)");
    let instructions = &program.functions[0].instructions;

    assert_eq!(
        &instructions[instructions.len() - 6..],
        [
            Instruction::Constant(Constant::Nil),
            Instruction::Set("ITEM".to_string()),
            Instruction::Pop,
            Instruction::Constant(Constant::Nil),
            Instruction::ExitScope,
            Instruction::Return,
        ]
    );
}

#[test]
fn rejects_malformed_multiple_value_control_forms_from_table_cases() {
    for source in [
        "(multiple-value-bind (a))",
        "(multiple-value-bind (1) (values 1) nil)",
        "(multiple-value-call)",
        "(multiple-value-prog1)",
    ] {
        let forms = read(source).expect("test source should parse");
        assert!(Compiler::compile_forms(&forms).is_err(), "{source}");
    }
}

#[test]
fn lowers_multiple_value_prog1_with_an_empty_tail() {
    let program = compile("(multiple-value-prog1 1)");
    let instructions = &program.functions[0].instructions;

    assert_eq!(instructions.len(), 7, "{instructions:?}");
    assert!(matches!(instructions[0], Instruction::EnterScope));
    assert!(matches!(
        instructions[1],
        Instruction::Constant(Constant::Integer(1))
    ));
    assert!(matches!(instructions[2], Instruction::DefineValues(_)));
    assert!(matches!(instructions[3], Instruction::Pop));
    assert!(matches!(instructions[4], Instruction::Load(_)));
    assert!(matches!(instructions[5], Instruction::ExitScope));
    assert!(matches!(instructions[6], Instruction::Return));
}

#[test]
fn rejects_malformed_block_and_return_forms_from_table_cases() {
    for source in ["(block)", "(return 1 2)", "(return-from name 1 2)"] {
        let forms = read(source).expect("test source should parse");
        assert!(Compiler::compile_forms(&forms).is_err(), "{source}");
    }
}

#[test]
fn lowers_return_from_without_a_value_to_nil() {
    let program = compile("(block done (return-from done))");

    assert_eq!(
        program.functions[1].instructions,
        vec![
            Instruction::Constant(Constant::Nil),
            Instruction::ReturnFrom {
                name: "DONE".to_string(),
            },
            Instruction::Return,
        ]
    );
}
