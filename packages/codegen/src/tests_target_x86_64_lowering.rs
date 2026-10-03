use super::{
    ContextField, RuntimeFunction, ValueSlots, load_immediate, load_slot, lower_runtime_builtin,
    runtime_address, slot_mem_of, slots, store_return_values,
};
use crate::{AllocationTarget, CodegenError, RuntimeAbi, allocate};
use ncl_asm_x86_64::{Assembler, Reg};
use ncl_ir::{FunctionBuilder, ValueId};

struct MissingAbi;

impl RuntimeAbi for MissingAbi {
    fn builtin_address(
        &self,
        identifier: ncl_object::BuiltinIdentifier,
    ) -> Result<u64, crate::AbiError> {
        Err(crate::AbiError::MissingBuiltin(identifier))
    }

    fn field_offset(&self, field: ContextField) -> Result<i32, crate::AbiError> {
        Err(crate::AbiError::UnsupportedContextField(field))
    }

    fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, crate::AbiError> {
        Err(crate::AbiError::UnsupportedRuntimeFunction(function))
    }
}

fn empty_slots() -> ValueSlots {
    let function = FunctionBuilder::new(
        ncl_ir::FunctionId(210),
        "empty-slots",
        Vec::new(),
        Vec::new(),
    )
    .finish();
    slots(
        &function,
        0,
        allocate(&function, AllocationTarget::X86_64),
        0,
    )
    .0
}

#[test]
fn immediate_loading_uses_the_short_form_only_when_i32_can_hold_the_value() {
    let mut narrow = Assembler::new();
    load_immediate(&mut narrow, Reg::R10, -1).expect("narrow immediate");
    assert_eq!(narrow.bytes().len(), 7);
    assert_eq!(&narrow.bytes()[..3], &[0x49, 0xc7, 0xc2]);

    let mut wide = Assembler::new();
    load_immediate(&mut wide, Reg::R10, i64::from(i32::MAX) + 1).expect("wide immediate");
    assert_eq!(wide.bytes().len(), 10);
    assert_eq!(&wide.bytes()[..2], &[0x49, 0xba]);
}

#[test]
fn runtime_and_multiple_value_limits_return_typed_errors() {
    let slots = empty_slots();
    let mut assembler = Assembler::new();
    assert!(matches!(
        lower_runtime_builtin(
            &mut assembler,
            RuntimeFunction::MakeValueCell,
            &[1, 2, 3, 4, 5],
            &[],
            &slots,
            &MissingAbi,
        ),
        Err(CodegenError::Unsupported(message))
            if message.contains("at most four arguments")
    ));

    let values = (0..=ncl_sys::MULTIPLE_VALUE_AREA_WORDS)
        .map(|value| ValueId(u32::try_from(value).expect("value id")))
        .collect::<Vec<_>>();
    assert!(matches!(
        store_return_values(&mut assembler, &slots, &values, &MissingAbi),
        Err(CodegenError::MultipleValueAreaOverflow { count, capacity })
            if count == ncl_sys::MULTIPLE_VALUE_AREA_WORDS + 1
                && capacity == ncl_sys::MULTIPLE_VALUE_AREA_WORDS
    ));
}

#[test]
fn lowering_helpers_preserve_typed_abi_and_slot_failures() {
    assert!(matches!(
        runtime_address(&MissingAbi, RuntimeFunction::SafepointSlow),
        Err(CodegenError::Unsupported(message))
            if message.contains("runtime address is unavailable")
    ));
    assert!(matches!(
        super::context_mem(&MissingAbi, ContextField::Pending),
        Err(CodegenError::Unsupported(message))
            if message.contains("context offset is unavailable")
    ));

    let slots = empty_slots();
    let mut assembler = Assembler::new();
    assert_eq!(
        load_slot(&mut assembler, &slots, ValueId(99), Reg::R10),
        Err(CodegenError::UnknownValue(ValueId(99)))
    );
    assert_eq!(slot_mem_of(u32::MAX), Err(CodegenError::FrameOverflow));
}
