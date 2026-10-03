//! Value-sensitive coverage for lexical storage, assignment, and initializers.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use ncl_compiler_front::{
    AuxParam, Expr, LambdaExpr, LambdaList, LetBinding, Literal, OptionalParam, ParamName,
    SymbolRef, lower_toplevel,
};
use ncl_ir::{Constant, Function, OpKind, verify};

fn symbol(name: &str) -> SymbolRef {
    SymbolRef::interned("COMMON-LISP-USER", name)
}

fn assert_verifies(function: &Function) {
    if let Err(errors) = verify(function) {
        panic!("{} does not verify: {errors:?}\n{function}", function.name);
    }
}

fn ops(function: &Function) -> impl Iterator<Item = &OpKind> {
    function
        .blocks
        .iter()
        .flat_map(|block| block.ops.iter().map(|op| &op.kind))
}

#[test]
fn assignment_uses_cell_value_and_global_storage_for_distinct_values() {
    let cell = symbol("CELL");
    let lexical = symbol("LEXICAL");
    let expression = Expr::Let {
        sequential: false,
        bindings: vec![
            LetBinding {
                name: cell.clone(),
                value: Some(Expr::Constant(Literal::fixnum(11))),
            },
            LetBinding {
                name: lexical.clone(),
                value: Some(Expr::Constant(Literal::fixnum(22))),
            },
        ],
        declarations: Vec::new(),
        body: vec![
            Expr::Setq(vec![(cell.clone(), Expr::Constant(Literal::fixnum(33)))]),
            Expr::Lambda(Box::new(LambdaExpr {
                lambda_list: LambdaList::new(),
                declarations: Vec::new(),
                docstring: None,
                body: vec![Expr::Variable(cell)],
            })),
            Expr::Setq(vec![(lexical, Expr::Constant(Literal::fixnum(44)))]),
            Expr::Setq(vec![(
                symbol("GLOBAL"),
                Expr::Constant(Literal::fixnum(55)),
            )]),
        ],
    };

    let lowered = lower_toplevel(&expression).expect("storage and assignment matrix lowers");
    assert_verifies(&lowered.entry);
    for expected in [11, 22, 33, 44, 55] {
        assert!(
            lowered
                .entry
                .constants
                .contains(&Constant::Fixnum(expected)),
            "missing assigned or initialized value {expected}"
        );
    }
    assert_eq!(
        ops(&lowered.entry)
            .filter(|kind| matches!(kind, OpKind::StoreField { field: 0, .. }))
            .count(),
        2
    );
    assert_eq!(lowered.nested.len(), 1);
    assert_verifies(&lowered.nested[0]);
    assert!(ops(&lowered.nested[0]).any(|kind| matches!(kind, OpKind::LoadField { field: 0, .. })));
}

#[test]
fn let_and_lambda_initializers_keep_present_values_distinct_from_nil_defaults() {
    let let_value = symbol("LET-VALUE");
    let let_default = symbol("LET-DEFAULT");
    let expression = Expr::Let {
        sequential: true,
        bindings: vec![
            LetBinding {
                name: let_value.clone(),
                value: Some(Expr::Constant(Literal::fixnum(66))),
            },
            LetBinding {
                name: let_default.clone(),
                value: None,
            },
        ],
        declarations: Vec::new(),
        body: vec![Expr::Progn(vec![
            Expr::Variable(let_value),
            Expr::Variable(let_default),
        ])],
    };
    let lowered = lower_toplevel(&expression).expect("let initializer branches lower");
    assert_verifies(&lowered.entry);
    assert!(lowered.entry.constants.contains(&Constant::Fixnum(66)));
    assert!(lowered.entry.constants.contains(&Constant::Nil));

    let lambda = LambdaList {
        optional: vec![OptionalParam {
            name: ParamName::Symbol(symbol("OPTIONAL")),
            default: None,
            supplied_p: None,
        }],
        aux: vec![AuxParam {
            name: ParamName::Symbol(symbol("AUX")),
            default: None,
        }],
        ..LambdaList::new()
    };
    let lowered = lower_toplevel(&Expr::Lambda(Box::new(LambdaExpr {
        lambda_list: lambda,
        declarations: Vec::new(),
        docstring: None,
        body: vec![Expr::Variable(symbol("AUX"))],
    })))
    .expect("missing optional and aux initializers lower");
    assert_verifies(&lowered.entry);
    assert_eq!(lowered.nested.len(), 1);
    assert_verifies(&lowered.nested[0]);
    assert!(lowered.nested[0].constants.contains(&Constant::Nil));
}
