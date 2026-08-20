use ncl_compiler::{CompileErrorKind, Compiler, Constant, DestructureSpec, Instruction};
use ncl_syntax::{Form, FormKind, Span, read};

fn compile(source: &str) -> ncl_compiler::Program {
    let forms = read(source).expect("test source should parse");
    Compiler::compile_forms(&forms).expect("test source should compile")
}

fn compile_error_message(source: &str) -> String {
    let forms = read(source).expect("test source should parse");
    match Compiler::compile_forms(&forms) {
        Ok(program) => panic!("test source should fail to compile: {source}: {program:?}"),
        Err(error) => error.kind.to_string(),
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
fn compiles_destructuring_lambda_lists_and_rejects_malformed_sections() {
    let program = compile(
        "(destructuring-bind (&whole whole (first second)
            &optional (third (+ first 1) third-p)
            &rest rest
            &key ((:scale scale) scale-value scale-p) (limit 10)
            &allow-other-keys
            &aux (total (+ first third)))
            (list (list 1 2) 3 :scale 5 :extra 8)
            (list whole first second third third-p rest scale-value scale-p limit total))",
    );
    let Some(Instruction::Destructure(DestructureSpec::LambdaList(lambda_list))) = program
        .functions[0]
        .instructions
        .iter()
        .find(|instruction| matches!(instruction, Instruction::Destructure(_)))
    else {
        panic!("destructuring-bind should emit a lambda-list instruction");
    };
    assert_eq!(lambda_list.whole.as_deref(), Some("WHOLE"));
    assert_eq!(lambda_list.required.len(), 1);
    assert_eq!(lambda_list.optional.len(), 1);
    assert_eq!(
        lambda_list.optional[0].supplied_p.as_deref(),
        Some("THIRD-P")
    );
    assert_eq!(lambda_list.rest.as_deref(), Some("REST"));
    assert_eq!(lambda_list.keywords.len(), 2);
    assert_eq!(lambda_list.keywords[0].keyword_name, "SCALE");
    assert_eq!(
        lambda_list.keywords[0].supplied_p.as_deref(),
        Some("SCALE-P")
    );
    assert!(lambda_list.allow_other_keys);
    assert_eq!(lambda_list.auxiliary[0].name, "TOTAL");

    for (source, expected) in [
        (
            "(destructuring-bind (first))",
            "DESTRUCTURING-BIND expected two or more arguments, received 1",
        ),
        (
            "(destructuring-bind (&whole) value body)",
            "&whole must be the first marker followed by one parameter",
        ),
        (
            "(destructuring-bind (first &whole whole) value body)",
            "&whole must be the first marker followed by one parameter",
        ),
        (
            "(destructuring-bind (first &optional second &optional third) value body)",
            "&optional is out of order in destructuring lambda list",
        ),
        (
            "(destructuring-bind (first &rest) value body)",
            "&rest or &body must be followed by one parameter",
        ),
        (
            "(destructuring-bind (first &rest 1) value body)",
            "destructuring rest parameter name must be a symbol",
        ),
        (
            "(destructuring-bind (first &rest rest another) value body)",
            "destructuring rest parameter must be followed by a keyword or auxiliary section",
        ),
        (
            "(destructuring-bind (first &rest rest &rest more) value body)",
            "&rest or &body must be followed by one parameter",
        ),
        (
            "(destructuring-bind (first &key &key other) value body)",
            "&key is out of order or repeated in destructuring lambda list",
        ),
        (
            "(destructuring-bind (first &allow-other-keys) value body)",
            "&allow-other-keys requires a keyword section",
        ),
        (
            "(destructuring-bind (first &key key &allow-other-keys other) value body)",
            "&allow-other-keys must be the last keyword-list marker",
        ),
        (
            "(destructuring-bind (first &aux x &aux y) value body)",
            "&aux is repeated in destructuring lambda list",
        ),
        (
            "(destructuring-bind (first &unknown x) value body)",
            "unsupported marker in destructuring lambda list",
        ),
        (
            "(destructuring-bind ((&unknown)) value body)",
            "unsupported marker in destructuring lambda list",
        ),
        (
            "(destructuring-bind (#(first)) value body)",
            "destructuring pattern must be a symbol or list",
        ),
        (
            "(destructuring-bind (first &optional (second 1 2 3)) value body)",
            "destructuring optional parameter must contain one to three items",
        ),
        (
            "(destructuring-bind (first &optional (second . third)) value body)",
            "destructuring optional parameter must be a symbol or list",
        ),
        (
            "(destructuring-bind (first &key ((:scale) scale-value)) value body)",
            "destructuring keyword designator must contain a keyword and variable",
        ),
        (
            "(destructuring-bind (first &key ((scale scale) scale-value)) value body)",
            "destructuring keyword designator must start with a keyword",
        ),
        (
            "(destructuring-bind (first &key (:scale)) value body)",
            "destructuring keyword parameter needs a variable",
        ),
        (
            "(destructuring-bind (first &key ((nested . value))) value body)",
            "destructuring keyword parameter must have a variable name",
        ),
        (
            "(destructuring-bind (first &key (scale 1 2 3)) value body)",
            "destructuring keyword parameter contains too many items",
        ),
        (
            "(destructuring-bind (first &key (scale 1 2)) value body)",
            "destructuring supplied-p name must be a symbol",
        ),
        (
            "(destructuring-bind (first &key (scale . rest)) value body)",
            "destructuring keyword parameter must be a symbol or list",
        ),
        (
            "(destructuring-bind (first &key ((:scale second) 1) ((:scale third) 2)) value body)",
            "destructuring keyword names must be unique",
        ),
        (
            "(destructuring-bind (first &aux (total 1 2)) value body)",
            "destructuring auxiliary parameter must contain one or two items",
        ),
        (
            "(destructuring-bind (first &aux (total . rest)) value body)",
            "destructuring auxiliary parameter must be a symbol or list",
        ),
        (
            "(destructuring-bind (first first) value body)",
            "destructuring pattern names must be unique",
        ),
    ] {
        assert_eq!(compile_error_message(source), expected, "{source}");
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
fn lowers_defconstant_without_runtime_eval_fallback() {
    let program = compile("(defconstant +answer+ 42)");
    let instructions = &program.functions[0].instructions;

    assert!(
        instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::CheckConstant(_)))
    );
    assert!(
        instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::DefineConstant(_)))
    );
    assert!(
        !instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Eval(_)))
    );
}

