use ncl_hir::cl_type::NumericValue;
use ncl_hir::{
    BuiltinType, CallTarget, ClTypeKind, CompileTimeSituation, HirDeclaration, LocalFunctionKind,
    NodeKind,
};
use ncl_ir::{Constant, OpKind};
use ncl_object::Word;

use super::common::{TestResult, context, expand_build, lower_both_and_verify};

#[test]
fn empty_local_macro_environments_preserve_their_body_values() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(113);
    let macro_definitions = fixture.list(&[]);
    let macrolet = fixture.form("MACROLET", &[macro_definitions, Word::fixnum(31)]);
    let symbol_macro_definitions = fixture.list(&[]);
    let symbol_macrolet = fixture.form(
        "SYMBOL-MACROLET",
        &[symbol_macro_definitions, Word::fixnum(37)],
    );
    let input = fixture.form("PROGN", &[macrolet, symbol_macrolet]);
    let source = "(progn (macrolet () 31) (symbol-macrolet () 37))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    let implicit_progns = form
        .nodes()
        .iter()
        .filter(
            |(_, node)| matches!(&node.kind, NodeKind::Sequence { forms, .. } if forms.len() == 1),
        )
        .count();
    assert_eq!(implicit_progns, 2);

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    assert!(lowered.entry.constants.contains(&Constant::Fixnum(31)));
    assert!(lowered.entry.constants.contains(&Constant::Fixnum(37)));

    // CLHS specifies an implicit progn for MACROLET and SYMBOL-MACROLET bodies.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_flet_.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_symbol.htm
    Ok(())
}

#[test]
fn eval_when_retains_load_and_execute_situations_in_hir_and_mir() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(114);
    let load_situation = fixture.keyword("LOAD-TOPLEVEL");
    let load_situations = fixture.list(&[load_situation]);
    let load_only = fixture.form("EVAL-WHEN", &[load_situations, Word::fixnum(17)]);
    let execute_situation = fixture.keyword("EXECUTE");
    let execute_situations = fixture.list(&[execute_situation]);
    let execute = fixture.form("EVAL-WHEN", &[execute_situations, Word::fixnum(23)]);
    let input = fixture.form("PROGN", &[load_only, execute]);
    let source = "(progn (eval-when (:load-toplevel) 17) (eval-when (:execute) 23))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    let mut load_retained = false;
    let mut execute_retained = false;
    for (_, node) in form.nodes().iter() {
        if let NodeKind::CompileTimeForm { situations, .. } = &node.kind {
            load_retained |= situations.contains(&CompileTimeSituation::LoadToplevel);
            execute_retained |= situations.contains(&CompileTimeSituation::Execute);
        }
    }
    assert!(load_retained);
    assert!(execute_retained);

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    assert!(lowered.entry.constants.contains(&Constant::Nil));
    assert!(lowered.entry.constants.contains(&Constant::Fixnum(23)));
    assert!(!lowered.entry.constants.contains(&Constant::Fixnum(17)));

    // CLHS EVAL-WHEN executes non-top-level forms for :EXECUTE; :LOAD-TOPLEVEL is distinct.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_eval_w.htm
    Ok(())
}

#[test]
fn empty_let_and_local_function_groups_preserve_their_body_values() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(115);
    let bindings = fixture.list(&[]);
    let parallel_body = fixture.form("LET", &[bindings, Word::fixnum(41)]);
    let bindings = fixture.list(&[]);
    let sequential_body = fixture.form("LET*", &[bindings, Word::fixnum(43)]);
    let definitions = fixture.list(&[]);
    let local_defs_body = fixture.form("FLET", &[definitions, Word::fixnum(47)]);
    let definitions = fixture.list(&[]);
    let recursive_defs_body = fixture.form("LABELS", &[definitions, Word::fixnum(53)]);
    let input = fixture.form(
        "PROGN",
        &[
            parallel_body,
            sequential_body,
            local_defs_body,
            recursive_defs_body,
        ],
    );
    let source = "(progn (let () 41) (let* () 43) (flet () 47) (labels () 53))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    let empty_lets = form
        .nodes()
        .iter()
        .filter(
            |(_, node)| matches!(&node.kind, NodeKind::Let { bindings, .. } if bindings.is_empty()),
        )
        .count();
    let empty_local_functions = form
        .nodes()
        .iter()
        .filter(|(_, node)| {
            matches!(
                &node.kind,
                NodeKind::LocalFunctions { definitions, .. } if definitions.is_empty()
            )
        })
        .count();
    assert_eq!(empty_lets, 2);
    assert_eq!(empty_local_functions, 2);
    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::LocalFunctions { kind: LocalFunctionKind::Flet, definitions, .. }
            if definitions.is_empty()
    )));
    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::LocalFunctions { kind: LocalFunctionKind::Labels, definitions, .. }
            if definitions.is_empty()
    )));

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    for value in [41, 43, 47, 53] {
        assert!(lowered.entry.constants.contains(&Constant::Fixnum(value)));
    }

    // CLHS LET, LET*, FLET, and LABELS bodies are implicit progns, including empty definition lists.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_let_l.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_flet_.htm
    Ok(())
}

