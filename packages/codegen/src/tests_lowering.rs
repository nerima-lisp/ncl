#![allow(missing_docs, clippy::expect_used, clippy::too_many_lines)]

use super::tests::Aarch64FixtureAbi;
use super::*;
use ncl_ir::{Constant, FunctionBuilder, OpKind, Param, Prim, Terminator, Ty};

#[test]
fn lowering_generic_ops_emit_expected_code_and_safepoints() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(16),
        "generic-ops",
        vec![Param {
            name: "argument".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let constant = builder.add_constant(Constant::Fixnum(7));
    let value = builder
        .push_op(OpKind::Const { result: constant }, &[Ty::Word])
        .expect("constant op must produce a value")[0];
    let moved = builder
        .push_op(OpKind::Move { value }, &[Ty::Word])
        .expect("move op must produce a value")[0];
    let converted = builder
        .push_op(
            OpKind::Convert {
                op: ncl_ir::Convert::WordToI64,
                value: moved,
            },
            &[Ty::I64],
        )
        .expect("convert op must produce a value")[0];
    let loaded = builder
        .push_op(OpKind::Load { address: moved }, &[Ty::Word])
        .expect("load op must produce a value")[0];
    assert!(
        builder
            .push_op(
                OpKind::Store {
                    address: moved,
                    value: converted,
                },
                &[],
            )
            .is_ok()
    );
    let field = builder
        .push_op(
            OpKind::LoadField {
                object: moved,
                field: 1,
            },
            &[Ty::Word],
        )
        .expect("load-field op must produce a value")[0];
    assert!(
        builder
            .push_op(
                OpKind::StoreField {
                    object: moved,
                    field: 2,
                    value: field,
                },
                &[],
            )
            .is_ok()
    );
    assert!(
        builder
            .push_op(OpKind::Alloc { words: 2 }, &[Ty::Word])
            .is_ok()
    );
    assert!(
        builder
            .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
            .is_ok()
    );
    assert!(
        builder
            .push_op(
                OpKind::Call {
                    function: moved,
                    args: Vec::new(),
                },
                &[Ty::Word]
            )
            .is_ok()
    );
    assert!(
        builder
            .push_op(
                OpKind::CallIndirect {
                    callee: moved,
                    args: Vec::new(),
                },
                &[Ty::Word]
            )
            .is_ok()
    );
    assert!(
        builder
            .push_op(
                OpKind::Builtin {
                    name: "identity".into(),
                    args: Vec::new(),
                },
                &[Ty::Word],
            )
            .is_ok()
    );
    for prim in [
        Prim::FixnumAdd,
        Prim::FixnumSub,
        Prim::FixnumMul,
        Prim::FixnumEq,
        Prim::Eq,
        Prim::Eql,
        Prim::FixnumLt,
        Prim::FixnumLe,
        Prim::Car,
        Prim::Cdr,
        Prim::Svref,
        Prim::Aref,
        Prim::Rplaca,
        Prim::Rplacd,
        Prim::Aset,
    ] {
        assert!(
            builder
                .push_op(
                    OpKind::Prim {
                        op: prim,
                        args: vec![moved, loaded],
                        condition: None,
                    },
                    &[Ty::Word],
                )
                .is_ok()
        );
    }
    assert!(
        builder
            .push_op(
                OpKind::Compare {
                    op: ncl_ir::Compare::Ge,
                    left: moved,
                    right: loaded,
                },
                &[Ty::Bool],
            )
            .is_ok()
    );
    assert!(
        builder
            .push_op(
                OpKind::SetMultipleValues {
                    values: vec![moved, loaded],
                },
                &[Ty::Word],
            )
            .is_ok()
    );
    assert!(builder.push_op(OpKind::Safepoint, &[]).is_ok());
    assert!(
        builder
            .terminate(Terminator::Return {
                values: vec![value],
            })
            .is_ok()
    );

    let compiled = compile_function(&builder.finish(), &Aarch64FixtureAbi)
        .expect("generic lowering operations must compile");
    assert!(!compiled.code.is_empty());
    assert_eq!(compiled.code.last(), Some(&0xc3));
    assert_eq!(compiled.safepoint_maps.len(), 5);
    assert!(
        compiled
            .safepoint_maps
            .iter()
            .any(|map| map.map_flags & FLAG_ALLOCATION_SLOW != 0)
    );
    assert_eq!(compiled.debug.len(), 30);
}
