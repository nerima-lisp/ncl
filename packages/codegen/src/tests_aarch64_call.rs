#![allow(missing_docs)]

use super::*;

#[test]
fn aarch64_call_with_five_arguments_reserves_and_roots_rest_area() -> Result<(), String> {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(33),
        "aarch64-five-argument-call",
        vec![
            ncl_ir::Param {
                name: "callee".into(),
                ty: Ty::Word,
            },
            ncl_ir::Param {
                name: "first".into(),
                ty: Ty::Word,
            },
            ncl_ir::Param {
                name: "second".into(),
                ty: Ty::Word,
            },
            ncl_ir::Param {
                name: "third".into(),
                ty: Ty::Word,
            },
            ncl_ir::Param {
                name: "fourth".into(),
                ty: Ty::Word,
            },
            ncl_ir::Param {
                name: "fifth".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let argc = builder.add_constant(Constant::Fixnum(5));
    let Some(argc) = builder
        .push_op(OpKind::Const { result: argc }, &[Ty::Word])
        .ok()
        .and_then(|values| values.first().copied())
    else {
        return Err("argc constant did not lower".into());
    };
    let Some(result) = builder
        .push_op(
            OpKind::Call {
                function: ncl_ir::ValueId(0),
                args: vec![
                    argc,
                    ncl_ir::ValueId(1),
                    ncl_ir::ValueId(2),
                    ncl_ir::ValueId(3),
                    ncl_ir::ValueId(4),
                    ncl_ir::ValueId(5),
                ],
            },
            &[Ty::Word],
        )
        .ok()
        .and_then(|values| values.first().copied())
    else {
        return Err("call did not lower".into());
    };
    if builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .is_err()
    {
        return Err("return terminator did not lower".into());
    }

    let Ok(compiled) = compile_function_aarch64(&builder.finish(), &Aarch64FixtureAbi) else {
        return Err("five-argument call did not compile".into());
    };
    if compiled.frame_size < 48 {
        return Err("rest area was not reserved".into());
    }
    let Some(map) = compiled.safepoint_maps.first() else {
        return Err("call safepoint map was not emitted".into());
    };
    let Some(rest_roots) = map.bitmap.get(1) else {
        return Err("outgoing roots were not represented".into());
    };
    if *rest_roots & 0b1100 == 0 {
        return Err("rest roots were not marked".into());
    }
    Ok(())
}
