use ncl_hir::{BindingKind, HirDeclaration, LetKind, NodeKind, VariableTarget};
use ncl_ir::{Constant, OpKind};
use ncl_object::Word;

use super::common::{TestResult, build, constant_for_value, context, lower_both_and_verify};

fn result_is_three_argument_call(function: &ncl_ir::Function, expected: ncl_ir::ValueId) -> bool {
    function.blocks.iter().any(|block| {
        block.ops.iter().any(|operation| {
            operation
                .results
                .iter()
                .any(|(result, _)| *result == expected)
                && matches!(&operation.kind, OpKind::CallClosure { args, .. } if args.len() == 3)
        })
    })
}

#[test]
fn let_star_initializer_reads_the_previous_binding() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(64);
    let x = fixture.user("X");
    let y = fixture.user("Y");
    let initializer = fixture.form("+", &[x, Word::fixnum(3)]);
    let first_binding = fixture.list(&[x, Word::fixnum(2)]);
    let second_binding = fixture.list(&[y, initializer]);
    let bindings = fixture.list(&[first_binding, second_binding]);
    let values = fixture.form("VALUES", &[x, y]);
    let input = fixture.form("LET*", &[bindings, values]);
    let expr = fixture.expand(input)?;
    let form = build(
        &mut builder,
        id,
        &expr,
        "(let* ((x 2) (y (+ x 3))) (values x y))",
    )?;

    let sequential = form.nodes().iter().find_map(|(_, node)| match &node.kind {
        NodeKind::Let {
            kind: LetKind::Sequential,
            bindings,
            initializer_scopes,
            binding_scopes,
            ..
        } if bindings.len() == 2 => Some((bindings, initializer_scopes, binding_scopes)),
        _ => None,
    });
    let Some((bindings, initializer_scopes, binding_scopes)) = sequential else {
        return Err(std::io::Error::other("LET* sequential bindings are missing").into());
    };
    let previous = bindings
        .first()
        .ok_or_else(|| std::io::Error::other("first LET* binding is missing"))?
        .binding;
    let second = bindings
        .get(1)
        .ok_or_else(|| std::io::Error::other("second LET* binding is missing"))?;
    let second_initializer = second
        .initializer
        .ok_or_else(|| std::io::Error::other("second LET* initializer is missing"))?;
    let second_scope = *initializer_scopes
        .get(1)
        .ok_or_else(|| std::io::Error::other("second LET* initializer scope is missing"))?;
    let first_binding_scope = binding_scopes
        .first()
        .copied()
        .ok_or_else(|| std::io::Error::other("first LET* binding is missing"))?;
    let initializer_node = form
        .nodes()
        .get(second_initializer)
        .ok_or_else(|| std::io::Error::other("second LET* initializer node is missing"))?;
    let NodeKind::Call { arguments, .. } = &initializer_node.kind else {
        return Err(std::io::Error::other("second LET* initializer is not a call").into());
    };
    let first_argument = arguments
        .first()
        .copied()
        .ok_or_else(|| std::io::Error::other("second LET* initializer argument is missing"))?;
    let argument_node = form
        .nodes()
        .get(first_argument)
        .ok_or_else(|| std::io::Error::other("prior-binding reference node is missing"))?;

    // CLHS LET* makes each binding visible to later init forms.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_let_l.htm
    assert_eq!(second_scope, first_binding_scope);
    assert_eq!(argument_node.scope, second_scope);
    assert!(matches!(
        &argument_node.kind,
        NodeKind::VariableReference {
            target: VariableTarget::Lexical(binding),
        } if *binding == previous
    ));

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    let produced = lowered
        .entry
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|operation| match &operation.kind {
            OpKind::SetMultipleValues { values } if values.len() == 2 => Some(values),
            _ => None,
        })
        .ok_or_else(|| std::io::Error::other("two LET* values were not lowered"))?;
    let second_value = produced
        .get(1)
        .copied()
        .ok_or_else(|| std::io::Error::other("second LET* result is missing"))?;
    assert!(lowered.entry.constants.contains(&Constant::Fixnum(2)));
    assert!(lowered.entry.constants.contains(&Constant::Fixnum(3)));
    assert!(result_is_three_argument_call(&lowered.entry, second_value));
    Ok(())
}

