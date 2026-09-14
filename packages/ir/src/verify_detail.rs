#[allow(clippy::wildcard_imports)]
use super::*;
use crate::{Convert, Prim};

pub(super) fn check_op(
    op: &Op,
    block: &BasicBlock,
    position: usize,
    function: &Function,
    blocks: &HashMap<BlockId, &BasicBlock>,
    definitions: &HashMap<ValueId, Definition>,
    dominators: &HashMap<BlockId, HashSet<BlockId>>,
    errors: &mut Vec<VerifyError>,
) {
    let use_one = |value: ValueId, errors: &mut Vec<VerifyError>| {
        use_value(value, block.id, position, definitions, dominators, errors)
    };
    match &op.kind {
        OpKind::Const { result } => {
            if result.0 as usize >= function.constants.len() {
                errors.push(VerifyError::ConstantOutOfBounds(block.id));
            }
            if let Some(constant) = function.constants.get(result.0 as usize) {
                require_results(op, &[constant_type(constant)], block.id, errors);
            }
        }
        OpKind::Move { value } => require_results(
            op,
            &[use_one(*value, errors).unwrap_or(Ty::Unit)],
            block.id,
            errors,
        ),
        OpKind::Load { address } => {
            require_type(use_one(*address, errors), Ty::Address, block.id, errors);
            require_results(op, &[Ty::Word], block.id, errors);
        }
        OpKind::Store { address, value } => {
            require_type(use_one(*address, errors), Ty::Address, block.id, errors);
            require_type(use_one(*value, errors), Ty::Word, block.id, errors);
            require_results(op, &[], block.id, errors);
        }
        OpKind::LoadField { object, .. } => {
            require_type(use_one(*object, errors), Ty::Word, block.id, errors);
            require_results(op, &[Ty::Word], block.id, errors);
        }
        OpKind::StoreField { object, value, .. } => {
            require_type(use_one(*object, errors), Ty::Word, block.id, errors);
            require_type(use_one(*value, errors), Ty::Word, block.id, errors);
            require_results(op, &[], block.id, errors);
        }
        OpKind::Alloc { .. } => require_results(op, &[Ty::Address], block.id, errors),
        OpKind::LoadArg { index } => {
            if let Some(parameter) = function.params.get(*index as usize) {
                require_results(op, &[parameter.ty], block.id, errors);
            } else {
                errors.push(VerifyError::TypeMismatch(block.id));
            }
        }
        OpKind::Call { function, args }
        | OpKind::CallIndirect {
            callee: function,
            args,
        } => {
            require_type(use_one(*function, errors), Ty::Word, block.id, errors);
            require_word_args(args, block, position, definitions, dominators, errors);
            require_results(op, &[Ty::Word], block.id, errors);
        }
        OpKind::Builtin { args, .. } => {
            require_word_args(args, block, position, definitions, dominators, errors);
            require_results(op, &[Ty::Word], block.id, errors);
        }
        OpKind::Prim {
            op: primitive,
            args,
            condition,
        } => {
            check_prim(
                primitive,
                args,
                op,
                block,
                position,
                definitions,
                dominators,
                errors,
            );
            if let Some(target) = condition {
                if !blocks.contains_key(target) {
                    errors.push(VerifyError::MissingBlock(*target));
                }
            }
        }
        OpKind::Compare { left, right, .. } => {
            let left_ty = use_one(*left, errors);
            let right_ty = use_one(*right, errors);
            if left_ty.is_none() || left_ty != right_ty {
                errors.push(VerifyError::TypeMismatch(block.id));
            }
            require_results(op, &[Ty::Bool], block.id, errors);
        }
        OpKind::Convert {
            op: conversion,
            value,
        } => {
            let input = use_one(*value, errors);
            let (expected_input, expected_output) = match conversion {
                Convert::WordToI64 => (Ty::Word, Ty::I64),
                Convert::I64ToWord => (Ty::I64, Ty::Word),
                Convert::WordToF64 => (Ty::Word, Ty::F64),
                Convert::F64ToWord => (Ty::F64, Ty::Word),
                Convert::AddressToWord => (Ty::Address, Ty::Word),
                Convert::WordToAddress => (Ty::Word, Ty::Address),
            };
            require_type(input, expected_input, block.id, errors);
            require_results(op, &[expected_output], block.id, errors);
        }
        OpKind::SetMultipleValues { values } => {
            for value in values {
                use_one(*value, errors);
            }
            require_results(op, &[], block.id, errors);
        }
        OpKind::Safepoint => require_results(op, &[], block.id, errors),
    }
}

fn check_prim(
    primitive: &Prim,
    args: &[ValueId],
    op: &Op,
    block: &BasicBlock,
    position: usize,
    definitions: &HashMap<ValueId, Definition>,
    dominators: &HashMap<BlockId, HashSet<BlockId>>,
    errors: &mut Vec<VerifyError>,
) {
    let actual = args
        .iter()
        .map(|value| use_value(*value, block.id, position, definitions, dominators, errors))
        .collect::<Vec<_>>();
    let (expected, result) = match primitive {
        Prim::Car | Prim::Cdr | Prim::StructureSlot(_) => (vec![Ty::Word], Ty::Word),
        Prim::Rplaca | Prim::Rplacd => (vec![Ty::Word, Ty::Word], Ty::Unit),
        Prim::Svref | Prim::Aref => (vec![Ty::Word, Ty::Word], Ty::Word),
        Prim::Aset => (vec![Ty::Word, Ty::Word, Ty::Word], Ty::Unit),
        Prim::FixnumAdd | Prim::FixnumSub | Prim::FixnumMul | Prim::FixnumDiv => {
            (vec![Ty::I64, Ty::I64], Ty::I64)
        }
        Prim::FixnumLt | Prim::FixnumLe | Prim::FixnumEq => (vec![Ty::I64, Ty::I64], Ty::Bool),
        Prim::Eq | Prim::Eql | Prim::Typep => (vec![Ty::Word, Ty::Word], Ty::Bool),
        Prim::CharacterPredicate(_) => (vec![Ty::Word], Ty::Bool),
    };
    if actual.len() != expected.len()
        || actual
            .iter()
            .zip(expected)
            .any(|(actual, expected)| *actual != Some(expected))
    {
        errors.push(VerifyError::TypeMismatch(block.id));
    }
    require_results(op, &[result], block.id, errors);
}

fn successor(
    target: BlockId,
    args: &[ValueId],
    block: &BasicBlock,
    blocks: &HashMap<BlockId, &BasicBlock>,
    definitions: &HashMap<ValueId, Definition>,
    dominators: &HashMap<BlockId, HashSet<BlockId>>,
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
        let actual = use_value(
            *value,
            block.id,
            block.ops.len() + block.params.len() + 1,
            definitions,
            dominators,
            errors,
        );
        if let Some(parameter) = destination.params.get(index) {
            if actual != Some(parameter.ty) {
                errors.push(VerifyError::SuccessorType(block.id));
            }
        }
    }
}

pub(super) fn check_terminator(
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

fn constant_type(constant: &crate::Constant) -> Ty {
    match constant {
        crate::Constant::Fixnum(_) => Ty::I64,
        crate::Constant::SingleFloat(_) | crate::Constant::DoubleFloat(_) => Ty::F64,
        crate::Constant::Character(_)
        | crate::Constant::Symbol { .. }
        | crate::Constant::Object(_)
        | crate::Constant::StringBytes(_)
        | crate::Constant::Nil
        | crate::Constant::T
        | crate::Constant::Unbound => Ty::Word,
    }
}
