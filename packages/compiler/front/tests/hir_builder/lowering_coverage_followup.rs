use ncl_hir::{CompileTimeSituation, LocalFunctionKind, NodeKind};
use ncl_ir::{Constant, HandlerKind, OpKind, Terminator as MirTerminator};
use ncl_object::Word;

use super::common::{TestResult, context, expand_build, lower_both_and_verify};
use super::control::named_call_position;

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
fn empty_tagbody_returns_nil_and_block_returns_its_body_value() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(116);
    let empty_tagbody = fixture.form("TAGBODY", &[]);
    let block_name = fixture.user("RESULT");
    let block = fixture.form("BLOCK", &[block_name, Word::fixnum(59)]);
    let input = fixture.form("PROGN", &[empty_tagbody, block]);
    let source = "(progn (tagbody) (block result 59))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::Tagbody { tags, forms } if tags.is_empty() && forms.is_empty()
    )));
    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::Block { body, .. } if body.len() == 1
    )));

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    assert!(lowered.entry.constants.contains(&Constant::Nil));
    assert!(lowered.entry.constants.contains(&Constant::Fixnum(59)));

    // CLHS TAGBODY returns NIL; BLOCK returns the values of its body unless RETURN-FROM exits it.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_tagbod.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_block.htm
    Ok(())
}

#[test]
fn empty_catch_and_progv_bodies_and_valueless_return_from_produce_nil() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(117);
    let done = fixture.user("DONE");
    let return_from = fixture.form("RETURN-FROM", &[done]);
    let block = fixture.form("BLOCK", &[done, return_from]);
    let tag = fixture.keyword("TAG");
    let catch = fixture.form("CATCH", &[tag]);
    let nil = fixture.cl("NIL");
    let progv = fixture.form("PROGV", &[nil, nil]);
    let input = fixture.form("PROGN", &[block, catch, progv]);
    let source = "(progn (block done (return-from done)) (catch :tag) (progv nil nil))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(
        form.nodes()
            .iter()
            .any(|(_, node)| matches!(&node.kind, NodeKind::ReturnFrom { value: None, .. }))
    );
    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::Catch { body, .. } if body.is_empty()
    )));
    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::DynamicBinding { body, .. } if body.is_empty()
    )));

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    assert!(lowered.entry.constants.contains(&Constant::Nil));

    // CLHS RETURN-FROM without a result form, empty CATCH, and empty PROGV bodies return NIL.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_block.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_catch.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_progv.htm
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
fn read_only_load_time_value_accepts_a_string_literal() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(120);
    let string = fixture.string("immutable");
    let read_only = fixture.cl("T");
    let input = fixture.form("LOAD-TIME-VALUE", &[string, read_only]);
    let source = "(load-time-value \"immutable\" t)";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::LoadTimeValue {
            read_only: true,
            form: value_form,
        } if matches!(form.nodes().get(*value_form).map(|node| &node.kind), Some(NodeKind::Literal(_)))
    )));

    let (lowered, baseline) = lower_both_and_verify(&builder, &form, &expr)?;
    let string_constant = Constant::StringBytes(b"immutable".to_vec());
    assert!(lowered.entry.constants.contains(&string_constant));
    assert!(baseline.entry.constants.contains(&string_constant));

    // CLHS permits a read-only LOAD-TIME-VALUE result to be treated as a constant object.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_ld_tim.htm
    Ok(())
}