#[test]
fn parallel_let_initializer_reads_outer_binding_and_setq_updates_inner_binding() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(65);
    let x = fixture.user("X");
    let outer_binding = fixture.list(&[x, Word::fixnum(4)]);
    let outer_bindings = fixture.list(&[outer_binding]);
    let inner_binding = fixture.list(&[x, x]);
    let inner_bindings = fixture.list(&[inner_binding]);
    let setq = fixture.form("SETQ", &[x, Word::fixnum(9)]);
    let inner_let = fixture.form("LET", &[inner_bindings, setq, x]);
    let input = fixture.form("LET", &[outer_bindings, inner_let]);
    let expr = fixture.expand(input)?;
    let form = build(
        &mut builder,
        id,
        &expr,
        "(let ((x 4))\n  (let ((x x))\n    (setq x 9)\n    x))\n",
    )?;

    let root = form
        .roots()
        .first()
        .copied()
        .ok_or_else(|| std::io::Error::other("outer LET root is missing"))?;
    let outer_node = form
        .nodes()
        .get(root)
        .ok_or_else(|| std::io::Error::other("outer LET node is missing"))?;
    let NodeKind::Let {
        kind: LetKind::Parallel,
        bindings: outer_bindings,
        binding_scopes: outer_binding_scopes,
        body_scope: outer_body_scope,
        body: outer_body,
        ..
    } = &outer_node.kind
    else {
        return Err(std::io::Error::other("outer form is not a parallel LET").into());
    };
    let outer_binding = outer_bindings
        .first()
        .ok_or_else(|| std::io::Error::other("outer LET binding is missing"))?
        .binding;
    let outer_binding_scope = outer_binding_scopes
        .first()
        .copied()
        .ok_or_else(|| std::io::Error::other("outer LET binding scope is missing"))?;
    let inner_id = outer_body
        .first()
        .copied()
        .ok_or_else(|| std::io::Error::other("inner LET form is missing"))?;
    let inner_node = form
        .nodes()
        .get(inner_id)
        .ok_or_else(|| std::io::Error::other("inner LET node is missing"))?;
    let NodeKind::Let {
        kind: LetKind::Parallel,
        bindings: inner_bindings,
        initializer_scopes,
        binding_scopes,
        body: inner_body,
        ..
    } = &inner_node.kind
    else {
        return Err(std::io::Error::other("inner form is not a parallel LET").into());
    };
    let inner_binding = inner_bindings
        .first()
        .ok_or_else(|| std::io::Error::other("inner LET binding is missing"))?
        .binding;
    let initializer = inner_bindings
        .first()
        .and_then(|binding| binding.initializer)
        .ok_or_else(|| std::io::Error::other("inner LET initializer is missing"))?;
    let initializer_node = form
        .nodes()
        .get(initializer)
        .ok_or_else(|| std::io::Error::other("inner LET initializer node is missing"))?;
    let initializer_scope = initializer_scopes
        .first()
        .copied()
        .ok_or_else(|| std::io::Error::other("inner LET initializer scope is missing"))?;
    let inner_binding_scope = binding_scopes
        .first()
        .copied()
        .ok_or_else(|| std::io::Error::other("inner LET binding scope is missing"))?;

    // CLHS LET evaluates init forms before introducing its bindings; SETQ assigns to the resolved variable.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_let_l.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_setq.htm
    assert_eq!(initializer_scope, *outer_body_scope);
    assert_eq!(initializer_node.scope, initializer_scope);
    assert!(matches!(
        &initializer_node.kind,
        NodeKind::VariableReference {
            target: VariableTarget::Lexical(binding),
        } if *binding == outer_binding
    ));
    assert_ne!(outer_binding, inner_binding);
    let outer_record = form
        .bindings()
        .binding(outer_binding)
        .ok_or_else(|| std::io::Error::other("outer lexical binding is missing"))?;
    assert_eq!(outer_record.kind(), BindingKind::LexicalVariable);
    assert_eq!(outer_record.scope(), outer_binding_scope);
    assert_eq!(outer_record.reads(), &[initializer]);
    assert!(outer_record.writes().is_empty());

    let assignment = inner_body
        .first()
        .copied()
        .ok_or_else(|| std::io::Error::other("inner SETQ assignment is missing"))?;
    let assignment_node = form
        .nodes()
        .get(assignment)
        .ok_or_else(|| std::io::Error::other("inner SETQ node is missing"))?;
    let NodeKind::Assignment {
        target: VariableTarget::Lexical(assigned_binding),
        ..
    } = &assignment_node.kind
    else {
        return Err(std::io::Error::other("SETQ does not target a lexical binding").into());
    };
    let read = inner_body
        .get(1)
        .copied()
        .ok_or_else(|| std::io::Error::other("inner lexical read is missing"))?;
    let read_node = form
        .nodes()
        .get(read)
        .ok_or_else(|| std::io::Error::other("inner lexical read node is missing"))?;
    assert_eq!(*assigned_binding, inner_binding);
    assert!(matches!(
        &read_node.kind,
        NodeKind::VariableReference {
            target: VariableTarget::Lexical(binding),
        } if *binding == inner_binding
    ));
    let inner_record = form
        .bindings()
        .binding(inner_binding)
        .ok_or_else(|| std::io::Error::other("inner lexical binding is missing"))?;
    assert_eq!(inner_record.kind(), BindingKind::LexicalVariable);
    assert_eq!(inner_record.scope(), inner_binding_scope);
    assert_eq!(inner_record.reads(), &[read]);
    assert_eq!(inner_record.writes(), &[assignment]);

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    let returned = lowered
        .entry
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            ncl_ir::Terminator::Return { values } => values.first().copied(),
            _ => None,
        })
        .ok_or_else(|| std::io::Error::other("lowered LET result is missing"))?;
    let assigned_cell = lowered
        .entry
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|operation| match &operation.kind {
            OpKind::StoreField {
                object,
                field: 0,
                value,
            } if constant_for_value(&lowered.entry, *value) == Some(&Constant::Fixnum(9)) => {
                Some(*object)
            }
            _ => None,
        })
        .ok_or_else(|| std::io::Error::other("SETQ value was not stored in the lexical cell"))?;
    assert!(
        lowered
            .entry
            .blocks
            .iter()
            .flat_map(|block| &block.ops)
            .any(|operation| matches!(
                &operation.kind,
                OpKind::LoadField { object, field: 0 }
                    if *object == assigned_cell
                        && operation.results.iter().any(|(value, _)| *value == returned)
            ))
    );
    Ok(())
}

