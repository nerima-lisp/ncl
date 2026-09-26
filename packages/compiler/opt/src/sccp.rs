//! Sparse conditional constant propagation for one SSA function.

use super::{FunctionPass, Module, PassError, PassResult};
use ncl_ir::{
    BlockId, Compare, Constant, ConstantIndex, Convert, Function, Op, OpKind, Prim, Terminator, Ty,
    ValueId,
};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    Unknown,
    Constant(ConstantIndex),
    Overdefined,
}

impl State {
    fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Unknown, value) | (value, Self::Unknown) => value,
            (Self::Constant(left), Self::Constant(right)) if left == right => Self::Constant(left),
            _ => Self::Overdefined,
        }
    }
}

/// Sparse conditional constant propagation.
#[derive(Clone, Debug, Default)]
pub struct Sccp;

impl Sccp {
    fn update(states: &mut HashMap<ValueId, State>, value: ValueId, next: State) -> bool {
        let old = states.get(&value).copied().unwrap_or(State::Unknown);
        let merged = old.merge(next);
        if merged == old {
            false
        } else {
            states.insert(value, merged);
            true
        }
    }

    fn constant(function: &mut Function, value: Constant) -> Option<ConstantIndex> {
        if let Some((index, _)) = function
            .constants
            .iter()
            .enumerate()
            .find(|(_, existing)| **existing == value)
        {
            return u32::try_from(index).ok().map(ConstantIndex);
        }
        let index = ConstantIndex(u32::try_from(function.constants.len()).ok()?);
        function.constants.push(value);
        Some(index)
    }

    fn constant_value(function: &Function, state: State) -> Option<&Constant> {
        let State::Constant(index) = state else {
            return None;
        };
        usize::try_from(index.0)
            .ok()
            .and_then(|position| function.constants.get(position))
    }

    fn bool_state(function: &mut Function, value: bool) -> State {
        let constant = if value { Constant::T } else { Constant::Nil };
        Self::constant(function, constant).map_or(State::Overdefined, State::Constant)
    }

    fn binary_fixnum(
        function: &mut Function,
        states: &HashMap<ValueId, State>,
        left: ValueId,
        right: ValueId,
        operation: impl FnOnce(i64, i64) -> Option<i64>,
    ) -> State {
        let (Some(Constant::Fixnum(left)), Some(Constant::Fixnum(right))) = (
            Self::constant_value(
                function,
                states.get(&left).copied().unwrap_or(State::Unknown),
            ),
            Self::constant_value(
                function,
                states.get(&right).copied().unwrap_or(State::Unknown),
            ),
        ) else {
            return Self::binary_unknown(states, left, right);
        };
        operation(*left, *right).map_or(State::Overdefined, |value| {
            Self::constant(function, Constant::Fixnum(value))
                .map_or(State::Overdefined, State::Constant)
        })
    }

    fn binary_unknown(states: &HashMap<ValueId, State>, left: ValueId, right: ValueId) -> State {
        match (
            states.get(&left).copied().unwrap_or(State::Unknown),
            states.get(&right).copied().unwrap_or(State::Unknown),
        ) {
            (State::Overdefined, _) | (_, State::Overdefined) => State::Overdefined,
            _ => State::Unknown,
        }
    }

