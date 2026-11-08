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
        .push_op(OpKind::Const { result: argc }, &[Ty::I64])
        .ok()
        .and_then(|values| values.first().copied())
    else {
        return Err("argc constant did not lower".into());
    };
    let Some(argc) = builder
        .push_op(
            OpKind::Convert {
                op: ncl_ir::Convert::I64ToWord,
                value: argc,
            },
            &[Ty::Word],
        )
        .ok()
        .and_then(|values| values.first().copied())
    else {
        return Err("argc conversion did not lower".into());
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

    let Ok(compiled) = compile_function_aarch64(&builder.finish(), &Aarch64FixtureAbi::default())
    else {
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
    if *rest_roots & 0b1100 != 0b1100 {
        return Err("rest roots were not marked".into());
    }
    Ok(())
}

#[test]
fn aarch64_closure_call_acquire_loads_function_entry() -> Result<(), String> {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(34),
        "aarch64-acquire-function-entry",
        vec![
            ncl_ir::Param {
                name: "callee".into(),
                ty: Ty::Word,
            },
            ncl_ir::Param {
                name: "argc".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let Some(result) = builder
        .push_op(
            OpKind::CallClosure {
                closure: ncl_ir::ValueId(0),
                args: vec![ncl_ir::ValueId(1)],
                named_symbol: None,
            },
            &[Ty::Word],
        )
        .ok()
        .and_then(|values| values.first().copied())
    else {
        return Err("closure call did not lower".into());
    };
    if builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .is_err()
    {
        return Err("return terminator did not lower".into());
    }

    let Ok(compiled) = compile_function_aarch64(&builder.finish(), &Aarch64FixtureAbi::default())
    else {
        return Err("closure call did not compile".into());
    };
    let expected = encode_aarch64([
        ncl_asm_aarch64::Inst::AddImm {
            rd: ncl_asm_aarch64::RegOrSp::Reg(ncl_asm_aarch64::Reg(17)),
            rn: ncl_asm_aarch64::RegOrSp::Reg(ncl_asm_aarch64::Reg(17)),
            imm: 8,
            shift: false,
        },
        ncl_asm_aarch64::Inst::Ldar {
            rt: ncl_asm_aarch64::Reg(17),
            rn: ncl_asm_aarch64::Reg(17),
        },
        ncl_asm_aarch64::Inst::AsrImm {
            rd: ncl_asm_aarch64::Reg(17),
            rn: ncl_asm_aarch64::Reg(17),
            amount: 1,
        },
    ]);
    assert_code_contains_aarch64(
        &compiled.code,
        &expected,
        "closure call acquire-loads the function entry before decoding it",
    );
    Ok(())
}

#[test]
fn aarch64_function_cell_field_load_uses_acquire() -> Result<(), String> {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(35),
        "aarch64-function-cell-load",
        vec![ncl_ir::Param {
            name: "symbol".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let field = u32::try_from(ncl_object::symbol_offset::FUNCTION)
        .map_err(|error| format!("function-cell field offset: {error}"))?;
    let Some(value) = builder
        .push_op(
            OpKind::LoadField {
                object: ncl_ir::ValueId(0),
                field,
            },
            &[Ty::Word],
        )
        .ok()
        .and_then(|values| values.first().copied())
    else {
        return Err("function-cell field load did not lower".into());
    };
    if builder
        .terminate(Terminator::Return {
            values: vec![value],
        })
        .is_err()
    {
        return Err("return terminator did not lower".into());
    }

    let Ok(compiled) = compile_function_aarch64(&builder.finish(), &Aarch64FixtureAbi::default())
    else {
        return Err("function-cell field load did not compile".into());
    };
    let offset = u16::try_from((ncl_object::symbol_offset::FUNCTION + 1) * 8)
        .map_err(|error| format!("function-cell byte offset: {error}"))?;
    let expected = encode_aarch64([
        ncl_asm_aarch64::Inst::AndImm {
            rd: ncl_asm_aarch64::Reg(16),
            rn: ncl_asm_aarch64::Reg(16),
            imm: !ncl_sys::LOWTAG_MASK,
        },
        ncl_asm_aarch64::Inst::AddImm {
            rd: ncl_asm_aarch64::RegOrSp::Reg(ncl_asm_aarch64::Reg(16)),
            rn: ncl_asm_aarch64::RegOrSp::Reg(ncl_asm_aarch64::Reg(16)),
            imm: offset,
            shift: false,
        },
        ncl_asm_aarch64::Inst::Ldar {
            rt: ncl_asm_aarch64::Reg(16),
            rn: ncl_asm_aarch64::Reg(16),
        },
    ]);
    assert_code_contains_aarch64(
        &compiled.code,
        &expected,
        "function-cell LoadField forms its address and uses LDAR",
    );
    Ok(())
}
