use crate::{Block, CodegenError, CompiledFunction, FrameLayout, MachineFunction, MachineOp};
use crate::{RuntimeAbi, SafepointMap};
use ncl_ir::{Function, Terminator, ValueId};
use std::collections::HashSet;

use super::encode::encode;

pub(super) fn slots(function: &Function) -> (Vec<(ValueId, u32)>, u32) {
    let mut result = Vec::new();
    let mut next = 4;
    for block in &function.blocks {
        for param in &block.params {
            result.push((param.value, next));
            next += 1;
        }
        for op in &block.ops {
            for (value, _) in &op.results {
                result.push((*value, next));
                next += 1;
            }
        }
    }
    (result, next - 4)
}

/// Lowers one validated IR function using fixed stack-slot templates.
///
/// # Errors
///
/// Returns an error when the function is empty, references an unknown IR
/// value or block, exceeds frame limits, or requires an unavailable runtime
/// operation.
pub fn compile_function(
    function: &Function,
    abi: &dyn RuntimeAbi,
) -> Result<CompiledFunction, CodegenError> {
    let Some(entry) = function.blocks.first() else {
        return Err(CodegenError::EmptyFunction);
    };
    let (value_slots, count) = slots(function);
    let arguments =
        u32::try_from(function.params.len()).map_err(|_| CodegenError::FrameOverflow)?;
    let frame = FrameLayout::new(arguments, count, 0)?;
    let known: HashSet<_> = function.blocks.iter().map(|block| block.id).collect();
    for block in &function.blocks {
        validate_targets(&block.terminator, &known)?;
    }
    let blocks = function
        .blocks
        .iter()
        .map(|block| Block {
            id: block.id,
            operations: block
                .ops
                .iter()
                .map(|op| {
                    op.results.first().map_or(MachineOp::Return, |(value, _)| {
                        let slot = value_slots
                            .iter()
                            .find(|(id, _)| id == value)
                            .map_or(0, |(_, slot)| *slot);
                        MachineOp::Move {
                            source: slot,
                            destination: slot,
                        }
                    })
                })
                .chain(
                    matches!(block.terminator, Terminator::Return { .. })
                        .then_some(MachineOp::Return),
                )
                .collect(),
            offset: 0,
        })
        .collect();
    encode(
        function,
        MachineFunction {
            entry: entry.id,
            blocks,
            frame,
            safepoints: Vec::<SafepointMap>::new(),
            relocations: Vec::new(),
            slots: value_slots,
        },
        abi,
    )
}

pub(super) fn validate_targets(
    terminator: &Terminator,
    known: &HashSet<ncl_ir::BlockId>,
) -> Result<(), CodegenError> {
    let check = |id| {
        known
            .contains(&id)
            .then_some(())
            .ok_or(CodegenError::UnknownBlock(id))
    };
    match terminator {
        Terminator::Jump { target, .. } => check(*target),
        Terminator::Branch {
            then_target,
            else_target,
            ..
        } => {
            check(*then_target)?;
            check(*else_target)
        }
        Terminator::Switch { cases, default, .. } => {
            check(*default)?;
            for (_, target, _) in cases {
                check(*target)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::{compile_function, slots, validate_targets};
    use crate::{CodegenError, X86_64Abi};
    use ncl_ir::{
        BasicBlock, BlockId, BlockParam, Function, FunctionId, Op, OpKind, Terminator, Ty, ValueId,
    };
    use std::collections::HashSet;

    fn function_with_blocks(blocks: Vec<BasicBlock>) -> Function {
        Function {
            id: FunctionId(1),
            name: "compile-test".into(),
            params: Vec::new(),
            return_types: Vec::new(),
            blocks,
            locals: Vec::new(),
            constants: Vec::new(),
            handler_regions: Vec::new(),
            debug: Vec::new(),
        }
    }

    #[test]
    fn assigns_slots_to_block_parameters_and_operation_results_in_order() {
        let function = function_with_blocks(vec![BasicBlock {
            id: BlockId(0),
            params: vec![BlockParam {
                value: ValueId(7),
                ty: Ty::Word,
            }],
            ops: vec![Op {
                results: vec![(ValueId(3), Ty::I64)],
                kind: OpKind::Safepoint,
                loc: None,
            }],
            terminator: Terminator::Unreachable,
        }]);

        assert_eq!(
            slots(&function),
            (vec![(ValueId(7), 4), (ValueId(3), 5)], 2)
        );
    }

    #[test]
    fn validates_all_successor_forms_and_reports_each_invalid_target() {
        let known = HashSet::from([BlockId(0), BlockId(1), BlockId(2)]);
        let valid = [
            Terminator::Jump {
                target: BlockId(1),
                args: Vec::new(),
            },
            Terminator::Branch {
                condition: ValueId(0),
                then_target: BlockId(1),
                then_args: Vec::new(),
                else_target: BlockId(2),
                else_args: Vec::new(),
            },
            Terminator::Switch {
                value: ValueId(0),
                cases: vec![(1, BlockId(1), Vec::new())],
                default: BlockId(2),
                default_args: Vec::new(),
            },
            Terminator::CallReturn {
                function: ValueId(0),
                args: Vec::new(),
            },
            Terminator::TailCall {
                function: ValueId(0),
                args: Vec::new(),
            },
            Terminator::Return { values: Vec::new() },
            Terminator::Throw {
                condition: ValueId(0),
            },
            Terminator::Unreachable,
        ];
        for terminator in valid {
            assert!(
                validate_targets(&terminator, &known).is_ok(),
                "{terminator:?}"
            );
        }

        assert_eq!(
            validate_targets(
                &Terminator::Jump {
                    target: BlockId(9),
                    args: Vec::new(),
                },
                &known
            ),
            Err(CodegenError::UnknownBlock(BlockId(9)))
        );
        assert_eq!(
            validate_targets(
                &Terminator::Branch {
                    condition: ValueId(0),
                    then_target: BlockId(9),
                    then_args: Vec::new(),
                    else_target: BlockId(2),
                    else_args: Vec::new(),
                },
                &known
            ),
            Err(CodegenError::UnknownBlock(BlockId(9)))
        );
        assert_eq!(
            validate_targets(
                &Terminator::Switch {
                    value: ValueId(0),
                    cases: vec![(1, BlockId(9), Vec::new())],
                    default: BlockId(2),
                    default_args: Vec::new(),
                },
                &known
            ),
            Err(CodegenError::UnknownBlock(BlockId(9)))
        );
    }

    #[test]
    fn compile_function_reports_empty_functions_unknown_values_and_compiles_unit_returns() {
        let empty = function_with_blocks(Vec::new());
        assert_eq!(
            compile_function(&empty, &X86_64Abi),
            Err(CodegenError::EmptyFunction)
        );

        let missing_value = function_with_blocks(vec![BasicBlock {
            id: BlockId(0),
            params: Vec::new(),
            ops: Vec::new(),
            terminator: Terminator::Return {
                values: vec![ValueId(9)],
            },
        }]);
        assert_eq!(
            compile_function(&missing_value, &X86_64Abi),
            Err(CodegenError::UnknownValue(ValueId(9)))
        );

        let unit = function_with_blocks(vec![BasicBlock {
            id: BlockId(0),
            params: Vec::new(),
            ops: Vec::new(),
            terminator: Terminator::Return { values: Vec::new() },
        }]);
        let compiled = compile_function(&unit, &X86_64Abi).expect("unit return");
        assert!(!compiled.code.is_empty());
        assert_eq!(compiled.safepoint_maps, Vec::new());
    }
}
