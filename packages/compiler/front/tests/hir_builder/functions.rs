use ncl_hir::{LocalFunctionKind, NodeKind, VariableTarget};
use ncl_object::Word;

use super::common::{context, expand_build, fixture_text, has_node, lower_both_and_verify};

#[test]
fn labels_and_lexical_assignment_lower_to_mir() {
    let (first_id, mut builder, mut fixture) = context(42);
    let function_name = fixture.user("COUNT-DOWN");
    let argument = fixture.user("N");
    let lambda_list = fixture.list(&[argument]);
    let nil = fixture.cl("NIL");
    let recur = fixture.form("COUNT-DOWN", &[nil]);
    let branch = fixture.form("IF", &[argument, recur, Word::fixnum(7)]);
    let definition = fixture.list(&[function_name, lambda_list, branch]);
    let definitions = fixture.list(&[definition]);
    let truth = fixture.cl("T");
    let call = fixture.form("COUNT-DOWN", &[truth]);
    let input = fixture.form("LABELS", &[definitions, call]);
    let source = fixture_text(
        include_str!("../fixtures/hir-bridge/labels.lisp"),
        "(labels ((count-down (n) (if n (count-down nil) 7)))\n  (count-down t))\n",
    );
    let (expr, form) = expand_build(&mut builder, &mut fixture, first_id, input, source);

    assert!(has_node(&form, |node| matches!(
        node,
        NodeKind::LocalFunctions {
            kind: LocalFunctionKind::Labels,
            ..
        }
    )));
    lower_both_and_verify(&builder, &form, &expr);

    let id = ncl_hir::FormId::new(first_id.compilation_unit(), 1);
    let x = fixture.user("X");
    let binding = fixture.list(&[x, Word::fixnum(1)]);
    let bindings = fixture.list(&[binding]);
    let setq = fixture.form("SETQ", &[x, Word::fixnum(2)]);
    let input = fixture.form("LET", &[bindings, setq, x]);
    let source = fixture_text(
        include_str!("../fixtures/hir-bridge/assignment.lisp"),
        "(let ((x 1))\n  (setq x 2)\n  x)\n",
    );
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source);

    assert!(has_node(&form, |node| matches!(
        node,
        NodeKind::Assignment {
            target: VariableTarget::Lexical(_),
            ..
        }
    )));
    lower_both_and_verify(&builder, &form, &expr);
}
