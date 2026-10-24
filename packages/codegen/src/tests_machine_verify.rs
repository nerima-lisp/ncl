#![allow(missing_docs, clippy::unwrap_used)]

use crate::{
    AbiError, Allocation, AllocationTarget, Block, CodegenError, ContextField, FrameLayout,
    Location, MachineFunction, MachineOp, MachineVerificationError, RuntimeAbi, RuntimeFunction,
    SafepointMap, allocate,
};
use ncl_ir::{
    Constant, ConstantIndex, Function, FunctionBuilder, FunctionId, OpKind, Terminator, Ty, ValueId,
};

struct EmptyAbi;

impl RuntimeAbi for EmptyAbi {
    fn builtin_address(&self, id: ncl_object::BuiltinIdentifier) -> Result<u64, AbiError> {
        Err(AbiError::MissingBuiltin(id))
    }

    fn field_offset(&self, field: ContextField) -> Result<i32, AbiError> {
        Err(AbiError::UnsupportedContextField(field))
    }

    fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, AbiError> {
        Err(AbiError::UnsupportedRuntimeFunction(function))
    }
}

fn simple_function() -> Function {
    let mut builder = FunctionBuilder::new(FunctionId(0), "verify", Vec::new(), vec![Ty::Word]);
    let constant = builder.add_constant(Constant::T);
    let value = builder
        .push_op(OpKind::Const { result: constant }, &[Ty::Word])
        .unwrap()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![value],
        })
        .unwrap();
    builder.finish()
}

fn machine_function(function: &Function) -> MachineFunction {
    let mut slots = Vec::new();
    let mut next_slot = crate::FRAME_HEADER_WORDS;
    for block in &function.blocks {
        for parameter in &block.params {
            slots.push((parameter.value, next_slot));
            next_slot += 1;
        }
        for op in &block.ops {
            for (value, _) in &op.results {
                slots.push((*value, next_slot));
                next_slot += 1;
            }
        }
    }
    let blocks = function
        .blocks
        .iter()
        .map(|source| {
            let operations = source
                .ops
                .iter()
                .map(|op| {
                    op.results.first().map_or(MachineOp::Return, |(value, _)| {
                        let slot = slots
                            .iter()
                            .find(|(id, _)| id == value)
                            .map_or(0, |(_, slot)| *slot);
                        MachineOp::move_value(slot, slot)
                    })
                })
                .chain(
                    matches!(source.terminator, Terminator::Return { .. })
                        .then_some(MachineOp::Return),
                )
                .collect();
            Block::new(source.id, operations)
        })
        .collect();
    let local_words = u32::try_from(slots.len()).unwrap();
    let frame = FrameLayout::new(
        u32::try_from(function.params.len()).unwrap(),
        local_words,
        0,
    )
    .unwrap();
    MachineFunction::new(
        function.blocks[0].id,
        blocks,
        frame,
        Vec::new(),
        Vec::new(),
        slots,
    )
}

fn verify_machine(
    function: &Function,
    machine: &MachineFunction,
    maps: &[SafepointMap],
) -> Result<(), CodegenError> {
    crate::machine_verify::verify_machine_function(function, machine, maps, 0, 1)
}

fn verify_aarch64(function: &Function, allocation: &Allocation) -> Result<(), CodegenError> {
    let frame = crate::FrameLayout::new(0, allocation.spill_words, 0)?;
    crate::machine_verify::verify_allocation(
        function,
        AllocationTarget::AArch64,
        allocation,
        frame,
        0,
        0,
        None,
    )
}

#[test]
fn accepts_allocator_output_and_safepoint_warnings() {
    let mut function = simple_function();
    function.blocks[0].ops.push(ncl_ir::Op {
        results: vec![(ValueId(1), Ty::Word)],
        kind: OpKind::Call {
            function: ValueId(0),
            args: Vec::new(),
        },
        loc: None,
    });
    assert!(matches!(
        ncl_ir::verify(&function),
        Err(errors) if errors.iter().any(|error| matches!(error, ncl_ir::VerifyError::SafepointWarning(_)))
    ));
    let mut allocation = allocate(&function, AllocationTarget::AArch64);
    allocation.outgoing_base = allocation.spill_words;
    assert!(verify_aarch64(&function, &allocation).is_ok());
}

