#![allow(clippy::unwrap_used)]

use super::{Evidence, FunctionFacts, GlobalValueNumbering, Module, ScalarFact, ScalarType};
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
use ncl_ir::{
    BlockId, Compare, Constant, Convert, FunctionBuilder, FunctionId, OpKind, Param, Prim,
    Terminator, Ty, ValueId,
};

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
fn preserves_original_block_order() {
    let mut builder = FunctionBuilder::new(FunctionId(9), "block_order", vec![], vec![]);
    let mut expected = vec![BlockId(0)];
    for _ in 0..12 {
        builder
            .terminate(Terminator::Return { values: vec![] })
            .fixture();
        expected.push(builder.create_block(Vec::new()));
    }
    builder
        .terminate(Terminator::Return { values: vec![] })
        .fixture();
    let mut function = builder.finish();
    let mut pass = GlobalValueNumbering;

    assert!(!pass.run(&mut function, &Module::default()).fixture());
    assert_eq!(
        function
            .blocks
            .iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        expected
    );
    ncl_ir::verify(&function).fixture();
}

#[test]
fn removes_redundant_proven_fixnum_word_constant() {
    let mut builder = FunctionBuilder::new(FunctionId(3), "word_constant", vec![], vec![Ty::Word]);
    let constant = builder.add_constant(Constant::Fixnum(7));
    let fixnum = builder
        .push_op(OpKind::Const { result: constant }, &[Ty::I64])
        .fixture()[0];
    let first = builder
        .push_op(
            OpKind::Convert {
                op: Convert::I64ToWord,
                value: fixnum,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    let second = builder
        .push_op(
            OpKind::Convert {
                op: Convert::I64ToWord,
                value: fixnum,
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
    let facts = FunctionFacts::analyze(&function);
    assert_eq!(
        facts.values.get(&first),
        Some(&ScalarFact::Known {
            ty: ScalarType::Fixnum,
            evidence: Evidence::Proven,
        })
    );
    let mut pass = GlobalValueNumbering;
    assert!(pass.run(&mut function, &Module::default()).fixture());
    assert_eq!(function.blocks[0].ops.len(), 2);
    assert_eq!(
        function.blocks[0]
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OpKind::Convert { .. }))
            .count(),
        1
    );
    assert!(matches!(
        &function.blocks[0].terminator,
        Terminator::Return { values } if values == &[first]
    ));
}

#[test]
fn removes_redundant_f64_comparison_with_boolean_result() {
    let mut builder = FunctionBuilder::new(
        FunctionId(8),
        "float_comparison",
        vec![
            Param {
                name: "left".into(),
                ty: Ty::F64,
            },
            Param {
                name: "right".into(),
                ty: Ty::F64,
            },
        ],
        vec![Ty::Bool],
    );
    builder
        .push_op(
            OpKind::Compare {
                op: Compare::Eq,
                left: ValueId(0),
                right: ValueId(1),
            },
            &[Ty::Bool],
        )
        .fixture();
    let second = builder
        .push_op(
            OpKind::Compare {
                op: Compare::Eq,
                left: ValueId(0),
                right: ValueId(1),
            },
            &[Ty::Bool],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![second],
        })
        .fixture();
    let mut function = builder.finish();
    let mut pass = GlobalValueNumbering;
    assert!(pass.run(&mut function, &Module::default()).fixture());
    assert_eq!(function.blocks[0].ops.len(), 1);
    assert!(matches!(
        &function.blocks[0].terminator,
        Terminator::Return { values } if values == &[ValueId(2)]
    ));
}

#[test]
fn unknown_word_moves_are_not_value_numbered() {
    let mut builder = FunctionBuilder::new(
        FunctionId(4),
        "unknown_word",
        vec![Param {
            name: "value".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    builder
        .push_op(OpKind::Move { value: ValueId(0) }, &[Ty::Word])
        .fixture();
    let second = builder
        .push_op(OpKind::Move { value: ValueId(0) }, &[Ty::Word])
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![second],
        })
        .fixture();
    let mut function = builder.finish();
    let mut pass = GlobalValueNumbering;
    assert!(!pass.run(&mut function, &Module::default()).fixture());
    assert_eq!(function.blocks[0].ops.len(), 2);
}

#[test]
fn address_conversions_are_excluded_from_gvn() {
    let mut builder = FunctionBuilder::new(
        FunctionId(5),
        "address_conversion",
        vec![Param {
            name: "address".into(),
            ty: Ty::Address,
        }],
        vec![Ty::Word],
    );
    builder
        .push_op(
            OpKind::Convert {
                op: Convert::AddressToWord,
                value: ValueId(0),
            },
            &[Ty::Word],
        )
        .fixture();
    let converted = builder
        .push_op(
            OpKind::Convert {
                op: Convert::AddressToWord,
                value: ValueId(0),
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![converted],
        })
        .fixture();
    let mut function = builder.finish();
    let mut pass = GlobalValueNumbering;
    assert!(!pass.run(&mut function, &Module::default()).fixture());
    assert_eq!(function.blocks[0].ops.len(), 2);
}

#[test]
fn loads_are_excluded_from_gvn() {
    let mut pass = GlobalValueNumbering;
    let mut builder = FunctionBuilder::new(
        FunctionId(6),
        "load",
        vec![Param {
            name: "address".into(),
            ty: Ty::Address,
        }],
        vec![Ty::Word],
    );
    builder
        .push_op(
            OpKind::Load {
                address: ValueId(0),
            },
            &[Ty::Word],
        )
        .fixture();
    let loaded = builder
        .push_op(
            OpKind::Load {
                address: ValueId(0),
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![loaded],
        })
        .fixture();
    let mut function = builder.finish();
    assert!(!pass.run(&mut function, &Module::default()).fixture());
    assert_eq!(function.blocks[0].ops.len(), 2);
}

#[test]
fn signalling_primitives_are_excluded_from_gvn() {
    let mut pass = GlobalValueNumbering;
    let mut builder = FunctionBuilder::new(
        FunctionId(7),
        "signalling_primitive",
        vec![Param {
            name: "value".into(),
            ty: Ty::I64,
        }],
        vec![Ty::I64],
    );
    builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumDiv,
                args: vec![ValueId(0), ValueId(0)],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture();
    let divided = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumDiv,
                args: vec![ValueId(0), ValueId(0)],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![divided],
        })
        .fixture();
    let mut function = builder.finish();
    assert!(!pass.run(&mut function, &Module::default()).fixture());
    assert_eq!(function.blocks[0].ops.len(), 2);
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
