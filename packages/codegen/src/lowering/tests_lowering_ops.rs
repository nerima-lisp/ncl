use super::*;
use crate::{ContextField, RuntimeFunction};

struct Abi;
impl RuntimeAbi for Abi {
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
    fn constant_word(&self, name: &str) -> Option<i64> {
        (name == "function-entry:9").then_some(0x9000)
    }
}

#[test]
fn constant_values_cover_machine_constants_and_runtime_entries() {
    let abi = Abi;
    let constants = [
        Constant::Fixnum(-3),
        Constant::Character('A' as u32),
        Constant::SingleFloat(1.5),
        Constant::DoubleFloat(2.5),
        Constant::Nil,
        Constant::Unbound,
        Constant::T,
    ];
    for constant in constants {
        assert!(constant_value(&constant, &abi).is_ok(), "{constant:?}");
    }
    assert_eq!(
        constant_value(&Constant::FunctionEntry(ncl_ir::FunctionId(9)), &abi),
        Ok(0x9000)
    );
    assert!(matches!(
        constant_value(&Constant::FunctionEntry(ncl_ir::FunctionId(8)), &abi),
        Err(CodegenError::Unsupported(message)) if message == "entry unavailable"
    ));
    assert!(matches!(
        constant_value(&Constant::StringBytes(vec![1, 2]), &abi),
        Err(CodegenError::Unsupported(message)) if message.contains("runtime constant table")
    ));
}

#[test]
fn fixed_template_primitives_emit_or_reject_by_contract() {
    let slots = [(ValueId(0), 0), (ValueId(1), 1), (ValueId(2), 2)];
    for primitive in [
        Prim::FixnumAdd,
        Prim::FixnumSub,
        Prim::FixnumMul,
        Prim::FixnumEq,
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
        let mut assembler = Assembler::new();
        assert!(
            lower_prim(
                &mut assembler,
                &primitive,
                &[ValueId(0), ValueId(1)],
                Some(ValueId(2)),
                &slots,
            )
            .is_ok(),
            "{primitive:?}"
        );
        assert!(!assembler.bytes().is_empty(), "{primitive:?}");
    }
    for primitive in [
        Prim::FixnumDiv,
        Prim::Typep,
        Prim::CharacterPredicate("characterp".into()),
        Prim::StructureSlot("car".into()),
    ] {
        let mut assembler = Assembler::new();
        assert!(matches!(
            lower_prim(&mut assembler, &primitive, &[ValueId(0)], None, &slots),
            Err(CodegenError::Unsupported(_))
        ));
    }
    let mut assembler = Assembler::new();
    assert!(matches!(
        lower_prim(&mut assembler, &Prim::Car, &[], None, &slots),
        Err(CodegenError::Unsupported(message)) if message == "primitive has no operands"
    ));
}

struct MatrixAbi;

impl RuntimeAbi for MatrixAbi {
    fn builtin_address(
        &self,
        identifier: ncl_object::BuiltinIdentifier,
    ) -> Result<u64, crate::AbiError> {
        (identifier.name.as_str() == "identity")
            .then_some(0x1234)
            .ok_or(crate::AbiError::MissingBuiltin(identifier))
    }

    fn field_offset(&self, field: ContextField) -> Result<i32, crate::AbiError> {
        Err(crate::AbiError::UnsupportedContextField(field))
    }

    fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, crate::AbiError> {
        Err(crate::AbiError::UnsupportedRuntimeFunction(function))
    }

    fn constant_word(&self, name: &str) -> Option<i64> {
        (name == "function-entry:5").then_some(0x5000)
    }
}

fn test_function(constants: Vec<Constant>) -> Function {
    Function {
        id: ncl_ir::FunctionId(5),
        name: "operation-matrix".into(),
        params: Vec::new(),
        return_types: Vec::new(),
        blocks: Vec::new(),
        locals: Vec::new(),
        constants,
        handler_regions: Vec::new(),
        debug: Vec::new(),
    }
}