#[test]
fn rejects_missing_duplicate_and_invalid_locations() {
    let function = simple_function();
    let allocation = allocate(&function, AllocationTarget::AArch64);

    let mut missing = allocation.clone();
    missing.locations.pop();
    assert!(matches!(
        verify_aarch64(&function, &missing),
        Err(CodegenError::MachineVerification(
            MachineVerificationError::LocationMismatch
        ))
    ));

    let mut duplicate = allocation.clone();
    duplicate.locations.push(duplicate.locations[0]);
    assert!(matches!(
        verify_aarch64(&function, &duplicate),
        Err(CodegenError::MachineVerification(
            MachineVerificationError::LocationMismatch
        ))
    ));

    let mut invalid_register = allocation.clone();
    invalid_register.locations[0].1 = Location::Register(31);
    assert!(matches!(
        verify_aarch64(&function, &invalid_register),
        Err(CodegenError::MachineVerification(
            MachineVerificationError::LocationMismatch
        ))
    ));

    let mut invalid_spill = allocation;
    invalid_spill.locations[0].1 = Location::Spill(1);
    invalid_spill.spill_words = 1;
    assert!(matches!(
        verify_aarch64(&function, &invalid_spill),
        Err(CodegenError::MachineVerification(
            MachineVerificationError::LocationMismatch
        ))
    ));
}

#[test]
fn rejects_use_before_definition() {
    let mut function = simple_function();
    function.blocks[0].terminator = Terminator::Return {
        values: vec![ValueId(99)],
    };
    let allocation = allocate(&function, AllocationTarget::AArch64);
    assert!(matches!(
        verify_aarch64(&function, &allocation),
        Err(CodegenError::MachineVerification(
            MachineVerificationError::InvalidIr(_)
        ))
    ));
}

#[test]
fn rejects_frame_area_mismatch_and_omits_raw_address_roots() {
    let function = simple_function();
    let allocation = allocate(&function, AllocationTarget::AArch64);
    let oversized = crate::FrameLayout::new(0, 1, 0).unwrap();
    assert!(matches!(
        crate::machine_verify::verify_allocation(
            &function,
            AllocationTarget::AArch64,
            &allocation,
            oversized,
            0,
            0,
            None,
        ),
        Err(CodegenError::MachineVerification(
            MachineVerificationError::FrameMismatch
        ))
    ));

    let mut address_allocation = allocation;
    address_allocation.intervals[0].ty = Ty::Address;
    let (registers, slots) =
        crate::machine_verify::root_locations_at(&function, &address_allocation, 1, 0);
    assert!(registers.is_empty());
    assert!(slots.is_empty());
}

