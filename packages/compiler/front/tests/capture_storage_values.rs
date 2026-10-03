//! Exact IR value checks for read-only and cell-backed closure captures.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use ncl_compiler_front::{
    Expr, LambdaExpr, LambdaList, LetBinding, Literal, SymbolRef, lower_toplevel,
};
use ncl_ir::{ConstantIndex, Function, OpKind, ValueId, verify};

fn symbol(name: &str) -> SymbolRef {
    SymbolRef::interned("COMMON-LISP-USER", name)
}

fn lambda(body: Expr) -> LambdaExpr {
    LambdaExpr {
        lambda_list: LambdaList::new(),
        declarations: Vec::new(),
        docstring: None,
        body: vec![body],
    }
}

fn assert_verifies(function: &Function) {
    if let Err(errors) = verify(function) {
        panic!("{} does not verify: {errors:?}\n{function}", function.name);
    }
}

#[test]
fn read_only_capture_uses_the_value_in_closure_and_nested_load() {
    let captured = symbol("CAPTURED");
    let expression = Expr::Let {
        sequential: false,
        bindings: vec![LetBinding {
            name: captured.clone(),
            value: Some(Expr::Constant(Literal::fixnum(7))),
        }],
        declarations: Vec::new(),
        body: vec![Expr::Lambda(Box::new(lambda(Expr::Variable(captured))))],
    };
    let lowered = lower_toplevel(&expression).expect("read-only capture lowers");
    assert_verifies(&lowered.entry);
    assert_eq!(lowered.entry.blocks.len(), 1);
    assert_eq!(
        lowered.entry.blocks[0].ops[0].kind,
        OpKind::Const {
            result: ConstantIndex(0)
        }
    );
    assert_eq!(lowered.entry.blocks[0].ops[1].kind, OpKind::Convert {
        op: ncl_ir::Convert::I64ToWord,
        value: ValueId(0),
    });
    assert_eq!(
        lowered.entry.blocks[0].ops[2].kind,
        OpKind::Const {
            result: ConstantIndex(1)
        }
    );
    assert_eq!(
        lowered.entry.blocks[0].ops[3].kind,
        OpKind::MakeClosure {
            entry: ValueId(2),
            captures: vec![ValueId(1)]
        }
    );
    assert_eq!(
        lowered.entry.blocks[0].terminator,
        ncl_ir::Terminator::Return {
            values: vec![ValueId(3)]
        }
    );

    assert_eq!(lowered.nested.len(), 1);
    let nested = &lowered.nested[0];
    assert_verifies(nested);
    assert_eq!(nested.blocks[0].ops.len(), 1);
    assert_eq!(nested.blocks[0].ops[0].kind, OpKind::LoadCapture { index: 0 });
    assert_eq!(
        nested.blocks[0].terminator,
        ncl_ir::Terminator::Return {
            values: vec![ValueId(1)]
        }
    );
}

#[test]
fn assigned_capture_uses_one_cell_value_for_closure_and_store() {
    let captured = symbol("CAPTURED");
    let expression = Expr::Let {
        sequential: false,
        bindings: vec![LetBinding {
            name: captured.clone(),
            value: Some(Expr::Constant(Literal::fixnum(7))),
        }],
        declarations: Vec::new(),
        body: vec![Expr::Lambda(Box::new(lambda(Expr::Setq(vec![(
            captured,
            Expr::Constant(Literal::fixnum(9)),
        )]))))],
    };
    let lowered = lower_toplevel(&expression).expect("assigned capture lowers");
    assert_verifies(&lowered.entry);
    assert_eq!(
        lowered.entry.blocks[0].ops[0].kind,
        OpKind::Const {
            result: ConstantIndex(0)
        }
    );
    assert_eq!(lowered.entry.blocks[0].ops[1].kind, OpKind::Convert {
        op: ncl_ir::Convert::I64ToWord,
        value: ValueId(0),
    });
    assert_eq!(
        lowered.entry.blocks[0].ops[2].kind,
        OpKind::MakeValueCell { value: ValueId(1) }
    );
    assert_eq!(
        lowered.entry.blocks[0].ops[3].kind,
        OpKind::Const {
            result: ConstantIndex(1)
        }
    );
    assert_eq!(
        lowered.entry.blocks[0].ops[4].kind,
        OpKind::MakeClosure {
            entry: ValueId(3),
            captures: vec![ValueId(2)]
        }
    );
    assert_eq!(
        lowered.entry.blocks[0].terminator,
        ncl_ir::Terminator::Return {
            values: vec![ValueId(4)]
        }
    );

    assert_eq!(lowered.nested.len(), 1);
    let nested = &lowered.nested[0];
    assert_verifies(nested);
    assert_eq!(nested.blocks[0].ops[0].kind, OpKind::LoadCapture { index: 0 });
    assert_eq!(
        nested.blocks[0].ops[1].kind,
        OpKind::Const {
            result: ConstantIndex(0)
        }
    );
    assert_eq!(nested.blocks[0].ops[2].kind, OpKind::Convert {
        op: ncl_ir::Convert::I64ToWord,
        value: ValueId(2),
    });
    assert_eq!(nested.blocks[0].ops[3].kind, OpKind::StoreField {
        object: ValueId(1),
        field: 0,
        value: ValueId(3),
    });
    assert_eq!(
        nested.blocks[0].terminator,
        ncl_ir::Terminator::Return {
            values: vec![ValueId(3)]
        }
    );
}
