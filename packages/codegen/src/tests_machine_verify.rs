#![allow(missing_docs, clippy::unwrap_used)]

use crate::{
    AbiError, Allocation, AllocationTarget, CodegenError, ContextField, Location,
    MachineVerificationError, RuntimeAbi, RuntimeFunction, allocate,
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
    let constant = builder.add_constant(Constant::Fixnum(7));
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
    let allocation = allocate(&function, AllocationTarget::AArch64);
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
    invalid_spill.locations[0].1 = Location::Spill(u32::MAX);
    invalid_spill.spill_words = u32::MAX;
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
    let mut builder =
        FunctionBuilder::new(FunctionId(30), "bad-constant", Vec::new(), vec![Ty::Word]);
    builder
        .push_op(
            OpKind::Const {
                result: ConstantIndex(1),
            },
            &[Ty::Word],
        )
        .unwrap();
    let function = builder.finish();
    assert!(matches!(
        crate::compile_function_aarch64(&function, &EmptyAbi),
        Err(CodegenError::MachineVerification(
            MachineVerificationError::InvalidIr(_)
        ))
    ));
}