fn test_op(kind: OpKind, result: Option<ValueId>) -> Op {
    Op {
        results: result
            .map(|value| vec![(value, ncl_ir::Ty::Word)])
            .unwrap_or_default(),
        kind,
        loc: None,
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn lowers_the_generic_operation_matrix_and_records_safepoints() {
    let function = test_function(vec![Constant::Fixnum(1)]);
    let slots: Vec<_> = (0..16).map(|index| (ValueId(index), index + 4)).collect();
    let frame = FrameLayout::new(0, 16, 0).expect("matrix frame");
    let mut assembler = Assembler::new();
    let mut maps = Vec::new();
    let mut operations = vec![
        test_op(
            OpKind::Const {
                result: ncl_ir::ConstantIndex(0),
            },
            Some(ValueId(2)),
        ),
        test_op(OpKind::Move { value: ValueId(0) }, Some(ValueId(2))),
        test_op(
            OpKind::Load {
                address: ValueId(0),
            },
            Some(ValueId(2)),
        ),
        test_op(
            OpKind::Store {
                address: ValueId(0),
                value: ValueId(1),
            },
            None,
        ),
        test_op(
            OpKind::LoadField {
                object: ValueId(0),
                field: 2,
            },
            Some(ValueId(2)),
        ),
        test_op(
            OpKind::StoreField {
                object: ValueId(0),
                field: 3,
                value: ValueId(1),
            },
            None,
        ),
        test_op(OpKind::Alloc { words: 2 }, Some(ValueId(2))),
        test_op(OpKind::LoadArg { index: 0 }, Some(ValueId(2))),
        test_op(OpKind::LoadArg { index: 4 }, Some(ValueId(2))),
        test_op(
            OpKind::Call {
                function: ValueId(0),
                args: vec![ValueId(1)],
            },
            Some(ValueId(2)),
        ),
        test_op(
            OpKind::CallIndirect {
                callee: ValueId(0),
                args: vec![ValueId(1)],
            },
            Some(ValueId(2)),
        ),
        test_op(
            OpKind::Builtin {
                name: "identity".into(),
                args: vec![ValueId(0)],
            },
            Some(ValueId(2)),
        ),
        test_op(
            OpKind::SetMultipleValues {
                values: vec![ValueId(0), ValueId(1)],
            },
            Some(ValueId(2)),
        ),
        test_op(OpKind::SetMultipleValues { values: Vec::new() }, None),
        test_op(OpKind::Safepoint, None),
    ];
    for primitive in [
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
        operations.push(test_op(
            OpKind::Prim {
                op: primitive,
                args: vec![ValueId(0), ValueId(1)],
                condition: None,
            },
            Some(ValueId(2)),
        ));
    }
    for comparison in [
        Compare::Eq,
        Compare::Ne,
        Compare::Lt,
        Compare::Le,
        Compare::Gt,
        Compare::Ge,
    ] {
        operations.push(test_op(
            OpKind::Compare {
                op: comparison,
                left: ValueId(0),
                right: ValueId(1),
            },
            Some(ValueId(2)),
        ));
    }
    operations.push(test_op(
        OpKind::Convert {
            op: ncl_ir::Convert::WordToI64,
            value: ValueId(0),
        },
        Some(ValueId(2)),
    ));

    for operation in operations {
        let before = assembler.bytes().len();
        lower_op(
            &mut assembler,
            &operation,
            &function,
            &slots,
            &MatrixAbi,
            frame,
            &mut maps,
        )
        .unwrap_or_else(|error| panic!("{operation:?}: {error:?}"));
        assert!(
            assembler.bytes().len() > before || matches!(operation.kind, OpKind::Safepoint),
            "{operation:?}"
        );
    }
    assert_eq!(maps.len(), 5);
    assert_eq!(
        maps.iter()
            .filter(|map| map.map_flags & FLAG_CALL != 0)
            .count(),
        4
    );
    assert_eq!(
        maps.iter()
            .filter(|map| map.map_flags & FLAG_ALLOCATION_SLOW != 0)
            .count(),
        1
    );
}

#[test]
fn reports_malformed_generic_operations_without_silently_emitting_code() {
    let function = test_function(Vec::new());
    let frame = FrameLayout::new(0, 1, 0).expect("small frame");
    let slots = [(ValueId(0), 4), (ValueId(1), 5)];
    let mut maps = Vec::new();

    let mut assembler = Assembler::new();
    assert!(matches!(
        lower_op(
            &mut assembler,
            &test_op(
                OpKind::Const {
                    result: ncl_ir::ConstantIndex(0)
                },
                Some(ValueId(0))
            ),
            &function,
            &slots,
            &MatrixAbi,
            frame,
            &mut maps,
        ),
        Err(CodegenError::Unsupported(message)) if message == "constant index out of range"
    ));
    assert!(matches!(
        lower_op(
            &mut assembler,
            &test_op(
                OpKind::Load {
                    address: ValueId(9)
                },
                None
            ),
            &function,
            &slots,
            &MatrixAbi,
            frame,
            &mut maps,
        ),
        Err(CodegenError::UnknownValue(ValueId(9)))
    ));
    assert!(matches!(
        lower_op(
            &mut assembler,
            &test_op(OpKind::LoadArg { index: 5 }, Some(ValueId(0))),
            &function,
            &slots,
            &MatrixAbi,
            frame,
            &mut maps,
        ),
        Err(CodegenError::Unsupported(message)) if message.contains("stack argument")
    ));
    assert_eq!(
        lower_op(
            &mut assembler,
            &test_op(
                OpKind::LoadField {
                    object: ValueId(0),
                    field: u32::MAX,
                },
                None,
            ),
            &function,
            &slots,
            &MatrixAbi,
            frame,
            &mut maps,
        ),
        Err(CodegenError::FrameOverflow)
    );
    assert!(matches!(
        lower_op(
            &mut assembler,
            &test_op(OpKind::LoadCapture { index: 0 }, Some(ValueId(0))),
            &function,
            &slots,
            &MatrixAbi,
            frame,
            &mut maps,
        ),
        Err(CodegenError::Unsupported(message)) if message == "target-specific operation"
    ));

    let oversized_slots = vec![(ValueId(0), u32::MAX)];
    assert_eq!(
        add_map(&assembler, frame, &oversized_slots, &mut maps, FLAG_CALL,),
        Err(CodegenError::FrameOverflow)
    );
    assert_eq!(
        emit_return(&mut assembler, &[ValueId(9)], &slots, &MatrixAbi),
        Err(CodegenError::UnknownValue(ValueId(9)))
    );
}

#[test]
fn checks_all_compare_conditions_and_memory_boundaries() {
    assert_eq!(compare_condition(Compare::Eq), Cond::E);
    assert_eq!(compare_condition(Compare::Ne), Cond::Ne);
    assert_eq!(compare_condition(Compare::Lt), Cond::L);
    assert_eq!(compare_condition(Compare::Le), Cond::Le);
    assert_eq!(compare_condition(Compare::Gt), Cond::G);
    assert_eq!(compare_condition(Compare::Ge), Cond::Ge);
    assert!(memory(0).is_ok());
    assert_eq!(memory(u32::MAX), Err(CodegenError::FrameOverflow));
    assert_eq!(
        memory((i32::MAX as u32 / 8) + 1),
        Err(CodegenError::FrameOverflow)
    );
    assert_eq!(
        slot(&[(ValueId(1), 4)], ValueId(2)),
        Err(CodegenError::UnknownValue(ValueId(2)))
    );
}