#[test]
fn special_declaration_keeps_inner_let_binding_lexical() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(66);
    let x = fixture.user("X");
    let outer_binding = fixture.list(&[x, Word::fixnum(1)]);
    let outer_bindings = fixture.list(&[outer_binding]);
    let special_head = fixture.cl("SPECIAL");
    let special = fixture.list(&[special_head, x]);
    let declaration = fixture.form("DECLARE", &[special]);
    let inner_binding = fixture.list(&[x, Word::fixnum(2)]);
    let inner_bindings = fixture.list(&[inner_binding]);
    let setq = fixture.form("SETQ", &[x, Word::fixnum(3)]);
    let inner_let = fixture.form("LET", &[inner_bindings, setq, x]);
    let input = fixture.form("LET", &[outer_bindings, declaration, inner_let, x]);
    let expr = fixture.expand(input)?;
    let form = build(
        &mut builder,
        id,
        &expr,
        "(let ((x 1))\n  (declare (special x))\n  (let ((x 2))\n    (setq x 3)\n    x)\n  x)\n",
    )?;

    let root = form
        .roots()
        .first()
        .copied()
        .ok_or_else(|| std::io::Error::other("declared LET root is missing"))?;
    let outer_node = form
        .nodes()
        .get(root)
        .ok_or_else(|| std::io::Error::other("declared LET node is missing"))?;
    let NodeKind::Let {
        bindings: outer_bindings,
        binding_scopes: outer_binding_scopes,
        body_scope: outer_body_scope,
        body: outer_body,
        ..
    } = &outer_node.kind
    else {
        return Err(std::io::Error::other("outer form is not a LET").into());
    };
    let outer_binding = outer_bindings
        .first()
        .ok_or_else(|| std::io::Error::other("outer LET binding is missing"))?
        .binding;
    let outer_binding_scope = outer_binding_scopes
        .first()
        .copied()
        .ok_or_else(|| std::io::Error::other("outer binding scope is missing"))?;
    let symbol = form
        .bindings()
        .binding(outer_binding)
        .ok_or_else(|| std::io::Error::other("outer binding identity is missing"))?
        .symbol();
    let inner_id = outer_body
        .first()
        .copied()
        .ok_or_else(|| std::io::Error::other("inner LET form is missing"))?;
    let inner_node = form
        .nodes()
        .get(inner_id)
        .ok_or_else(|| std::io::Error::other("inner LET node is missing"))?;
    let NodeKind::Let {
        bindings: inner_bindings,
        binding_scopes,
        body: inner_body,
        ..
    } = &inner_node.kind
    else {
        return Err(std::io::Error::other("nested form is not a LET").into());
    };
    let inner_binding = inner_bindings
        .first()
        .ok_or_else(|| std::io::Error::other("inner LET binding is missing"))?
        .binding;
    let inner_binding_scope = binding_scopes
        .first()
        .copied()
        .ok_or_else(|| std::io::Error::other("inner LET binding scope is missing"))?;

    // CLHS SPECIAL makes the declared LET binding dynamic; an inner LET binding shadows it unless redeclared.
    // https://www.lispworks.com/documentation/HyperSpec/Body/d_specia.htm
    assert_ne!(outer_binding, inner_binding);
    let outer_record = form
        .bindings()
        .binding(outer_binding)
        .ok_or_else(|| std::io::Error::other("outer binding record is missing"))?;
    assert_eq!(outer_record.scope(), outer_binding_scope);
    assert!(outer_record.reads().is_empty());
    assert!(outer_record.writes().is_empty());
    assert_eq!(
        form.bindings()
            .scope_parent(inner_binding_scope)
            .map_err(|error| std::io::Error::other(error.to_string()))?,
        Some(*outer_body_scope)
    );
    assert!(form
        .scope_metadata()
        .get(outer_body_scope)
        .is_some_and(|metadata| metadata.declarations.iter().any(|declaration| {
            matches!(declaration, HirDeclaration::Special { names } if names.len() == 1 && names.first() == Some(&symbol))
        })));

    let assignment = inner_body
        .first()
        .copied()
        .ok_or_else(|| std::io::Error::other("inner SETQ assignment is missing"))?;
    let assignment_node = form
        .nodes()
        .get(assignment)
        .ok_or_else(|| std::io::Error::other("inner SETQ node is missing"))?;
    assert!(matches!(
        &assignment_node.kind,
        NodeKind::Assignment {
            target: VariableTarget::Lexical(binding),
            ..
        } if *binding == inner_binding
    ));
    let inner_read = inner_body
        .get(1)
        .copied()
        .ok_or_else(|| std::io::Error::other("inner lexical read is missing"))?;
    let inner_read_node = form
        .nodes()
        .get(inner_read)
        .ok_or_else(|| std::io::Error::other("inner lexical read node is missing"))?;
    assert!(matches!(
        &inner_read_node.kind,
        NodeKind::VariableReference {
            target: VariableTarget::Lexical(binding),
        } if *binding == inner_binding
    ));
    let outer_read = outer_body
        .get(1)
        .copied()
        .ok_or_else(|| std::io::Error::other("outer special read is missing"))?;
    let outer_read_node = form
        .nodes()
        .get(outer_read)
        .ok_or_else(|| std::io::Error::other("outer special read node is missing"))?;
    assert_eq!(outer_read_node.scope, *outer_body_scope);
    assert!(matches!(
        &outer_read_node.kind,
        NodeKind::VariableReference {
            target: VariableTarget::Special(target),
        } if *target == symbol
    ));
    let inner_record = form
        .bindings()
        .binding(inner_binding)
        .ok_or_else(|| std::io::Error::other("inner binding record is missing"))?;
    assert_eq!(inner_record.scope(), inner_binding_scope);
    assert_eq!(inner_record.reads(), &[inner_read]);
    assert_eq!(inner_record.writes(), &[assignment]);
    Ok(())
}
