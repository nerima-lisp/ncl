use ncl_ir::{Constant, OpKind, Terminator as MirTerminator};
use ncl_object::Word;

use super::common::{TestResult, constant_for_value, context, expand_build, lower_both_and_verify};

#[test]
fn omitted_lambda_parameters_bind_nil_with_and_without_keyword_processing() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(124);

    let optional_marker = fixture.cl("&OPTIONAL");
    let solo = fixture.user("SOLO");
    let solo_parameters = fixture.list(&[optional_marker, solo]);
    let solo_lambda = fixture.form("LAMBDA", &[solo_parameters, solo]);
    let solo_call = fixture.form("FUNCALL", &[solo_lambda]);

    let optional_marker = fixture.cl("&OPTIONAL");
    let optional = fixture.user("OPTIONAL-VALUE");
    let rest_marker = fixture.cl("&REST");
    let rest = fixture.user("REST-VALUE");
    let key_marker = fixture.cl("&KEY");
    let key = fixture.user("KEY-VALUE");
    let aux_marker = fixture.cl("&AUX");
    let aux = fixture.user("AUX-VALUE");
    let parameters = fixture.list(&[
        optional_marker,
        optional,
        rest_marker,
        rest,
        key_marker,
        key,
        aux_marker,
        aux,
    ]);
    let list_name = fixture.cl("LIST");
    let list_function = fixture.form("FUNCTION", &[list_name]);
    let body = fixture.form("FUNCALL", &[list_function, optional, rest, key, aux]);
    let lambda = fixture.form("LAMBDA", &[parameters, body]);
    let call = fixture.form("FUNCALL", &[lambda]);
    let input = fixture.form("PROGN", &[solo_call, call]);
    let source = "(progn (funcall (lambda (&optional solo) solo)) (funcall (lambda (&optional optional-value &rest rest-value &key key-value &aux aux-value) (funcall #'list optional-value rest-value key-value aux-value))))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    let signatures = form.lambda_signatures().values().collect::<Vec<_>>();
    assert_eq!(signatures.len(), 2);
    assert!(signatures.iter().any(|signature| {
        signature.optional.len() == 1
            && signature.optional[0].default.is_none()
            && signature.optional[0].supplied_p.is_none()
            && signature.rest.is_none()
            && signature.keys.is_empty()
            && signature.aux.is_empty()
    }));
    assert!(signatures.iter().any(|signature| {
        signature.optional.len() == 1
            && signature.optional[0].default.is_none()
            && signature.optional[0].supplied_p.is_none()
            && signature.rest.is_some()
            && signature.keys.len() == 1
            && signature.keys[0].default.is_none()
            && signature.keys[0].supplied_p.is_none()
            && !signature.allow_other_keys
            && signature.aux.len() == 1
            && signature.aux[0].default.is_none()
    }));

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    assert_eq!(lowered.nested.len(), 2);
    let branch_count = lowered
        .nested
        .iter()
        .flat_map(|function| &function.blocks)
        .filter(|block| matches!(&block.terminator, MirTerminator::Branch { .. }))
        .count();
    assert_eq!(branch_count, 3);
    assert!(
        lowered
            .nested
            .iter()
            .all(|function| { function.constants.contains(&Constant::Nil) })
    );
    assert!(lowered.nested.iter().any(|function| {
        function
            .blocks
            .iter()
            .flat_map(|block| &block.ops)
            .any(|operation| {
                matches!(
                    &operation.kind,
                    OpKind::Builtin { name, .. } if name == "make-rest-list"
                )
            })
    }));
    assert!(lowered.nested.iter().any(|function| {
        ["check-keywords", "keyword-value", "keyword-supplied-p"]
            .into_iter()
            .all(|name| {
                function.blocks.iter().flat_map(|block| &block.ops).any(|operation| {
                matches!(&operation.kind, OpKind::Builtin { name: actual, .. } if actual == name)
            })
            })
    }));

    // CLHS defaults omitted optional, keyword, and auxiliary parameters to NIL.
    // https://www.lispworks.com/documentation/HyperSpec/Body/03_da.htm
    Ok(())
}

#[test]
fn conditional_tagbody_go_paths_keep_a_verified_loop_backedge() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(125);
    let flag = fixture.user("FLAG");
    let start = fixture.user("START");
    let done = fixture.user("DONE");
    let go_done = fixture.form("GO", &[done]);
    let go_start = fixture.form("GO", &[start]);
    let branch = fixture.form("IF", &[flag, go_done, go_start]);
    let tagbody = fixture.form("TAGBODY", &[start, branch, done]);
    let lambda_parameters = fixture.list(&[flag]);
    let lambda = fixture.form("LAMBDA", &[lambda_parameters, tagbody]);
    let input = fixture.form("FUNCALL", &[lambda, Word::fixnum(1)]);
    let source = "(funcall (lambda (flag) (tagbody start (if flag (go done) (go start)) done)) 1)";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert_eq!(
        form.nodes()
            .iter()
            .filter(|(_, node)| matches!(&node.kind, ncl_hir::NodeKind::Go { .. }))
            .count(),
        2
    );
    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    let function = lowered
        .nested
        .first()
        .ok_or_else(|| std::io::Error::other("lambda MIR is missing"))?;
    assert!(
        function
            .blocks
            .iter()
            .any(|block| matches!(&block.terminator, MirTerminator::Branch { .. }))
    );
    assert!(function.blocks.iter().any(|block| {
        block
            .ops
            .iter()
            .any(|operation| matches!(&operation.kind, OpKind::Safepoint))
    }));
    assert!(function.blocks.iter().any(|block| {
        match &block.terminator {
            MirTerminator::Return { values } => {
                values
                    .first()
                    .and_then(|value| constant_for_value(function, *value))
                    == Some(&Constant::Nil)
            }
            _ => false,
        }
    }));

    // CLHS TAGBODY returns NIL; GO transfers to its target tag.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_tagbod.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_go.htm
    Ok(())
}