#[test]
fn empty_implicit_progn_bodies_return_nil() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(118);
    let empty_progn = fixture.form("PROGN", &[]);
    let macrolet_definitions = fixture.list(&[]);
    let macrolet = fixture.form("MACROLET", &[macrolet_definitions]);
    let symbol_macrolet_definitions = fixture.list(&[]);
    let symbol_macrolet = fixture.form("SYMBOL-MACROLET", &[symbol_macrolet_definitions]);
    let let_bindings = fixture.list(&[]);
    let let_form = fixture.form("LET", &[let_bindings]);
    let let_star_bindings = fixture.list(&[]);
    let let_star = fixture.form("LET*", &[let_star_bindings]);
    let flet_definitions = fixture.list(&[]);
    let flet = fixture.form("FLET", &[flet_definitions]);
    let labels_definitions = fixture.list(&[]);
    let labels = fixture.form("LABELS", &[labels_definitions]);
    let locally = fixture.form("LOCALLY", &[]);
    let input = fixture.form(
        "PROGN",
        &[
            empty_progn,
            macrolet,
            symbol_macrolet,
            let_form,
            let_star,
            flet,
            labels,
            locally,
        ],
    );
    let source = "(progn (progn) (macrolet ()) (symbol-macrolet ()) (let ()) (let* ()) (flet ()) (labels ()) (locally))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert_eq!(
        form.nodes()
            .iter()
            .filter(|(_, node)| matches!(
                &node.kind,
                NodeKind::Let { bindings, body, .. }
                    if bindings.is_empty() && body.is_empty()
            ))
            .count(),
        2
    );
    assert_eq!(
        form.nodes()
            .iter()
            .filter(|(_, node)| matches!(
                &node.kind,
                NodeKind::LocalFunctions { definitions, body, .. }
                    if definitions.is_empty() && body.is_empty()
            ))
            .count(),
        2
    );
    assert_eq!(
        form.nodes()
            .iter()
            .filter(|(_, node)| matches!(
                &node.kind,
                NodeKind::Sequence { forms, .. } if forms.is_empty()
            ))
            .count(),
        3
    );

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    assert!(lowered.entry.constants.contains(&Constant::Nil));

    // CLHS implicit-progn bodies and PROGN itself return NIL when empty.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_progn.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_let_l.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_flet_.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_locall.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_symbol.htm
    Ok(())
}

#[test]
fn if_without_an_else_form_returns_nil_in_hir_and_mir() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(119);
    let nil = fixture.cl("NIL");
    let input = fixture.form("IF", &[nil, Word::fixnum(61)]);
    let source = "(if nil 61)";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::If {
            else_branch: None,
            ..
        }
    )));

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    assert!(lowered.entry.constants.contains(&Constant::Nil));
    assert!(
        lowered
            .entry
            .blocks
            .iter()
            .any(|block| matches!(&block.terminator, ncl_ir::Terminator::Branch { .. }))
    );

    // CLHS gives the omitted IF else-form the default value NIL.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_if.htm
    Ok(())
}

#[test]
fn omitted_let_initializer_defaults_to_nil_in_hir_and_mir() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(124);
    let value = fixture.user("VALUE");
    let binding = fixture.list(&[value]);
    let bindings = fixture.list(&[binding]);
    let list = fixture.form("LIST", &[value]);
    let input = fixture.form("LET", &[bindings, list]);
    let source = "(let ((value)) (list value))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::Let { bindings, .. }
            if bindings.len() == 1
                && bindings.first().is_some_and(|binding| binding.initializer.is_none())
    )));

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    assert!(lowered.entry.constants.contains(&Constant::Nil));
    assert!(
        lowered
            .entry
            .blocks
            .iter()
            .flat_map(|block| &block.ops)
            .any(|operation| {
                matches!(
                    &operation.kind,
                    OpKind::CallClosure {
                        named_symbol: Some(_),
                        ..
                    }
                )
            })
    );

    // CLHS specifies NIL as the default initform when a LET binding omits one.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_let_l.htm
    Ok(())
}

#[test]
fn let_optimization_declaration_is_preserved_alongside_its_binding() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(125);
    let value = fixture.user("VALUE");
    let binding = fixture.list(&[value, Word::fixnum(13)]);
    let bindings = fixture.list(&[binding]);
    let speed_name = fixture.cl("SPEED");
    let speed = fixture.list(&[speed_name, Word::fixnum(2)]);
    let optimize_name = fixture.cl("OPTIMIZE");
    let optimize = fixture.list(&[optimize_name, speed]);
    let declaration = fixture.form("DECLARE", &[optimize]);
    let input = fixture.form("LET", &[bindings, declaration, value]);
    let source = "(let ((value 13)) (declare (optimize (speed 2))) value)";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(form.scope_metadata().values().any(|metadata| {
        metadata.policy.speed().value() == 2
            && metadata
                .declarations
                .iter()
                .any(|declaration| matches!(declaration, HirDeclaration::Optimize { .. }))
    }));
    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::Let { bindings, .. }
            if bindings.len() == 1
                && bindings.first().is_some_and(|binding| binding.initializer.is_some())
    )));

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    assert!(lowered.entry.constants.contains(&Constant::Fixnum(13)));

    // CLHS permits OPTIMIZE declarations in a LET body and defines SPEED as an optimization quality.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_let_l.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/d_optimi.htm
    Ok(())
}

