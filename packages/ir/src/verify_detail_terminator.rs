use super::super::{Definition, VerifyError, require_word_args};
use super::{require_type, successor, use_value};
use crate::{BasicBlock, BlockId, Function, Terminator, Ty, ValueId};
use std::collections::{HashMap, HashSet};

pub(in crate::verify) fn check_terminator(
    term: &Terminator,
    block: &BasicBlock,
    function: &Function,
    blocks: &HashMap<BlockId, &BasicBlock>,
    definitions: &HashMap<ValueId, Definition>,
    dominators: &HashMap<BlockId, HashSet<BlockId>>,
    errors: &mut Vec<VerifyError>,
) {
    let position = block.ops.len() + block.params.len() + 1;
    match term {
        Terminator::Jump { target, args } => successor(
            *target,
            args,
            block,
            blocks,
            definitions,
            dominators,
            errors,
        ),
        Terminator::Branch {
            condition,
            then_target,
            then_args,
            else_target,
            else_args,
        } => {
            require_type(
                use_value(
                    *condition,
                    block.id,
                    position,
                    definitions,
                    dominators,
                    errors,
                ),
                Ty::Bool,
                block.id,
                errors,
            );
            successor(
                *then_target,
                then_args,
                block,
                blocks,
                definitions,
                dominators,
                errors,
            );
            successor(
                *else_target,
                else_args,
                block,
                blocks,
                definitions,
                dominators,
                errors,
            );
        }
        Terminator::Switch {
            value,
            cases,
            default,
            default_args,
        } => {
            require_type(
                use_value(*value, block.id, position, definitions, dominators, errors),
                Ty::I64,
                block.id,
                errors,
            );
            for (_, target, args) in cases {
                successor(
                    *target,
                    args,
                    block,
                    blocks,
                    definitions,
                    dominators,
                    errors,
                );
            }
            successor(
                *default,
                default_args,
                block,
                blocks,
                definitions,
                dominators,
                errors,
            );
        }
        Terminator::CallReturn {
            function: callee,
            args,
        }
        | Terminator::TailCall {
            function: callee,
            args,
        } => {
            require_type(
                use_value(*callee, block.id, position, definitions, dominators, errors),
                Ty::Word,
                block.id,
                errors,
            );
            require_word_args(args, block, position, definitions, dominators, errors);
        }
        Terminator::Return { values } => {
            if values.len() != function.return_types.len() {
                errors.push(VerifyError::ReturnArity(block.id));
            }
            for (index, value) in values.iter().enumerate() {
                let actual = use_value(*value, block.id, position, definitions, dominators, errors);
                if let Some(expected) = function.return_types.get(index) {
                    require_type(actual, *expected, block.id, errors);
                }
            }
        }
        Terminator::Throw { condition } => require_type(
            use_value(
                *condition,
                block.id,
                position,
                definitions,
                dominators,
                errors,
            ),
            Ty::Word,
            block.id,
            errors,
        ),
        Terminator::Unreachable => {}
    }
}
