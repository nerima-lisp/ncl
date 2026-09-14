//! Structural and type checks for IR functions.
#![allow(missing_docs)]
#![allow(clippy::all)]

use crate::{BasicBlock, BlockId, Function, Op, OpKind, Terminator, Ty, ValueId};
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// A precise reason why an IR function is invalid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerifyError {
    DuplicateBlock(BlockId),
    MissingBlock(BlockId),
    MissingTerminator(BlockId),
    SuccessorArity(BlockId),
    SuccessorType(BlockId),
    UndefinedValue(ValueId),
    DuplicateValue(ValueId),
    ConstantOutOfBounds(BlockId),
    ReturnArity(BlockId),
    TypeMismatch(BlockId),
    HandlerTarget(BlockId),
    SafepointWarning(BlockId),
}
impl Display for VerifyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl Error for VerifyError {}

/// Verifies block structure, SSA visibility, and the function return contract.
///
/// # Errors
///
/// Returns every structural, SSA, and type error found in `function`.
pub fn verify(function: &Function) -> Result<(), Vec<VerifyError>> {
    let mut errors = Vec::new();
    let mut blocks = HashMap::new();
    for block in &function.blocks {
        if blocks.insert(block.id, block).is_some() {
            errors.push(VerifyError::DuplicateBlock(block.id));
        }
    }
    let mut values = HashMap::new();
    for block in &function.blocks {
        for parameter in &block.params {
            if values.insert(parameter.value, parameter.ty).is_some() {
                errors.push(VerifyError::DuplicateValue(parameter.value));
            }
        }
        let mut defined = values.keys().copied().collect::<Vec<_>>();
        for op in &block.ops {
            check_op(op, block.id, function, &values, &mut errors);
            for (value, ty) in &op.results {
                if values.insert(*value, *ty).is_some() {
                    errors.push(VerifyError::DuplicateValue(*value));
                }
                defined.push(*value);
            }
        }
        check_terminator(
            &block.terminator,
            block,
            function,
            &blocks,
            &values,
            &mut errors,
        );
        if matches!(block.terminator, Terminator::Unreachable) {
            errors.push(VerifyError::MissingTerminator(block.id));
        }
        let _ = defined;
    }
    for region in &function.handler_regions {
        for block in region
            .protected
            .iter()
            .chain(std::iter::once(&region.handler))
            .chain(region.cleanup.iter())
        {
            if !blocks.contains_key(block) {
                errors.push(VerifyError::HandlerTarget(*block));
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn use_value(value: ValueId, values: &HashMap<ValueId, Ty>, errors: &mut Vec<VerifyError>) {
    if values.contains_key(&value) {
        return;
    }
    errors.push(VerifyError::UndefinedValue(value));
}
fn check_op(
    op: &Op,
    block: BlockId,
    function: &Function,
    values: &HashMap<ValueId, Ty>,
    errors: &mut Vec<VerifyError>,
) {
    let mut args = Vec::new();
    match &op.kind {
        OpKind::Const { result } => {
            if result.0 as usize >= function.constants.len() {
                errors.push(VerifyError::ConstantOutOfBounds(block));
            }
        }
        OpKind::Move { value }
        | OpKind::Load { address: value }
        | OpKind::Convert { value, .. } => args.push(*value),
        OpKind::Store { address, value }
        | OpKind::Compare {
            left: address,
            right: value,
            ..
        } => {
            args.extend([*address, *value]);
        }
        OpKind::LoadField { object, .. } => args.push(*object),
        OpKind::Alloc { .. } | OpKind::LoadArg { .. } | OpKind::Safepoint => {}
        OpKind::StoreField { object, value, .. } => args.extend([*object, *value]),
        OpKind::Call {
            function,
            args: operands,
        } => {
            args.push(*function);
            args.extend(operands);
        }
        OpKind::CallIndirect {
            callee,
            args: operands,
        } => {
            args.push(*callee);
            args.extend(operands);
        }
        OpKind::Builtin { args: operands, .. } | OpKind::SetMultipleValues { values: operands } => {
            args.extend(operands);
        }
        OpKind::Prim { args: operands, .. } => args.extend(operands),
    }
    for value in args {
        use_value(value, values, errors);
    }
    if let OpKind::Prim {
        condition: Some(target),
        ..
    } = op.kind
    {
        if !function
            .blocks
            .iter()
            .any(|candidate| candidate.id == target)
        {
            errors.push(VerifyError::MissingBlock(target));
        }
    }
}
fn successor(
    target: BlockId,
    args: &[ValueId],
    block: &BasicBlock,
    blocks: &HashMap<BlockId, &BasicBlock>,
    values: &HashMap<ValueId, Ty>,
    errors: &mut Vec<VerifyError>,
) {
    let Some(destination) = blocks.get(&target) else {
        errors.push(VerifyError::MissingBlock(target));
        return;
    };
    if destination.params.len() != args.len() {
        errors.push(VerifyError::SuccessorArity(block.id));
    }
    for (index, value) in args.iter().enumerate() {
        use_value(*value, values, errors);
        if let Some(parameter) = destination.params.get(index) {
            if values.get(value) != Some(&parameter.ty) {
                errors.push(VerifyError::SuccessorType(block.id));
            }
        }
    }
}
fn check_terminator(
    term: &Terminator,
    block: &BasicBlock,
    function: &Function,
    blocks: &HashMap<BlockId, &BasicBlock>,
    values: &HashMap<ValueId, Ty>,
    errors: &mut Vec<VerifyError>,
) {
    match term {
        Terminator::Jump { target, args } => {
            successor(*target, args, block, blocks, values, errors);
        }
        Terminator::Branch {
            condition,
            then_target,
            then_args,
            else_target,
            else_args,
        } => {
            use_value(*condition, values, errors);
            successor(*then_target, then_args, block, blocks, values, errors);
            successor(*else_target, else_args, block, blocks, values, errors);
        }
        Terminator::Switch {
            value,
            cases,
            default,
            default_args,
        } => {
            use_value(*value, values, errors);
            for (_, target, args) in cases {
                successor(*target, args, block, blocks, values, errors);
            }
            successor(*default, default_args, block, blocks, values, errors);
        }
        Terminator::CallReturn {
            function: callee,
            args,
        }
        | Terminator::TailCall {
            function: callee,
            args,
        } => {
            use_value(*callee, values, errors);
            for value in args {
                use_value(*value, values, errors);
            }
        }
        Terminator::Return { values: returned } => {
            for value in returned {
                use_value(*value, values, errors);
            }
            if returned.len() != function.return_types.len() {
                errors.push(VerifyError::ReturnArity(block.id));
            }
        }
        Terminator::Throw { condition } => use_value(*condition, values, errors),
        Terminator::Unreachable => {}
    }
}