    #[allow(clippy::too_many_lines)]
    fn eval_op(function: &mut Function, states: &HashMap<ValueId, State>, op: &Op) -> State {
        match &op.kind {
            OpKind::Const { result }
                if usize::try_from(result.0)
                    .ok()
                    .and_then(|index| function.constants.get(index))
                    .is_some() =>
            {
                State::Constant(*result)
            }
            OpKind::Move { value }
            | OpKind::Convert {
                op: Convert::I64ToWord | Convert::WordToI64,
                value,
            } => states.get(value).copied().unwrap_or(State::Unknown),
            OpKind::Prim { op, args, .. } => match (op, args.as_slice()) {
                (Prim::FixnumAdd, [left, right]) => {
                    Self::binary_fixnum(function, states, *left, *right, i64::checked_add)
                }
                (Prim::FixnumSub, [left, right]) => {
                    Self::binary_fixnum(function, states, *left, *right, i64::checked_sub)
                }
                (Prim::FixnumMul, [left, right]) => {
                    Self::binary_fixnum(function, states, *left, *right, i64::checked_mul)
                }
                (Prim::FixnumDiv, [left, right]) => {
                    Self::binary_fixnum(function, states, *left, *right, |left, right| {
                        left.checked_div(right)
                    })
                }
                (Prim::FixnumLt | Prim::FixnumLe | Prim::FixnumEq, [left, right]) => {
                    let (Some(Constant::Fixnum(left)), Some(Constant::Fixnum(right))) = (
                        Self::constant_value(
                            function,
                            states.get(left).copied().unwrap_or(State::Unknown),
                        ),
                        Self::constant_value(
                            function,
                            states.get(right).copied().unwrap_or(State::Unknown),
                        ),
                    ) else {
                        return Self::binary_unknown(states, *left, *right);
                    };
                    let result = match op {
                        Prim::FixnumLt => left < right,
                        Prim::FixnumLe => left <= right,
                        Prim::FixnumEq => left == right,
                        Prim::Car
                        | Prim::Cdr
                        | Prim::Rplaca
                        | Prim::Rplacd
                        | Prim::Svref
                        | Prim::Aref
                        | Prim::Aset
                        | Prim::FixnumAdd
                        | Prim::FixnumSub
                        | Prim::FixnumMul
                        | Prim::FixnumDiv
                        | Prim::Eq
                        | Prim::Eql
                        | Prim::Typep
                        | Prim::CharacterPredicate(_)
                        | Prim::StructureSlot(_) => false,
                    };
                    Self::bool_state(function, result)
                }
                (Prim::Eq | Prim::Eql, [left, right]) if left == right => {
                    Self::bool_state(function, true)
                }
                (Prim::Eq | Prim::Eql, [left, right]) => {
                    Self::binary_unknown(states, *left, *right)
                }
                _ => State::Overdefined,
            },
            OpKind::Compare { op, left, right } if left == right => Self::bool_state(
                function,
                matches!(op, Compare::Eq | Compare::Le | Compare::Ge),
            ),
            OpKind::Compare { op, left, right } => {
                let (Some(Constant::Fixnum(left)), Some(Constant::Fixnum(right))) = (
                    Self::constant_value(
                        function,
                        states.get(left).copied().unwrap_or(State::Unknown),
                    ),
                    Self::constant_value(
                        function,
                        states.get(right).copied().unwrap_or(State::Unknown),
                    ),
                ) else {
                    return Self::binary_unknown(states, *left, *right);
                };
                let result = match op {
                    Compare::Eq => left == right,
                    Compare::Ne => left != right,
                    Compare::Lt => left < right,
                    Compare::Le => left <= right,
                    Compare::Gt => left > right,
                    Compare::Ge => left >= right,
                };
                Self::bool_state(function, result)
            }
            OpKind::Load { .. }
            | OpKind::LoadField { .. }
            | OpKind::Alloc { .. }
            | OpKind::LoadArg { .. }
            | OpKind::Call { .. }
            | OpKind::CallIndirect { .. }
            | OpKind::MakeClosure { .. }
            | OpKind::CallClosure { .. }
            | OpKind::Builtin { .. }
            | OpKind::Convert { .. } => State::Overdefined,
            _ => State::Unknown,
        }
    }

    fn successors(
        function: &Function,
        states: &HashMap<ValueId, State>,
        block: &ncl_ir::BasicBlock,
    ) -> Vec<(BlockId, Vec<ValueId>)> {
        match &block.terminator {
            Terminator::Jump { target, args } => vec![(*target, args.clone())],
            Terminator::Branch {
                condition,
                then_target,
                then_args,
                else_target,
                else_args,
            } => match states.get(condition).copied().unwrap_or(State::Unknown) {
                State::Constant(index)
                    if matches!(
                        Self::constant_value(function, State::Constant(index)),
                        Some(Constant::T)
                    ) =>
                {
                    vec![(*then_target, then_args.clone())]
                }
                State::Constant(index)
                    if matches!(
                        Self::constant_value(function, State::Constant(index)),
                        Some(Constant::Nil)
                    ) =>
                {
                    vec![(*else_target, else_args.clone())]
                }
                _ => vec![
                    (*then_target, then_args.clone()),
                    (*else_target, else_args.clone()),
                ],
            },
            Terminator::Switch {
                value,
                cases,
                default,
                default_args,
            } => {
                let State::Constant(index) = states.get(value).copied().unwrap_or(State::Unknown)
                else {
                    return cases
                        .iter()
                        .map(|(_, target, args)| (*target, args.clone()))
                        .chain(std::iter::once((*default, default_args.clone())))
                        .collect();
                };
                let Some(Constant::Fixnum(value)) =
                    Self::constant_value(function, State::Constant(index))
                else {
                    return vec![(*default, default_args.clone())];
                };
                cases.iter().find(|(case, _, _)| case == value).map_or_else(
                    || vec![(*default, default_args.clone())],
                    |(_, target, args)| vec![(*target, args.clone())],
                )
            }
            _ => Vec::new(),
        }
    }

