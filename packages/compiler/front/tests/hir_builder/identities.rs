use ncl_compiler_front::hir_builder::HirBuilder;
use ncl_compiler_front::{Expr, SymbolRef};
use ncl_hir::{CompilationUnitId, FormId, HirForm, NodeKind, VariableTarget};

use super::common::build;

fn global_symbol(form: &HirForm, index: usize) -> ncl_hir::SymbolId {
    let node = form
        .nodes()
        .iter()
        .nth(index)
        .map(|(_, node)| node)
        .expect("variable node");
    let NodeKind::VariableReference {
        target: VariableTarget::Global(symbol),
    } = &node.kind
    else {
        panic!("expected global variable reference")
    };
    *symbol
}

#[test]
fn compilation_unit_interner_scopes_uninterned_identity_to_each_form() {
    let unit = CompilationUnitId::new(40);
    let mut builder = HirBuilder::new(unit);
    let first_id = FormId::new(unit, 0);
    let second_id = FormId::new(unit, 1);
    let uninterned = SymbolRef::uninterned("TEMP", 0);
    let shared = SymbolRef::interned("COMMON-LISP-USER", "SHARED");
    let first_expr = Expr::Progn(vec![
        Expr::Variable(uninterned.clone()),
        Expr::Variable(shared.clone()),
    ]);
    let second_expr = Expr::Progn(vec![Expr::Variable(uninterned), Expr::Variable(shared)]);
    let first = build(&mut builder, first_id, &first_expr, "(progn #:temp shared)");
    let second = build(
        &mut builder,
        second_id,
        &second_expr,
        "(progn #:temp shared)",
    );
    let first_uninterned = global_symbol(&first, 0);
    let second_uninterned = global_symbol(&second, 0);
    let first_interned = global_symbol(&first, 1);
    let second_interned = global_symbol(&second, 1);

    assert_ne!(first_uninterned, second_uninterned);
    assert_eq!(first_interned, second_interned);
    assert!(first.symbol_ids().contains(&first_uninterned));
    assert!(second.symbol_ids().contains(&second_interned));
}
