use ncl_hir::{
    BindingId, BindingKind, HirDeclaration, HirForm, LetKind, NodeId, NodeKind, ScopeId,
    VariableTarget,
};
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

struct LetView<'a> {
    kind: LetKind,
    bindings: &'a [ncl_hir::LetBinding],
    initializer_scopes: &'a [ScopeId],
    binding_scopes: &'a [ScopeId],
    body_scope: ScopeId,
    body: &'a [NodeId],
}

fn required<T>(value: Option<T>, message: &str) -> TestResult<T> {
    value.ok_or_else(|| std::io::Error::other(message).into())
}

fn node<'a>(form: &'a HirForm, id: NodeId, message: &str) -> TestResult<&'a ncl_hir::Node> {
    required(form.nodes().get(id), message)
}

fn item<T: Copy>(items: &[T], index: usize, message: &str) -> TestResult<T> {
    required(items.get(index).copied(), message)
}

fn let_view<'a>(form: &'a HirForm, id: NodeId, message: &str) -> TestResult<LetView<'a>> {
    let node = node(form, id, message)?;
    let NodeKind::Let {
        kind,
        bindings,
        initializer_scopes,
        binding_scopes,
        body_scope,
        body,
    } = &node.kind
    else {
        return Err(std::io::Error::other(message).into());
    };
    Ok(LetView {
        kind: *kind,
        bindings,
        initializer_scopes,
        binding_scopes,
        body_scope: *body_scope,
        body,
    })
}

fn binding_id(view: &LetView<'_>, message: &str) -> TestResult<BindingId> {
    Ok(required(view.bindings.first(), message)?.binding)
}

fn assert_reference(
    form: &HirForm,
    id: NodeId,
    scope: ScopeId,
    target: VariableTarget,
) -> TestResult<()> {
    let node = node(form, id, "variable reference node is missing")?;
    assert_eq!(node.scope, scope);
    let NodeKind::VariableReference { target: actual } = &node.kind else {
        return Err(std::io::Error::other("node is not a variable reference").into());
    };
    assert_eq!(*actual, target);
    Ok(())
}

fn assert_assignment(
    form: &HirForm,
    id: NodeId,
    scope: ScopeId,
    target: VariableTarget,
) -> TestResult<()> {
    let node = node(form, id, "assignment node is missing")?;
    assert_eq!(node.scope, scope);
    let NodeKind::Assignment { target: actual, .. } = &node.kind else {
        return Err(std::io::Error::other("node is not an assignment").into());
    };
    assert_eq!(*actual, target);
    Ok(())
}

fn assert_binding_uses(
    form: &HirForm,
    id: BindingId,
    scope: ScopeId,
    reads: &[NodeId],
    writes: &[NodeId],
) -> TestResult<()> {
    let binding = required(
        form.bindings().binding(id),
        "lexical binding record is missing",
    )?;
    assert_eq!(binding.kind(), BindingKind::LexicalVariable);
    assert_eq!(binding.scope(), scope);
    assert_eq!(binding.reads(), reads);
    assert_eq!(binding.writes(), writes);
    Ok(())
}

fn assert_parallel_let_bindings(form: &HirForm) -> TestResult<()> {
    let outer = let_view(
        form,
        item(form.roots(), 0, "outer LET root is missing")?,
        "outer LET node is missing",
    )?;
    assert_eq!(outer.kind, LetKind::Parallel);
    let outer_binding = binding_id(&outer, "outer LET binding is missing")?;
    let outer_binding_scope = item(
        outer.binding_scopes,
        0,
        "outer LET binding scope is missing",
    )?;
    let inner = let_view(
        form,
        item(outer.body, 0, "inner LET form is missing")?,
        "inner LET node is missing",
    )?;
    assert_eq!(inner.kind, LetKind::Parallel);
    let inner_binding = binding_id(&inner, "inner LET binding is missing")?;
    let initializer = required(
        inner
            .bindings
            .first()
            .and_then(|binding| binding.initializer),
        "inner LET initializer is missing",
    )?;
    let initializer_scope = item(
        inner.initializer_scopes,
        0,
        "inner LET initializer scope is missing",
    )?;

    // CLHS LET evaluates init forms before introducing its bindings; SETQ assigns to the resolved variable.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_let_l.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_setq.htm
    assert_eq!(initializer_scope, outer.body_scope);
    assert_reference(
        form,
        initializer,
        initializer_scope,
        VariableTarget::Lexical(outer_binding),
    )?;
    assert_ne!(outer_binding, inner_binding);
    assert_binding_uses(
        form,
        outer_binding,
        outer_binding_scope,
        &[initializer],
        &[],
    )?;

    let assignment = item(inner.body, 0, "inner SETQ assignment is missing")?;
    let read = item(inner.body, 1, "inner lexical read is missing")?;
    assert_assignment(
        form,
        assignment,
        inner.body_scope,
        VariableTarget::Lexical(inner_binding),
    )?;
    assert_reference(
        form,
        read,
        inner.body_scope,
        VariableTarget::Lexical(inner_binding),
    )?;
    assert_binding_uses(
        form,
        inner_binding,
        item(
            inner.binding_scopes,
            0,
            "inner LET binding scope is missing",
        )?,
        &[read],
        &[assignment],
    )?;
    Ok(())
}