#[test]
fn rejects_call_crossing_register_and_safepoint_mismatch() {
    let mut builder = FunctionBuilder::new(
        FunctionId(1),
        "call-crossing",
        vec![
            ncl_ir::Param {
                name: "callee".into(),
                ty: Ty::Word,
            },
            ncl_ir::Param {
                name: "live".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    builder
        .push_op(
            OpKind::Call {
                function: ValueId(0),
                args: Vec::new(),
            },
            &[Ty::Word],
        )
        .unwrap();
    builder
        .terminate(Terminator::Return {
            values: vec![ValueId(1)],
        })
        .unwrap();
    let function = builder.finish();
    let allocation = allocate(&function, AllocationTarget::AArch64);
    assert!(
        allocation
            .intervals
            .iter()
            .any(|interval| { interval.value == ValueId(1) && interval.crosses_call })
    );
    assert!(matches!(
        allocation.location(ValueId(1)),
        Some(Location::Spill(_))
    ));

    let mut call_register = allocation.clone();
    let live = call_register
        .locations
        .iter_mut()
        .find(|(value, _)| *value == ValueId(1));
    live.unwrap().1 = Location::Register(6);
    assert!(matches!(
        verify_aarch64(&function, &call_register),
        Err(CodegenError::MachineVerification(
            MachineVerificationError::LocationMismatch
        ))
    ));

    let mut bad_map = allocation;
    bad_map.safepoint_registers.clear();
    assert!(matches!(
        verify_aarch64(&function, &bad_map),
        Err(CodegenError::MachineVerification(
            MachineVerificationError::SafepointMismatch
        ))
    ));
}

#[test]
fn rejects_constant_index_before_emission() {
    let mut builder = FunctionBuilder::new(FunctionId(30), "bad-constant", Vec::new(), Vec::new());
    builder
        .push_op(
            OpKind::Const {
                result: ConstantIndex(1),
            },
            &[Ty::Word],
        )
        .unwrap();
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .unwrap();
    let function = builder.finish();
    assert!(matches!(
        crate::compile_function_aarch64(&function, &EmptyAbi),
        Err(CodegenError::MachineVerification(
            MachineVerificationError::InvalidIr(_)
        ))
    ));
}

#[test]
fn verifies_fixed_slot_machine_and_rejects_corrupt_slots_frame_and_maps() {
    let mut function = simple_function();
    function.blocks[0].ops.push(ncl_ir::Op {
        results: Vec::new(),
        kind: OpKind::Safepoint,
        loc: None,
    });
    let mut machine = machine_function(&function);
    assert!(matches!(
        verify_machine(&function, &machine, &[]),
        Err(CodegenError::MachineVerification(
            MachineVerificationError::MachineSafepointMismatch
        ))
    ));

    let mut bad_slot = machine.clone();
    bad_slot.slots[0].1 = 3;
    assert!(matches!(
        verify_machine(&function, &bad_slot, &[]),
        Err(CodegenError::MachineVerification(
            MachineVerificationError::MachineSlotMismatch
        ))
    ));

    let mut bad_frame = machine.clone();
    bad_frame.frame = FrameLayout::new(0, 2, 0).unwrap();
    assert!(matches!(
        verify_machine(&function, &bad_frame, &[]),
        Err(CodegenError::MachineVerification(
            MachineVerificationError::FrameMismatch
        ))
    ));

    machine.blocks[0].operations[0] = MachineOp::move_value(4, 5);
    assert!(matches!(
        verify_machine(&function, &machine, &[]),
        Err(CodegenError::MachineVerification(
            MachineVerificationError::MachineStructureMismatch
        ))
    ));

    let machine = machine_function(&function);
    let frame_words = u16::try_from(machine.frame.frame_words).unwrap();
    let live_slots = machine
        .slots
        .iter()
        .map(|(_, slot)| u16::try_from(*slot).unwrap())
        .collect::<Vec<_>>();
    let mut map = SafepointMap::new(
        0,
        frame_words,
        frame_words,
        &live_slots,
        &[],
        crate::FLAG_CALL,
    )
    .unwrap();
    let mut maps = vec![map.clone()];
    assert!(verify_machine(&function, &machine, &maps).is_ok());

    let mut wrong_flags = maps.clone();
    wrong_flags[0].map_flags = crate::FLAG_LOOP_BACKEDGE;
    assert!(matches!(
        verify_machine(&function, &machine, &wrong_flags),
        Err(CodegenError::MachineVerification(
            MachineVerificationError::MachineSafepointMismatch
        ))
    ));

    map.bitmap[0] ^= 1 << 4;
    maps[0] = map;
    assert!(matches!(
        verify_machine(&function, &machine, &maps),
        Err(CodegenError::MachineVerification(
            MachineVerificationError::MachineSafepointMismatch
        ))
    ));

    let mut invalid_ir = function.clone();
    invalid_ir.blocks[0].terminator = Terminator::Return {
        values: vec![ValueId(99)],
    };
    assert!(matches!(
        verify_machine(&invalid_ir, &machine_function(&invalid_ir), &[]),
        Err(CodegenError::MachineVerification(
            MachineVerificationError::InvalidIr(_)
        ))
    ));

    maps[0].pc_offset = 2;
    assert!(matches!(
        verify_machine(&function, &machine, &maps),
        Err(CodegenError::MachineVerification(
            MachineVerificationError::MachineCodeBoundsMismatch
        ))
    ));
}

#[test]
fn verifies_target_map_spill_incoming_and_outgoing_roots() {
    let mut builder = FunctionBuilder::new(
        FunctionId(31),
        "map-verifier",
        vec![ncl_ir::Param {
            name: "arg".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    builder.push_op(OpKind::Safepoint, &[]).unwrap();
    builder
        .terminate(Terminator::Return {
            values: vec![ValueId(0)],
        })
        .unwrap();
    let function = builder.finish();
    let mut allocation = allocate(&function, AllocationTarget::AArch64);
    assert!(matches!(
        allocation.location(ValueId(0)),
        Some(Location::Spill(_))
    ));
    allocation.incoming_args_base = Some(allocation.spill_words);
    allocation.outgoing_base = allocation.spill_words + 5;
    let frame = FrameLayout::new(0, allocation.spill_words + 5, 2).unwrap();
    let frame_words = u16::try_from(frame.frame_words).unwrap();
    let live_slots = (4_u16..12).collect::<Vec<_>>();
    let mut map = SafepointMap::new(
        17,
        frame_words,
        frame_words,
        &live_slots,
        &[],
        crate::FLAG_CALL,
    )
    .unwrap();
    assert!(
        crate::machine_verify::verify_safepoint_map(
            &function,
            &allocation,
            frame,
            0,
            0,
            crate::FLAG_CALL,
            &map,
        )
        .is_ok()
    );

    map.bitmap[0] &= !(1 << 5);
    assert_eq!(
        crate::machine_verify::verify_safepoint_map(
            &function,
            &allocation,
            frame,
            0,
            0,
            crate::FLAG_CALL,
            &map,
        ),
        Err(MachineVerificationError::SafepointMismatch)
    );
}

fn address_live_across_safepoint() -> Function {
    let mut builder = FunctionBuilder::new(
        FunctionId(1),
        "address-live-across-safepoint",
        vec![ncl_ir::Param {
            name: "address".into(),
            ty: Ty::Address,
        }],
        vec![Ty::Word],
    );
    builder.push_op(OpKind::Safepoint, &[]).unwrap();
    let loaded = builder
        .push_op(
            OpKind::Load {
                address: ValueId(0),
            },
            &[Ty::Word],
        )
        .unwrap()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![loaded],
        })
        .unwrap();
    builder.finish()
}

fn reports_address_live_across_safepoint(errors: &[ncl_ir::VerifyError]) -> bool {
    errors.iter().any(|error| {
        matches!(
            error,
            ncl_ir::VerifyError::AddressLiveAcross {
                block: ncl_ir::BlockId(0),
                value: ValueId(0),
                boundary: ncl_ir::AddressLivenessBoundary::Safepoint,
            }
        )
    })
}

#[test]
fn both_target_codegen_entries_reject_address_live_across_safepoint() {
    let function = address_live_across_safepoint();
    assert!(matches!(
        ncl_ir::verify(&function),
        Err(errors) if reports_address_live_across_safepoint(&errors)
    ));

    assert!(matches!(
        crate::compile_function_x86_64(&function, &EmptyAbi),
        Err(CodegenError::MachineVerification(
            MachineVerificationError::InvalidIr(errors)
        )) if reports_address_live_across_safepoint(&errors)
    ));
    assert!(matches!(
        crate::compile_function_aarch64(&function, &EmptyAbi),
        Err(CodegenError::MachineVerification(
            MachineVerificationError::InvalidIr(errors)
        )) if reports_address_live_across_safepoint(&errors)
    ));
}
