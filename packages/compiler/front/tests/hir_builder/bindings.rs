use ncl_hir::{LetKind, NodeKind, VariableTarget};
use ncl_ir::{Constant, OpKind};
use ncl_object::Word;

use super::common::{TestResult, build, context, lower_both_and_verify};

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
