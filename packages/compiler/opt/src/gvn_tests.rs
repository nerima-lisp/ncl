#![allow(clippy::unwrap_used)]

use super::{GlobalValueNumbering, Module};
use crate::FunctionPass;
use std::fmt::Debug;

trait Fixture<T> {
    fn fixture(self) -> T;
}

impl<T, E: Debug> Fixture<T> for Result<T, E> {
    fn fixture(self) -> T {
        self.unwrap_or_else(|_| std::process::exit(1))
    }
}
use ncl_ir::{FunctionBuilder, FunctionId, OpKind, Param, Prim, Terminator, Ty, ValueId};

fn binary_duplicate() -> ncl_ir::Function {
    let mut builder = FunctionBuilder::new(
        FunctionId(1),
        "duplicate",
        vec![Param {
            name: "x".into(),
            ty: Ty::I64,
        }],
        vec![Ty::I64],
    );
    builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![ValueId(0), ValueId(0)],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture();
    let second = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![ValueId(0), ValueId(0)],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![second],
        })
        .fixture();
    builder.finish()
}

#[test]
fn removes_redundant_pure_computation() {
    let mut function = binary_duplicate();
    let mut pass = GlobalValueNumbering;
    assert!(pass.run(&mut function, &Module::default()).fixture());
    assert_eq!(function.blocks[0].ops.len(), 1);
    assert!(
        matches!(&function.blocks[0].terminator, Terminator::Return { values } if values.len() == 1)
    );
    ncl_ir::verify(&function).fixture();
}

#[test]
fn store_invalidates_load_value() {
    let mut builder = FunctionBuilder::new(
        FunctionId(2),
        "memory",
        vec![
            Param {
                name: "address".into(),
                ty: Ty::Address,
            },
            Param {
                name: "value".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let first = builder
        .push_op(
            OpKind::Load {
                address: ValueId(0),
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder
        .push_op(
            OpKind::Store {
                address: ValueId(0),
                value: ValueId(1),
            },
            &[],
        )
        .fixture();
    let second = builder
        .push_op(
            OpKind::Load {
                address: ValueId(0),
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![second],
        })
        .fixture();
    let mut function = builder.finish();
    let mut pass = GlobalValueNumbering;
    assert!(!pass.run(&mut function, &Module::default()).fixture());
    assert_eq!(
        function.blocks[0]
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OpKind::Load { .. }))
            .count(),
        2
    );
    assert_ne!(first, second);
}