#[test]
fn lambda_optional_keyword_rest_and_aux_modes_lower_to_distinct_argument_paths() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(121);
    let optional = fixture.cl("&OPTIONAL");
    let first = fixture.user("FIRST");
    let first_supplied = fixture.user("FIRST-P");
    let first_spec = fixture.list(&[first, Word::fixnum(5), first_supplied]);
    let second = fixture.user("SECOND");
    let second_supplied = fixture.user("SECOND-P");
    let second_spec = fixture.list(&[second, first, second_supplied]);
    let rest_marker = fixture.cl("&REST");
    let rest = fixture.user("REST");
    let key_marker = fixture.cl("&KEY");
    let alias = fixture.keyword("ALIAS");
    let key_value = fixture.user("KEY-VALUE");
    let key_parameter = fixture.list(&[alias, key_value]);
    let key_supplied = fixture.user("KEY-P");
    let key_spec = fixture.list(&[key_parameter, second, key_supplied]);
    let allow_other_keys = fixture.cl("&ALLOW-OTHER-KEYS");
    let aux_marker = fixture.cl("&AUX");
    let copy = fixture.user("COPY");
    let copy_spec = fixture.list(&[copy, key_value]);
    let lambda_list = fixture.list(&[
        optional,
        first_spec,
        second_spec,
        rest_marker,
        rest,
        key_marker,
        key_spec,
        allow_other_keys,
        aux_marker,
        copy_spec,
    ]);
    let list_name = fixture.cl("LIST");
    let list_function = fixture.form("FUNCTION", &[list_name]);
    let body = fixture.form(
        "FUNCALL",
        &[
            list_function,
            first,
            first_supplied,
            second,
            second_supplied,
            rest,
            key_value,
            key_supplied,
            copy,
        ],
    );
    let input = fixture.form("LAMBDA", &[lambda_list, body]);
    let source = "(lambda (&optional (first 5 first-p) (second first second-p) &rest rest &key ((:alias key-value) second key-p) &allow-other-keys &aux (copy key-value)) (funcall #'list first first-p second second-p rest key-value key-p copy))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    let signature = form
        .lambda_signatures()
        .values()
        .next()
        .ok_or_else(|| std::io::Error::other("lambda signature is missing"))?;
    assert_eq!(signature.optional.len(), 2);
    assert!(signature.rest.is_some());
    assert_eq!(signature.keys.len(), 1);
    assert!(signature.allow_other_keys);
    assert_eq!(signature.aux.len(), 1);

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    let function = lowered
        .nested
        .first()
        .ok_or_else(|| std::io::Error::other("lambda MIR is missing"))?;
    let operations = function.blocks.iter().flat_map(|block| &block.ops);
    assert_eq!(
        function
            .blocks
            .iter()
            .filter(|block| matches!(&block.terminator, MirTerminator::Branch { .. }))
            .count(),
        3
    );
    for name in [
        "make-rest-list",
        "check-keywords",
        "keyword-value",
        "keyword-supplied-p",
    ] {
        assert!(operations.clone().any(|operation| matches!(
            &operation.kind,
            OpKind::Builtin { name: actual, .. } if actual == name
        )));
    }
    assert!(function.constants.contains(&Constant::Fixnum(5)));
    assert!(function.constants.contains(&Constant::Fixnum(2)));
    assert!(function.constants.contains(&Constant::T));
    assert!(function.constants.contains(&Constant::Nil));

    // CLHS lambda lists bind supplied-p variables and evaluate optional, keyword, and auxiliary defaults.
    // https://www.lispworks.com/documentation/HyperSpec/Body/03_da.htm
    Ok(())
}

