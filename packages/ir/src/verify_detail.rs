#[allow(clippy::wildcard_imports)]
use super::*;
use crate::{ConstantIndex, Convert, FixnumOp, Prim};

/// Record an out-of-bounds error when a constant references a table index
/// beyond the current constant table.
fn check_constant_reference(
    index: ConstantIndex,
    function: &Function,
    block: BlockId,
    errors: &mut Vec<VerifyError>,
) {
    let Some(object_index) = usize::try_from(index.0).ok() else {
        errors.push(VerifyError::ConstantOutOfBounds(block));
        return;
    };
    if object_index >= function.constants.len() {
        errors.push(VerifyError::ConstantOutOfBounds(block));
    }
}

pub(super) fn check_op(
    op: &Op,
    block: &BasicBlock,
    position: usize,
    function: &Function,
    blocks: &HashMap<BlockId, &BasicBlock>,
    definitions: &HashMap<ValueId, Definition>,
    dominators: &HashMap<BlockId, HashSet<BlockId>>,
    proven_fixnums: &HashSet<ValueId>,
    errors: &mut Vec<VerifyError>,
) {
    let use_one = |value: ValueId, errors: &mut Vec<VerifyError>| {
        use_value(value, block.id, position, definitions, dominators, errors)
    };
    match &op.kind {
        OpKind::Const { result } => {
            let Some(result_index) = usize::try_from(result.0).ok() else {
                errors.push(VerifyError::ConstantOutOfBounds(block.id));
                return;
            };
            if let Some(constant) = function.constants.get(result_index) {
                match constant {
                    crate::Constant::Object(index) => {
                        check_constant_reference(*index, function, block.id, errors);
                    }
                    crate::Constant::LiteralObject { value, .. } => {
                        check_constant_reference(*value, function, block.id, errors);
                    }
                    crate::Constant::Structure { elements, .. } => {
                        for index in elements {
                            check_constant_reference(*index, function, block.id, errors);
                        }
                    }
                    crate::Constant::Ratio {
                        numerator,
                        denominator,
                    } => {
                        check_constant_reference(*numerator, function, block.id, errors);
                        check_constant_reference(*denominator, function, block.id, errors);
                    }
                    crate::Constant::Complex { real, imaginary } => {
                        check_constant_reference(*real, function, block.id, errors);
                        check_constant_reference(*imaginary, function, block.id, errors);
                    }
                    crate::Constant::Fixnum(_)
                    | crate::Constant::Character(_)
                    | crate::Constant::SingleFloat(_)
                    | crate::Constant::DoubleFloat(_)
                    | crate::Constant::Symbol { .. }
                    | crate::Constant::UninternedSymbol { .. }
                    | crate::Constant::StringBytes(_)
                    | crate::Constant::Bignum { .. }
                    | crate::Constant::Nil
                    | crate::Constant::T
                    | crate::Constant::Unbound
                    | crate::Constant::FunctionEntry(_) => {}
                }
                require_results(op, &[constant_type(constant)], block.id, errors);
            } else {
                errors.push(VerifyError::ConstantOutOfBounds(block.id));
            }
        }
        OpKind::Move { value } => require_results(
            op,
            &[use_one(*value, errors).unwrap_or(Ty::Unit)],
            block.id,
            errors,
        ),
        OpKind::TypeHint { value, .. } | OpKind::MakeValueCell { value } => {
            require_type(use_one(*value, errors), Ty::Word, block.id, errors);
            require_results(op, &[Ty::Word], block.id, errors);
        }
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
            if let Some(parameter) = usize::try_from(*index)
                .ok()
                .and_then(|parameter_index| function.params.get(parameter_index))
            {
                require_results(op, &[parameter.ty], block.id, errors);
            } else {
                errors.push(VerifyError::TypeMismatch(block.id));
            }
        }
        OpKind::LoadCapture { .. } | OpKind::LoadFunctionObject => {
            require_results(op, &[Ty::Word], block.id, errors);
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
        OpKind::MakeClosure { entry, captures } => {
            require_type(use_one(*entry, errors), Ty::Word, block.id, errors);
            require_word_args(captures, block, position, definitions, dominators, errors);
            require_results(op, &[Ty::Word], block.id, errors);
        }
        OpKind::CallClosure {
            closure,
            args,
            named_symbol,
        } => {
            require_type(use_one(*closure, errors), Ty::Word, block.id, errors);
            if let Some(symbol) = named_symbol {
                require_type(use_one(*symbol, errors), Ty::Word, block.id, errors);
            }
            require_word_args(args, block, position, definitions, dominators, errors);
            require_results(op, &[Ty::Word], block.id, errors);
        }
        OpKind::Builtin {
            name,
            args,
            specialization,
            ..
        } => {
            require_word_args(args, block, position, definitions, dominators, errors);
            require_results(op, &[Ty::Word], block.id, errors);
            if specialization.is_some_and(|specialization| {
                FixnumOp::from_builtin(name, args.len()) != Some(specialization.operation())
                    || args.iter().enumerate().any(|(index, value)| {
                        !specialization.needs_tag_check(index) && !proven_fixnums.contains(value)
                    })
            }) {
                errors.push(VerifyError::InvalidFixnumSpecialization(block.id));
            }
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
        OpKind::EnterHandler { .. } | OpKind::LeaveHandler { .. } => {
            require_results(op, &[], block.id, errors);
        }
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

#[path = "verify_detail_terminator.rs"]
mod terminator;
pub(super) use terminator::check_terminator;

const fn constant_type(constant: &crate::Constant) -> Ty {
    match constant {
        crate::Constant::Fixnum(_) => Ty::I64,
        crate::Constant::SingleFloat(_) | crate::Constant::DoubleFloat(_) => Ty::F64,
        crate::Constant::Character(_)
        | crate::Constant::Symbol { .. }
        | crate::Constant::UninternedSymbol { .. }
        | crate::Constant::Object(_)
        | crate::Constant::LiteralObject { .. }
        | crate::Constant::StringBytes(_)
        | crate::Constant::Structure { .. }
        | crate::Constant::Bignum { .. }
        | crate::Constant::Ratio { .. }
        | crate::Constant::Complex { .. }
        | crate::Constant::Nil
        | crate::Constant::T
        | crate::Constant::Unbound
        | crate::Constant::FunctionEntry(_) => Ty::Word,
    }
}

pub(super) fn successors(term: &Terminator) -> Vec<BlockId> {
    match term {
        Terminator::Jump { target, .. } => vec![*target],
        Terminator::Branch {
            then_target,
            else_target,
            ..
        } => vec![*then_target, *else_target],
        Terminator::Switch { cases, default, .. } => cases
            .iter()
            .map(|(_, target, _)| *target)
            .chain([*default])
            .collect(),
        Terminator::CallReturn { .. }
        | Terminator::TailCall { .. }
        | Terminator::Return { .. }
        | Terminator::Throw { .. }
        | Terminator::Unreachable => Vec::new(),
    }
}

pub(super) fn op_uses(op: &Op) -> Vec<ValueId> {
    match &op.kind {
        OpKind::Const { .. }
        | OpKind::Alloc { .. }
        | OpKind::LoadArg { .. }
        | OpKind::LoadCapture { .. }
        | OpKind::LoadFunctionObject
        | OpKind::Safepoint
        | OpKind::EnterHandler { .. }
        | OpKind::LeaveHandler { .. } => Vec::new(),
        OpKind::Move { value }
        | OpKind::TypeHint { value, .. }
        | OpKind::Load { address: value }
        | OpKind::MakeValueCell { value }
        | OpKind::Convert { value, .. } => vec![*value],
        OpKind::Store { address, value } => vec![*address, *value],
        OpKind::LoadField { object, .. } => vec![*object],
        OpKind::StoreField { object, value, .. } => vec![*object, *value],
        OpKind::Call { function, args }
        | OpKind::CallIndirect {
            callee: function,
            args,
        } => std::iter::once(*function)
            .chain(args.iter().copied())
            .collect(),
        OpKind::MakeClosure { entry, captures } => std::iter::once(*entry)
            .chain(captures.iter().copied())
            .collect(),
        OpKind::CallClosure {
            closure,
            args,
            named_symbol,
        } => std::iter::once(*closure)
            .chain(named_symbol.iter().copied())
            .chain(args.iter().copied())
            .collect(),
        OpKind::Builtin { args, .. }
        | OpKind::SetMultipleValues { values: args }
        | OpKind::Prim { args, .. } => args.clone(),
        OpKind::Compare { left, right, .. } => vec![*left, *right],
    }
}

pub(super) fn terminator_uses(term: &Terminator) -> Vec<ValueId> {
    match term {
        Terminator::Jump { .. } | Terminator::Unreachable => Vec::new(),
        Terminator::Branch { condition, .. } | Terminator::Throw { condition } => {
            vec![*condition]
        }
        Terminator::Switch { value, .. } => vec![*value],
        Terminator::CallReturn { function, args } | Terminator::TailCall { function, args } => {
            std::iter::once(*function)
                .chain(args.iter().copied())
                .collect()
        }
        Terminator::Return { values } => values.clone(),
    }
}

pub(super) fn successor_edges(term: &Terminator) -> Vec<(BlockId, Vec<ValueId>)> {
    match term {
        Terminator::Jump { target, args } => vec![(*target, args.clone())],
        Terminator::Branch {
            then_target,
            then_args,
            else_target,
            else_args,
            ..
        } => vec![
            (*then_target, then_args.clone()),
            (*else_target, else_args.clone()),
        ],
        Terminator::Switch {
            cases,
            default,
            default_args,
            ..
        } => cases
            .iter()
            .map(|(_, target, args)| (*target, args.clone()))
            .chain(std::iter::once((*default, default_args.clone())))
            .collect(),
        Terminator::CallReturn { .. }
        | Terminator::TailCall { .. }
        | Terminator::Return { .. }
        | Terminator::Throw { .. }
        | Terminator::Unreachable => Vec::new(),
    }
}

pub(super) fn op_successors(op: &Op) -> Vec<BlockId> {
    match &op.kind {
        OpKind::Prim {
            condition: Some(target),
            ..
        } => vec![*target],
        OpKind::Const { .. }
        | OpKind::Move { .. }
        | OpKind::TypeHint { .. }
        | OpKind::Load { .. }
        | OpKind::Store { .. }
        | OpKind::LoadField { .. }
        | OpKind::StoreField { .. }
        | OpKind::Alloc { .. }
        | OpKind::LoadArg { .. }
        | OpKind::LoadCapture { .. }
        | OpKind::LoadFunctionObject
        | OpKind::Call { .. }
        | OpKind::CallIndirect { .. }
        | OpKind::MakeClosure { .. }
        | OpKind::MakeValueCell { .. }
        | OpKind::CallClosure { .. }
        | OpKind::Builtin { .. }
        | OpKind::Prim {
            condition: None, ..
        }
        | OpKind::Compare { .. }
        | OpKind::Convert { .. }
        | OpKind::SetMultipleValues { .. }
        | OpKind::Safepoint
        | OpKind::EnterHandler { .. }
        | OpKind::LeaveHandler { .. } => Vec::new(),
    }
}