    fn rewrite(
        function: &mut Function,
        reachable: &HashSet<BlockId>,
        states: &HashMap<ValueId, State>,
    ) -> bool {
        let mut changed = false;
        let constants = function.constants.clone();
        for block in &mut function.blocks {
            if !reachable.contains(&block.id) {
                if !matches!(block.terminator, Terminator::Unreachable) || !block.ops.is_empty() {
                    block.ops.clear();
                    block.terminator = Terminator::Unreachable;
                    changed = true;
                }
                continue;
            }
            for op in &mut block.ops {
                let Some((value, ty)) = op.results.first().copied() else {
                    continue;
                };
                let State::Constant(index) = states.get(&value).copied().unwrap_or(State::Unknown)
                else {
                    continue;
                };
                let Some(constant) = Self::constant_value_from(&constants, State::Constant(index))
                else {
                    continue;
                };
                let valid_type = matches!(
                    (ty, constant),
                    (Ty::I64, Constant::Fixnum(_))
                        | (Ty::F64, Constant::SingleFloat(_) | Constant::DoubleFloat(_))
                        | (Ty::Word, _)
                );
                if valid_type && !matches!(op.kind, OpKind::Const { .. }) {
                    op.kind = OpKind::Const { result: index };
                    changed = true;
                }
            }
            let folded = match block.terminator.clone() {
                Terminator::Branch {
                    condition,
                    then_target,
                    then_args,
                    else_target,
                    else_args,
                } => match Self::constant_value_from(
                    &constants,
                    states.get(&condition).copied().unwrap_or(State::Unknown),
                ) {
                    Some(Constant::T) => Some(Terminator::Jump {
                        target: then_target,
                        args: then_args,
                    }),
                    Some(Constant::Nil) => Some(Terminator::Jump {
                        target: else_target,
                        args: else_args,
                    }),
                    _ => None,
                },
                Terminator::Switch {
                    value,
                    cases,
                    default,
                    default_args,
                } => match Self::constant_value_from(
                    &constants,
                    states.get(&value).copied().unwrap_or(State::Unknown),
                ) {
                    Some(Constant::Fixnum(value)) => {
                        cases.into_iter().find(|(case, _, _)| case == value).map_or(
                            Some(Terminator::Jump {
                                target: default,
                                args: default_args,
                            }),
                            |(_, target, args)| Some(Terminator::Jump { target, args }),
                        )
                    }
                    _ => None,
                },
                _ => None,
            };
            if let Some(terminator) = folded {
                block.terminator = terminator;
                changed = true;
            }
        }
        changed
    }

    fn constant_value_from(constants: &[Constant], state: State) -> Option<&Constant> {
        let State::Constant(index) = state else {
            return None;
        };
        usize::try_from(index.0)
            .ok()
            .and_then(|position| constants.get(position))
    }
}

impl FunctionPass for Sccp {
    fn name(&self) -> &'static str {
        "sccp"
    }

    fn run(&mut self, function: &mut Function, _module: &Module) -> PassResult {
        ncl_ir::verify(function)
            .map_err(|errors| PassError::new(self.name(), format!("{errors:?}")))?;
        let Some(entry) = function.blocks.first().map(|block| block.id) else {
            return Ok(false);
        };
        let mut states = HashMap::new();
        for index in 0..function.params.len() {
            let Some(value) = u32::try_from(index).ok().map(ValueId) else {
                return Err(PassError::new(
                    self.name(),
                    "parameter index exceeds SSA value range",
                ));
            };
            states.insert(value, State::Overdefined);
        }
        let mut reachable = HashSet::from([entry]);
        let mut incoming = HashMap::<BlockId, Vec<Vec<State>>>::new();
        let mut work = VecDeque::from([entry]);
        while let Some(block_id) = work.pop_front() {
            let Some(block_index) = function
                .blocks
                .iter()
                .position(|block| block.id == block_id)
            else {
                continue;
            };
            let block = function.blocks[block_index].clone();
            for (index, parameter) in block.params.iter().enumerate() {
                let merged = incoming
                    .get(&block_id)
                    .into_iter()
                    .flatten()
                    .filter_map(|edge| edge.get(index).copied())
                    .fold(State::Unknown, State::merge);
                Self::update(&mut states, parameter.value, merged);
            }
            for op in &block.ops {
                if let Some((value, _)) = op.results.first() {
                    let next = Self::eval_op(function, &states, op);
                    Self::update(&mut states, *value, next);
                }
            }
            for (target, args) in Self::successors(function, &states, &block) {
                let values = args
                    .iter()
                    .map(|value| states.get(value).copied().unwrap_or(State::Unknown))
                    .collect::<Vec<_>>();
                let edges = incoming.entry(target).or_default();
                let before = edges.clone();
                if !edges.iter().any(|edge| edge == &values) {
                    edges.push(values);
                }
                if before != *edges || reachable.insert(target) {
                    work.push_back(target);
                }
            }
        }
        let changed = Self::rewrite(function, &reachable, &states);
        ncl_ir::verify(function)
            .map_err(|errors| PassError::new(self.name(), format!("{errors:?}")))?;
        Ok(changed)
    }
}