#[test]
fn nested_lambda_captures_only_used_variables_and_a_local_function() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(123);
    let first = fixture.user("FIRST");
    let second = fixture.user("SECOND");
    let unused = fixture.user("UNUSED");
    let first_binding = fixture.list(&[first, Word::fixnum(3)]);
    let second_binding = fixture.list(&[second, Word::fixnum(5)]);
    let unused_binding = fixture.list(&[unused, Word::fixnum(7)]);
    let bindings = fixture.list(&[first_binding, second_binding, unused_binding]);

    let helper_name = fixture.user("HELPER");
    let argument = fixture.user("ARGUMENT");
    let helper_lambda_list = fixture.list(&[argument]);
    let addition = fixture.form("+", &[argument, first]);
    let helper_definition = fixture.list(&[helper_name, helper_lambda_list, addition]);
    let definitions = fixture.list(&[helper_definition]);

    let empty_lambda_list = fixture.list(&[]);
    let list_name = fixture.cl("LIST");
    let list_function = fixture.form("FUNCTION", &[list_name]);
    let helper_reference = fixture.form("FUNCTION", &[helper_name]);
    let body = fixture.form("FUNCALL", &[list_function, first, second, helper_reference]);
    let lambda = fixture.form("LAMBDA", &[empty_lambda_list, body]);
    let lambda_call = fixture.form("FUNCALL", &[lambda]);
    let flet = fixture.form("FLET", &[definitions, lambda_call]);
    let input = fixture.form("LET", &[bindings, flet]);
    let source = "(let ((first 3) (second 5) (unused 7)) (flet ((helper (argument) (+ argument first))) (funcall (lambda () (funcall #'list first second #'helper)))))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    let captured_counts = lowered
        .entry
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter_map(|operation| match &operation.kind {
            OpKind::MakeClosure { captures, .. } => Some(captures.len()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(captured_counts.contains(&1));
    assert!(captured_counts.contains(&3));
    assert!(!captured_counts.contains(&4));
    assert!(lowered.nested.iter().any(|function| {
        let captures = function
            .blocks
            .iter()
            .flat_map(|block| &block.ops)
            .filter_map(|operation| match &operation.kind {
                OpKind::LoadCapture { index } => Some(*index),
                _ => None,
            })
            .collect::<Vec<_>>();
        [0, 1, 2].iter().all(|index| captures.contains(index))
    }));
    assert!(!lowered.entry.blocks.iter().any(|block| {
        block
            .ops
            .iter()
            .any(|operation| matches!(&operation.kind, OpKind::MakeValueCell { .. }))
    }));

    // CLHS FLET has lexical function scope, and closures retain referenced lexical bindings.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_flet_.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/03_ad.htm
    Ok(())
}

#[test]
fn nested_unwind_protect_runs_each_cleanup_while_throw_unwinds_outward() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(122);
    let tag = fixture.keyword("ESCAPE");
    let throw = fixture.form("THROW", &[tag, Word::fixnum(43)]);
    let inner_name = fixture.user("INNER-CLEANUP");
    let inner_cleanup = fixture.list(&[inner_name]);
    let inner = fixture.form("UNWIND-PROTECT", &[throw, inner_cleanup]);
    let outer_name = fixture.user("OUTER-CLEANUP");
    let outer_cleanup = fixture.list(&[outer_name]);
    let outer = fixture.form("UNWIND-PROTECT", &[inner, outer_cleanup]);
    let input = fixture.form("CATCH", &[tag, outer]);
    let source = "(catch :escape (unwind-protect (unwind-protect (throw :escape 43) (inner-cleanup)) (outer-cleanup)))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    let unwind_regions = lowered
        .entry
        .handler_regions
        .iter()
        .filter(|region| region.kind == HandlerKind::UnwindProtect)
        .collect::<Vec<_>>();
    assert_eq!(unwind_regions.len(), 2);
    let throw_block = lowered
        .entry
        .blocks
        .iter()
        .find(|block| {
            matches!(&block.terminator, MirTerminator::Throw { .. })
                && block.ops.iter().any(|operation| {
                    matches!(
                        &operation.kind,
                        OpKind::Builtin { name, args, .. } if name == "throw" && args.len() == 2
                    )
                })
        })
        .ok_or_else(|| std::io::Error::other("THROW block is missing"))?;
    let inner_block =
        named_call_position(&lowered.entry, "COMMON-LISP-USER", "INNER-CLEANUP", true)
            .map(|(block, _)| block)
            .ok_or_else(|| std::io::Error::other("inner cleanup call is missing"))?;
    let outer_block =
        named_call_position(&lowered.entry, "COMMON-LISP-USER", "OUTER-CLEANUP", true)
            .map(|(block, _)| block)
            .ok_or_else(|| std::io::Error::other("outer cleanup call is missing"))?;
    let inner_region = unwind_regions
        .iter()
        .find(|region| region.cleanup == Some(inner_block))
        .ok_or_else(|| std::io::Error::other("inner cleanup handler is missing"))?;
    let outer_region = unwind_regions
        .iter()
        .find(|region| region.cleanup == Some(outer_block))
        .ok_or_else(|| std::io::Error::other("outer cleanup handler is missing"))?;
    assert!(inner_region.protected.contains(&throw_block.id));
    assert!(outer_region.protected.contains(&throw_block.id));
    assert!(outer_region.protected.contains(&inner_region.handler));
    assert!(lowered.entry.handler_regions.iter().any(|region| {
        region.kind == HandlerKind::Catch && region.protected.contains(&throw_block.id)
    }));

    // CLHS runs each UNWIND-PROTECT cleanup before THROW leaves that form's dynamic extent.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_unwind.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_throw.htm
    Ok(())
}