#[test]
fn flet_functions_shadow_global_values_and_funcall_calls() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(134);
    let values_name = fixture.cl("VALUES");
    let no_parameters = fixture.list(&[]);
    let values_definition = fixture.list(&[values_name, no_parameters, Word::fixnum(7)]);
    let funcall_name = fixture.cl("FUNCALL");
    let parameter = fixture.user("VALUE");
    let funcall_parameters = fixture.list(&[parameter]);
    let funcall_definition = fixture.list(&[funcall_name, funcall_parameters, parameter]);
    let definitions = fixture.list(&[values_definition, funcall_definition]);
    let values_call = fixture.form("VALUES", &[]);
    let funcall_call = fixture.form("FUNCALL", &[Word::fixnum(59)]);
    let body = fixture.form("PROGN", &[values_call, funcall_call]);
    let input = fixture.form("FLET", &[definitions, body]);
    let source = "(flet ((values () 7) (funcall (value) value)) (values) (funcall 59))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::LocalFunctions {
            kind: LocalFunctionKind::Flet,
            definitions,
            ..
        } if definitions.len() == 2
    )));
    let mut local_call_arities = form
        .nodes()
        .iter()
        .filter_map(|(_, node)| match &node.kind {
            NodeKind::Call {
                target: CallTarget::Local(_),
                arguments,
            } => Some(arguments.len()),
            _ => None,
        })
        .collect::<Vec<_>>();
    local_call_arities.sort_unstable();
    assert_eq!(local_call_arities, [0, 1]);

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    let mut lowered_local_call_arities = lowered
        .entry
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter_map(|operation| match &operation.kind {
            OpKind::CallClosure {
                args,
                named_symbol: None,
                ..
            } => args.len().checked_sub(1),
            _ => None,
        })
        .collect::<Vec<_>>();
    lowered_local_call_arities.sort_unstable();
    assert_eq!(lowered_local_call_arities, [0, 1]);

    // CLHS FLET establishes lexical function bindings that shadow same-named global functions.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_flet_.htm
    Ok(())
}

#[test]
fn finite_integer_and_two_operand_cons_types_survive_hir_lowering() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(135);
    let cons_name = fixture.cl("CONS");
    let integer_name = fixture.cl("INTEGER");
    let symbol_name = fixture.cl("SYMBOL");
    let cons_type = fixture.list(&[cons_name, integer_name, symbol_name]);
    let integer_name = fixture.cl("INTEGER");
    let integer_type = fixture.list(&[integer_name, Word::fixnum(-2), Word::fixnum(10)]);
    let x = fixture.user("X");
    let quoted_symbol = fixture.form("QUOTE", &[x]);
    let cons_value = fixture.form("CONS", &[Word::fixnum(1), quoted_symbol]);
    let cons_assertion = fixture.form("THE", &[cons_type, cons_value]);
    let integer_assertion = fixture.form("THE", &[integer_type, Word::fixnum(7)]);
    let input = fixture.form("PROGN", &[cons_assertion, integer_assertion]);
    let source = "(progn (the (cons integer symbol) (cons 1 'x)) (the (integer -2 10) 7))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    let asserted_types = form
        .nodes()
        .iter()
        .filter_map(|(_, node)| match &node.kind {
            NodeKind::TypeAssertion { asserted_type, .. } => Some(*asserted_type),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(asserted_types.len(), 2);
    let cons_type = form
        .types()
        .get(asserted_types[0])
        .ok_or_else(|| std::io::Error::other("CONS assertion type is missing"))?;
    assert!(matches!(
        cons_type.kind(),
        ClTypeKind::Cons { car, cdr }
            if form.types().get(*car).is_some_and(|ty| ty.kind() == &ClTypeKind::Named(BuiltinType::Integer))
                && form.types().get(*cdr).is_some_and(|ty| ty.kind() == &ClTypeKind::Named(BuiltinType::Symbol))
    ));
    let integer_type = form
        .types()
        .get(asserted_types[1])
        .ok_or_else(|| std::io::Error::other("INTEGER assertion type is missing"))?;
    assert!(matches!(
        integer_type.kind(),
        ClTypeKind::NumericRange(range)
            if range.lower().is_some_and(|bound| bound.value() == NumericValue::Integer(-2))
                && range.upper().is_some_and(|bound| bound.value() == NumericValue::Integer(10))
    ));

    lower_both_and_verify(&builder, &form, &expr)?;

    // CLHS CONS accepts two element type operands; INTEGER accepts finite inclusive bounds.
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_cons.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_intege.htm
    Ok(())
}