#[test]
fn lowers_nth_value_to_native_multiple_value_selection() {
    let program = compile("(nth-value 1 (values 10 20))");
    let instructions = &program.functions[0].instructions;

    assert!(
        instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::NthValue(_)))
    );
    assert!(
        !instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Eval(_)))
    );
}

#[test]
fn lowers_load_time_value_without_runtime_eval_fallback() {
    let program = compile("(load-time-value (values 10 20) (progn 1))");
    let instructions = &program.functions[0].instructions;

    assert!(
        instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::LoadTimeValue))
    );
    assert!(
        !instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Eval(_)))
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
            Instruction::DefineValues("__NCL_PROG1_VALUE_0".to_string()),
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
            Instruction::DefineValues("__NCL_PROG2_VALUE_0".to_string()),
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
            Instruction::DefineValues(name) if name == "__NCL_PROG1_VALUE_1"
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
fn emits_bit_vectors_as_vector_data() {
    let program = compile("#*101");
    let forms = read("#*101").expect("test source should parse");

    assert_eq!(
        program.functions[0].instructions,
        vec![Instruction::Quote(forms[0].clone()), Instruction::Return]
    );
}

#[test]
fn emits_radix_integer_literals_as_numbers() {
    let program = compile("#xff");

    assert_eq!(
        program.functions[0].instructions,
        vec![
            Instruction::Constant(Constant::Integer(255)),
            Instruction::Return
        ]
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

    let mapcar = compile("(mapcar + '(1 2) '(10 20))");
    assert!(
        mapcar.functions[0]
            .instructions
            .iter()
            .any(|instruction| { matches!(instruction, Instruction::MapCar(2)) })
    );
}

#[test]
fn lowers_dotimes_with_a_single_count_evaluation_and_result() {
    let program = compile("(dotimes (i (+ 1 1) (+ i 10)) (+ i 1))");

    assert_eq!(
        program.functions[0].instructions,
        vec![
            Instruction::EnterScope,
            Instruction::FunctionLoad("__NCL_REQUIRE_INTEGER".to_string()),
            Instruction::FunctionLoad("+".to_string()),
            Instruction::Constant(Constant::Integer(1)),
            Instruction::Constant(Constant::Integer(1)),
            Instruction::Call(2),
            Instruction::Call(1),
            Instruction::Define("__NCL_DOTIMES_LIMIT_0".to_string()),
            Instruction::Pop,
            Instruction::Constant(Constant::Integer(0)),
            Instruction::Define("I".to_string()),
            Instruction::Pop,
            Instruction::FunctionLoad("<".to_string()),
            Instruction::Load("I".to_string()),
            Instruction::Load("__NCL_DOTIMES_LIMIT_0".to_string()),
            Instruction::Call(2),
            Instruction::JumpIfFalse(29),
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
            Instruction::Jump(12),
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
            Instruction::FunctionLoad("__NCL_REQUIRE_INTEGER".to_string()),
            Instruction::Constant(Constant::Integer(0)),
            Instruction::Call(1),
            Instruction::Define("__NCL_DOTIMES_LIMIT_0".to_string()),
            Instruction::Pop,
            Instruction::Constant(Constant::Integer(0)),
            Instruction::Define("I".to_string()),
            Instruction::Pop,
            Instruction::FunctionLoad("<".to_string()),
            Instruction::Load("I".to_string()),
            Instruction::Load("__NCL_DOTIMES_LIMIT_0".to_string()),
            Instruction::Call(2),
            Instruction::JumpIfFalse(23),
            Instruction::Constant(Constant::Nil),
            Instruction::Pop,
            Instruction::FunctionLoad("+".to_string()),
            Instruction::Load("I".to_string()),
            Instruction::Constant(Constant::Integer(1)),
            Instruction::Call(2),
            Instruction::Set("I".to_string()),
            Instruction::Pop,
            Instruction::Jump(9),
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
            Instruction::FunctionLoad("__NCL_REQUIRE_LIST".to_string()),
            Instruction::FunctionLoad("LIST".to_string()),
            Instruction::Constant(Constant::Integer(1)),
            Instruction::Constant(Constant::Integer(2)),
            Instruction::Call(2),
            Instruction::Call(1),
            Instruction::Define("__NCL_DOLIST_TAIL_0".to_string()),
            Instruction::Pop,
            Instruction::Constant(Constant::Nil),
            Instruction::Define("ITEM".to_string()),
            Instruction::Pop,
            Instruction::FunctionLoad("ENDP".to_string()),
            Instruction::Load("__NCL_DOLIST_TAIL_0".to_string()),
            Instruction::Call(1),
            Instruction::JumpIfFalse(17),
            Instruction::Jump(33),
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
            Instruction::Jump(12),
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
