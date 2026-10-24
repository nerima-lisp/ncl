use super::*;
use crate::FunctionPass;
use ncl_ir::{Constant, Convert, FunctionBuilder, FunctionId, Terminator, Ty};

fn arithmetic_function() -> Function {
    let mut builder = FunctionBuilder::new(
        FunctionId(1),
        "arithmetic",
        vec![ncl_ir::Param {
            name: "argument".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let constant = builder.add_constant(Constant::Fixnum(2));
    let raw = builder
        .push_op(OpKind::Const { result: constant }, &[Ty::I64])
        .unwrap()[0];
    let tagged = builder
        .push_op(
            OpKind::Convert {
                op: Convert::I64ToWord,
                value: raw,
            },
            &[Ty::Word],
        )
        .unwrap()[0];
    builder.push_op(OpKind::Safepoint, &[]).unwrap();
    let result = builder
        .push_op(
            OpKind::Builtin {
                name: "+".into(),
                args: vec![ValueId(0), tagged],
                specialization: None,
            },
            &[Ty::Word],
        )
        .unwrap()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .unwrap();
    builder.finish()
}

#[test]
fn pass_specializes_direct_arithmetic_and_keeps_guards_for_unknown_inputs() {
    let mut function = arithmetic_function();
    let facts = FunctionFacts::analyze(&function);
    assert_eq!(
        facts.values.get(&ValueId(2)),
        Some(&ScalarFact::Known {
            ty: ScalarType::Fixnum,
            evidence: Evidence::Proven,
        })
    );
    let mut pass = TypeEffect;
    assert!(pass.run(&mut function, &Module::default()).unwrap());

    let specialization = function.blocks[0].ops[3].kind.clone();
    let OpKind::Builtin {
        specialization: Some(specialization),
        ..
    } = specialization
    else {
        panic!("arithmetic builtin was not specialized");
    };
    assert_eq!(specialization.operation(), FixnumOp::Add);
    assert!(specialization.needs_tag_check(0));
    assert!(!specialization.needs_tag_check(1));
    assert!(ncl_ir::verify(&function).is_ok());
}

#[test]
fn pass_keeps_tag_check_for_a_catch_payload() {
    let mut builder =
        FunctionBuilder::new(FunctionId(9), "catch-payload", Vec::new(), vec![Ty::Word]);
    let tag = builder.add_constant(Constant::Nil);
    let tag = builder
        .push_op(OpKind::Const { result: tag }, &[Ty::Word])
        .unwrap()[0];
    let raw = builder.add_constant(Constant::Fixnum(2));
    let raw = builder
        .push_op(OpKind::Const { result: raw }, &[Ty::I64])
        .unwrap()[0];
    let known_fixnum = builder
        .push_op(
            OpKind::Convert {
                op: Convert::I64ToWord,
                value: raw,
            },
            &[Ty::Word],
        )
        .unwrap()[0];
    let branch = builder
        .push_op(
            OpKind::Compare {
                op: ncl_ir::Compare::Eq,
                left: raw,
                right: raw,
            },
            &[Ty::Bool],
        )
        .unwrap()[0];
    builder
        .push_op(
            OpKind::EnterHandler {
                region: ncl_ir::HandlerRegionId(0),
            },
            &[],
        )
        .unwrap();
    builder
        .terminate(Terminator::Jump {
            target: ncl_ir::BlockId(1),
            args: Vec::new(),
        })
        .unwrap();

    let protected = builder.create_block(Vec::new());
    let handler = builder.create_block(vec![(Ty::Word, ValueId(100)), (Ty::Word, ValueId(101))]);
    let thrower = builder.create_block(Vec::new());
    builder.position_at(protected).unwrap();
    builder
        .terminate(Terminator::Branch {
            condition: branch,
            then_target: handler,
            then_args: vec![known_fixnum, known_fixnum],
            else_target: thrower,
            else_args: Vec::new(),
        })
        .unwrap();
    builder.position_at(handler).unwrap();
    builder
        .push_op(
            OpKind::LeaveHandler {
                region: ncl_ir::HandlerRegionId(0),
            },
            &[],
        )
        .unwrap();
    builder.push_op(OpKind::Safepoint, &[]).unwrap();
    let result = builder
        .push_op(
            OpKind::Builtin {
                name: "+".into(),
                args: vec![ValueId(100), ValueId(101)],
                specialization: None,
            },
            &[Ty::Word],
        )
        .unwrap()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .unwrap();
    builder.position_at(thrower).unwrap();
    builder
        .terminate(Terminator::Throw { condition: tag })
        .unwrap();
    builder.add_handler_region(ncl_ir::HandlerRegion {
        id: ncl_ir::HandlerRegionId(0),
        kind: ncl_ir::HandlerKind::Catch,
        protected: vec![protected, thrower],
        handler,
        cleanup: None,
        catch_tag: Some(tag),
        binding_targets: vec![known_fixnum],
        depth: 0,
        parent: None,
    });
    let mut function = builder.finish();

    let mut pass = TypeEffect;
    assert!(pass.run(&mut function, &Module::default()).unwrap());
    let OpKind::Builtin {
        specialization: Some(specialization),
        ..
    } = &function.blocks[handler.0 as usize].ops[2].kind
    else {
        panic!("catch arithmetic builtin was not specialized");
    };
    assert!(specialization.needs_tag_check(0));
    assert!(!specialization.needs_tag_check(1));
    assert!(ncl_ir::verify(&function).is_ok());
}

#[test]
fn pass_does_not_trust_progv_binding_targets_as_handler_inputs() {
    let mut builder =
        FunctionBuilder::new(FunctionId(10), "progv-handler", Vec::new(), vec![Ty::Word]);
    let tag = builder.add_constant(Constant::Nil);
    let tag = builder
        .push_op(OpKind::Const { result: tag }, &[Ty::Word])
        .unwrap()[0];
    let constant = builder.add_constant(Constant::Fixnum(3));
    let raw = builder
        .push_op(OpKind::Const { result: constant }, &[Ty::I64])
        .unwrap()[0];
    let known_fixnum = builder
        .push_op(
            OpKind::Convert {
                op: Convert::I64ToWord,
                value: raw,
            },
            &[Ty::Word],
        )
        .unwrap()[0];
    let branch = builder
        .push_op(
            OpKind::Compare {
                op: ncl_ir::Compare::Eq,
                left: raw,
                right: raw,
            },
            &[Ty::Bool],
        )
        .unwrap()[0];
    builder
        .push_op(
            OpKind::EnterHandler {
                region: ncl_ir::HandlerRegionId(0),
            },
            &[],
        )
        .unwrap();
    builder
        .terminate(Terminator::Jump {
            target: ncl_ir::BlockId(1),
            args: Vec::new(),
        })
        .unwrap();

    let protected = builder.create_block(Vec::new());
    let handler = builder.create_block(vec![(Ty::Word, ValueId(100)), (Ty::Word, ValueId(101))]);
    let thrower = builder.create_block(Vec::new());
    builder.position_at(protected).unwrap();
    builder
        .terminate(Terminator::Branch {
            condition: branch,
            then_target: handler,
            then_args: vec![known_fixnum, known_fixnum],
            else_target: thrower,
            else_args: Vec::new(),
        })
        .unwrap();
    builder.position_at(handler).unwrap();
    builder
        .push_op(
            OpKind::LeaveHandler {
                region: ncl_ir::HandlerRegionId(0),
            },
            &[],
        )
        .unwrap();
    builder.push_op(OpKind::Safepoint, &[]).unwrap();
    let result = builder
        .push_op(
            OpKind::Builtin {
                name: "+".into(),
                args: vec![ValueId(101), known_fixnum],
                specialization: None,
            },
            &[Ty::Word],
        )
        .unwrap()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .unwrap();
    builder.position_at(thrower).unwrap();
    builder
        .terminate(Terminator::Throw { condition: tag })
        .unwrap();
    builder.add_handler_region(ncl_ir::HandlerRegion {
        id: ncl_ir::HandlerRegionId(0),
        kind: ncl_ir::HandlerKind::Progv,
        protected: vec![protected, thrower],
        handler,
        cleanup: None,
        catch_tag: None,
        binding_targets: vec![known_fixnum],
        depth: 0,
        parent: None,
    });
    let mut function = builder.finish();

    let mut pass = TypeEffect;
    assert!(pass.run(&mut function, &Module::default()).unwrap());
    let OpKind::Builtin {
        specialization: Some(specialization),
        ..
    } = &function.blocks[handler.0 as usize].ops[2].kind
    else {
        panic!("progv arithmetic builtin was not specialized");
    };
    assert!(specialization.needs_tag_check(0));
    assert!(!specialization.needs_tag_check(1));
    let errors = ncl_ir::verify(&function).unwrap_err();
    assert!(errors.contains(&ncl_ir::VerifyError::HandlerMismatch(handler)));
}

#[test]
fn unknown_builtin_results_and_effects_remain_conservative() {
    let mut builder = FunctionBuilder::new(
        FunctionId(3),
        "unknown-builtin",
        vec![ncl_ir::Param {
            name: "argument".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    builder.push_op(OpKind::Safepoint, &[]).unwrap();
    let unknown = builder
        .push_op(
            OpKind::Builtin {
                name: "unknown-call".into(),
                args: vec![ValueId(0)],
                specialization: None,
            },
            &[Ty::Word],
        )
        .unwrap()[0];
    let constant = builder.add_constant(Constant::Fixnum(2));
    let raw = builder
        .push_op(OpKind::Const { result: constant }, &[Ty::I64])
        .unwrap()[0];
    let tagged = builder
        .push_op(
            OpKind::Convert {
                op: Convert::I64ToWord,
                value: raw,
            },
            &[Ty::Word],
        )
        .unwrap()[0];
    builder.push_op(OpKind::Safepoint, &[]).unwrap();
    let result = builder
        .push_op(
            OpKind::Builtin {
                name: "+".into(),
                args: vec![unknown, tagged],
                specialization: None,
            },
            &[Ty::Word],
        )
        .unwrap()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .unwrap();
    let function = builder.finish();

    let facts = FunctionFacts::analyze(&function);
    assert_eq!(facts.values.get(&unknown), None);
    assert_eq!(
        facts.effects.get(&(ncl_ir::BlockId(0), 1)),
        Some(&EffectSet::UNKNOWN_CALL)
    );
    assert!(EffectSet::UNKNOWN_CALL.contains(Effect::ReadMemory));
    assert_eq!(
        facts.values_arity.get(&(ncl_ir::BlockId(0), 1)),
        Some(&ValuesArity::Unknown)
    );
}

#[test]
fn object_constants_do_not_claim_a_non_numeric_type() {
    let mut builder = FunctionBuilder::new(
        FunctionId(8),
        "object-constant-type",
        Vec::new(),
        vec![Ty::Word],
    );
    let referenced = builder.add_constant(Constant::Fixnum(4));
    let object = builder.add_constant(Constant::Object(referenced));
    let value = builder
        .push_op(OpKind::Const { result: object }, &[Ty::Word])
        .unwrap()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![value],
        })
        .unwrap();

    let facts = FunctionFacts::analyze(&builder.finish());
    assert_eq!(facts.values.get(&value), None);
}

#[test]
fn branch_join_keeps_only_fixnum_facts_shared_by_all_inputs() {
    let mut builder = FunctionBuilder::new(
        FunctionId(4),
        "fixnum-branch-join",
        Vec::new(),
        vec![Ty::Word],
    );
    let left_constant = builder.add_constant(Constant::Fixnum(2));
    let left_raw = builder
        .push_op(
            OpKind::Const {
                result: left_constant,
            },
            &[Ty::I64],
        )
        .unwrap()[0];
    let left = builder
        .push_op(
            OpKind::Convert {
                op: Convert::I64ToWord,
                value: left_raw,
            },
            &[Ty::Word],
        )
        .unwrap()[0];
    let right_constant = builder.add_constant(Constant::Fixnum(3));
    let right_raw = builder
        .push_op(
            OpKind::Const {
                result: right_constant,
            },
            &[Ty::I64],
        )
        .unwrap()[0];
    let right = builder
        .push_op(
            OpKind::Convert {
                op: Convert::I64ToWord,
                value: right_raw,
            },
            &[Ty::Word],
        )
        .unwrap()[0];
    let condition = builder
        .push_op(
            OpKind::Compare {
                op: ncl_ir::Compare::Eq,
                left: left_raw,
                right: right_raw,
            },
            &[Ty::Bool],
        )
        .unwrap()[0];
    let then_block = builder.create_block(Vec::new());
    let else_block = builder.create_block(Vec::new());
    let merge_block = builder.create_block(vec![(Ty::Word, ValueId(5))]);
    builder.position_at(ncl_ir::BlockId(0)).unwrap();
    builder
        .terminate(Terminator::Branch {
            condition,
            then_target: then_block,
            then_args: Vec::new(),
            else_target: else_block,
            else_args: Vec::new(),
        })
        .unwrap();
    builder.position_at(then_block).unwrap();
    builder
        .terminate(Terminator::Jump {
            target: merge_block,
            args: vec![left],
        })
        .unwrap();
    builder.position_at(else_block).unwrap();
    builder
        .terminate(Terminator::Jump {
            target: merge_block,
            args: vec![right],
        })
        .unwrap();
    builder.position_at(merge_block).unwrap();
    builder.push_op(OpKind::Safepoint, &[]).unwrap();
    let result = builder
        .push_op(
            OpKind::Builtin {
                name: "+".into(),
                args: vec![ValueId(5), ValueId(5)],
                specialization: None,
            },
            &[Ty::Word],
        )
        .unwrap()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .unwrap();
    let mut function = builder.finish();

    let facts = FunctionFacts::analyze(&function);
    assert_eq!(
        facts.values.get(&ValueId(5)),
        Some(&ScalarFact::Known {
            ty: ScalarType::Fixnum,
            evidence: Evidence::Proven,
        })
    );
    TypeEffect.run(&mut function, &Module::default()).unwrap();
    let OpKind::Builtin {
        specialization: Some(specialization),
        ..
    } = &function.blocks[3].ops[1].kind
    else {
        panic!("join builtin was not specialized");
    };
    assert!(!specialization.needs_tag_check(0));
    assert!(!specialization.needs_tag_check(1));
    assert!(ncl_ir::verify(&function).is_ok());
}

#[test]
fn branch_join_drops_fixnum_fact_when_one_input_is_unknown() {
    let mut builder = FunctionBuilder::new(
        FunctionId(5),
        "mixed-fixnum-branch-join",
        vec![
            ncl_ir::Param {
                name: "condition".into(),
                ty: Ty::Bool,
            },
            ncl_ir::Param {
                name: "unknown".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let constant = builder.add_constant(Constant::Fixnum(2));
    let raw = builder
        .push_op(OpKind::Const { result: constant }, &[Ty::I64])
        .unwrap()[0];
    let tagged = builder
        .push_op(
            OpKind::Convert {
                op: Convert::I64ToWord,
                value: raw,
            },
            &[Ty::Word],
        )
        .unwrap()[0];
    let then_block = builder.create_block(Vec::new());
    let else_block = builder.create_block(Vec::new());
    let merge_block = builder.create_block(vec![(Ty::Word, ValueId(4))]);
    builder.position_at(ncl_ir::BlockId(0)).unwrap();
    builder
        .terminate(Terminator::Branch {
            condition: ValueId(0),
            then_target: then_block,
            then_args: Vec::new(),
            else_target: else_block,
            else_args: Vec::new(),
        })
        .unwrap();
    builder.position_at(then_block).unwrap();
    builder
        .terminate(Terminator::Jump {
            target: merge_block,
            args: vec![tagged],
        })
        .unwrap();
    builder.position_at(else_block).unwrap();
    builder
        .terminate(Terminator::Jump {
            target: merge_block,
            args: vec![ValueId(1)],
        })
        .unwrap();
    builder.position_at(merge_block).unwrap();
    builder.push_op(OpKind::Safepoint, &[]).unwrap();
    let result = builder
        .push_op(
            OpKind::Builtin {
                name: "+".into(),
                args: vec![ValueId(4), tagged],
                specialization: None,
            },
            &[Ty::Word],
        )
        .unwrap()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .unwrap();
    let mut function = builder.finish();

    let facts = FunctionFacts::analyze(&function);
    assert_eq!(facts.values.get(&ValueId(4)), None);
    TypeEffect.run(&mut function, &Module::default()).unwrap();
    let OpKind::Builtin {
        specialization: Some(specialization),
        ..
    } = &function.blocks[3].ops[1].kind
    else {
        panic!("join builtin was not specialized");
    };
    assert!(specialization.needs_tag_check(0));
    assert!(!specialization.needs_tag_check(1));
    assert!(ncl_ir::verify(&function).is_ok());
}

#[test]
fn values_arity_keeps_explicit_sets_and_treats_returns_as_unknown() {
    let mut builder = FunctionBuilder::new(
        FunctionId(2),
        "values-arity",
        vec![
            ncl_ir::Param {
                name: "first".into(),
                ty: Ty::Word,
            },
            ncl_ir::Param {
                name: "second".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    builder
        .push_op(
            OpKind::SetMultipleValues {
                values: vec![ValueId(0), ValueId(1)],
            },
            &[],
        )
        .unwrap();
    builder
        .terminate(Terminator::Return {
            values: vec![ValueId(0)],
        })
        .unwrap();
    let facts = FunctionFacts::analyze(&builder.finish());

    assert_eq!(
        facts.values_arity.get(&(ncl_ir::BlockId(0), 0)),
        Some(&ValuesArity::Exact(2))
    );
    assert_eq!(
        facts.values_arity.get(&(ncl_ir::BlockId(0), 1)),
        Some(&ValuesArity::Unknown)
    );
}

#[test]
fn terminators_record_call_and_signal_effects() {
    let mut call_return = FunctionBuilder::new(
        FunctionId(9),
        "call-return-effect",
        vec![ncl_ir::Param {
            name: "function".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    call_return
        .terminate(Terminator::CallReturn {
            function: ValueId(0),
            args: Vec::new(),
        })
        .unwrap();
    let facts = FunctionFacts::analyze(&call_return.finish());
    assert_eq!(
        facts.effects.get(&(ncl_ir::BlockId(0), 0)),
        Some(&EffectSet::UNKNOWN_CALL)
    );

    let mut call = FunctionBuilder::new(
        FunctionId(6),
        "tail-call-effect",
        vec![ncl_ir::Param {
            name: "function".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    call.terminate(Terminator::TailCall {
        function: ValueId(0),
        args: Vec::new(),
    })
    .unwrap();
    let facts = FunctionFacts::analyze(&call.finish());
    assert_eq!(
        facts.effects.get(&(ncl_ir::BlockId(0), 0)),
        Some(&EffectSet::UNKNOWN_CALL)
    );
    assert_eq!(
        facts.values_arity.get(&(ncl_ir::BlockId(0), 0)),
        Some(&ValuesArity::Unknown)
    );

    let mut throw = FunctionBuilder::new(
        FunctionId(7),
        "throw-effect",
        vec![ncl_ir::Param {
            name: "condition".into(),
            ty: Ty::Word,
        }],
        Vec::new(),
    );
    throw
        .terminate(Terminator::Throw {
            condition: ValueId(0),
        })
        .unwrap();
    let facts = FunctionFacts::analyze(&throw.finish());
    let throw_effect = facts
        .effects
        .get(&(ncl_ir::BlockId(0), 0))
        .expect("throw effect");
    assert!(throw_effect.contains(Effect::Signal));
    assert!(!throw_effect.contains(Effect::Call));
}