fn assert_parallel_let_mir_value_flow(function: &ncl_ir::Function) -> TestResult<()> {
    let returned = required(
        function
            .blocks
            .iter()
            .find_map(|block| match &block.terminator {
                ncl_ir::Terminator::Return { values } => values.first().copied(),
                _ => None,
            }),
        "lowered LET result is missing",
    )?;
    let assigned_cell = required(
        function
            .blocks
            .iter()
            .flat_map(|block| &block.ops)
            .find_map(|operation| match &operation.kind {
                OpKind::StoreField {
                    object,
                    field: 0,
                    value,
                } if constant_for_value(function, *value) == Some(&Constant::Fixnum(9)) => {
                    Some(*object)
                }
                _ => None,
            }),
        "SETQ value was not stored in the lexical cell",
    )?;
    assert!(
        function
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

fn assert_special_let_shadow(form: &HirForm) -> TestResult<()> {
    let outer = let_view(
        form,
        item(form.roots(), 0, "declared LET root is missing")?,
        "declared LET node is missing",
    )?;
    let outer_binding = binding_id(&outer, "outer LET binding is missing")?;
    let outer_binding_scope = item(outer.binding_scopes, 0, "outer binding scope is missing")?;
    let symbol = required(
        form.bindings().binding(outer_binding),
        "outer binding identity is missing",
    )?
    .symbol();
    let inner = let_view(
        form,
        item(outer.body, 0, "inner LET form is missing")?,
        "inner LET node is missing",
    )?;
    let inner_binding = binding_id(&inner, "inner LET binding is missing")?;
    let inner_binding_scope = item(
        inner.binding_scopes,
        0,
        "inner LET binding scope is missing",
    )?;

    // CLHS SPECIAL makes the declared LET binding dynamic; an inner LET binding shadows it unless redeclared.
    // https://www.lispworks.com/documentation/HyperSpec/Body/d_specia.htm
    assert_ne!(outer_binding, inner_binding);
    let outer_record = required(
        form.bindings().binding(outer_binding),
        "outer binding record is missing",
    )?;
    assert_eq!(outer_record.scope(), outer_binding_scope);
    assert!(outer_record.reads().is_empty());
    assert!(outer_record.writes().is_empty());
    assert_eq!(
        form.bindings()
            .scope_parent(inner_binding_scope)
            .map_err(|error| std::io::Error::other(error.to_string()))?,
        Some(outer.body_scope)
    );
    assert!(form
        .scope_metadata()
        .get(&outer.body_scope)
        .is_some_and(|metadata| metadata.declarations.iter().any(|declaration| {
            matches!(declaration, HirDeclaration::Special { names } if names.len() == 1 && names.first() == Some(&symbol))
        })));

    let assignment = item(inner.body, 0, "inner SETQ assignment is missing")?;
    let assignment_node = node(form, assignment, "inner SETQ node is missing")?;
    assert!(matches!(
        &assignment_node.kind,
        NodeKind::Assignment {
            target: VariableTarget::Lexical(binding),
            ..
        } if *binding == inner_binding
    ));
    let inner_read = item(inner.body, 1, "inner lexical read is missing")?;
    let inner_read_node = node(form, inner_read, "inner lexical read node is missing")?;
    assert!(matches!(
        &inner_read_node.kind,
        NodeKind::VariableReference {
            target: VariableTarget::Lexical(binding),
        } if *binding == inner_binding
    ));
    let outer_read = item(outer.body, 1, "outer special read is missing")?;
    let outer_read_node = node(form, outer_read, "outer special read node is missing")?;
    assert_eq!(outer_read_node.scope, outer.body_scope);
    assert!(matches!(
        &outer_read_node.kind,
        NodeKind::VariableReference {
            target: VariableTarget::Special(target),
        } if *target == symbol
    ));
    let inner_record = required(
        form.bindings().binding(inner_binding),
        "inner binding record is missing",
    )?;
    assert_eq!(inner_record.scope(), inner_binding_scope);
    assert_eq!(inner_record.reads(), &[inner_read]);
    assert_eq!(inner_record.writes(), &[assignment]);
    Ok(())
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
    assert_parallel_let_bindings(&form)?;
    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    assert_parallel_let_mir_value_flow(&lowered.entry)?;
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
    assert_special_let_shadow(&form)?;
    Ok(())
}
