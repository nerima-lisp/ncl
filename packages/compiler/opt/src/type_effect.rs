use crate::{FunctionPass, Module, PassResult};
use ncl_ir::{
    BlockId, Constant, FixnumOp, FixnumSpecialization, Function, OpKind, Prim, ValueId,
    proven_fixnum_values,
};
use std::collections::HashMap;

/// Scalar categories tracked by the local analysis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarType {
    /// No useful scalar classification is known.
    Unknown,
    /// A signed integer that fits the tagged fixnum payload.
    Fixnum,
    /// An integer that may require a bignum representation.
    Integer,
    /// A non-integer numeric value or an unknown numeric subtype.
    Number,
    /// A compiler boolean result.
    Boolean,
    /// A value known not to be numeric.
    NonNumber,
}

/// Whether a scalar fact follows from IR or from a source declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Evidence {
    /// The fact follows from a constant, operation, or control-flow join.
    Proven,
    /// The fact comes from a source declaration.
    Declared,
}

/// A scalar type fact for one SSA value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarFact {
    /// No scalar type information is available.
    Unknown,
    /// A classified value and the basis for that classification.
    Known {
        /// The scalar category.
        ty: ScalarType,
        /// Whether the category is proven or declared.
        evidence: Evidence,
    },
}

/// Effects an operation may have.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    /// Reads memory visible to the current function.
    ReadMemory,
    /// Writes memory visible to the current function.
    WriteMemory,
    /// Calls runtime or user code.
    Call,
    /// May allocate a heap object.
    Allocate,
    /// May signal a condition or transfer control nonlocally.
    Signal,
}

/// Effects that conservative analysis cannot rule out for an operation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EffectSet(u8);

impl EffectSet {
    const READ_MEMORY: u8 = 1;
    const WRITE_MEMORY: u8 = 2;
    const CALL: u8 = 4;
    const ALLOCATE: u8 = 8;
    const SIGNAL: u8 = 16;

    /// The conservative effects of an unknown call.
    pub const UNKNOWN_CALL: Self =
        Self(Self::READ_MEMORY | Self::WRITE_MEMORY | Self::CALL | Self::ALLOCATE | Self::SIGNAL);

    /// Whether this effect remains possible for the analyzed operation.
    #[must_use]
    pub const fn contains(self, effect: Effect) -> bool {
        let bit = match effect {
            Effect::ReadMemory => Self::READ_MEMORY,
            Effect::WriteMemory => Self::WRITE_MEMORY,
            Effect::Call => Self::CALL,
            Effect::Allocate => Self::ALLOCATE,
            Effect::Signal => Self::SIGNAL,
        };
        self.0 & bit != 0
    }

    const fn with(self, effect: Effect) -> Self {
        let bit = match effect {
            Effect::ReadMemory => Self::READ_MEMORY,
            Effect::WriteMemory => Self::WRITE_MEMORY,
            Effect::Call => Self::CALL,
            Effect::Allocate => Self::ALLOCATE,
            Effect::Signal => Self::SIGNAL,
        };
        Self(self.0 | bit)
    }
}

/// A known or unknown number of returned values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValuesArity {
    /// The number of returned values depends on a call or unknown state.
    Unknown,
    /// The exact number of returned values.
    Exact(usize),
}

/// Facts computed for one function.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FunctionFacts {
    /// Non-unknown scalar facts keyed by SSA value; a missing entry is unknown.
    pub values: HashMap<ValueId, ScalarFact>,
    /// Nonempty possible effects keyed by block and operation index; a missing
    /// entry means the analysis found no remaining effects. Terminators use
    /// `ops.len()`.
    pub effects: HashMap<(BlockId, usize), EffectSet>,
    /// Known exact arities keyed by block and operation or terminator index; a
    /// missing entry means the arity is unknown or inapplicable.
    pub values_arity: HashMap<(BlockId, usize), ValuesArity>,
}

impl FunctionFacts {
    /// Compute conservative scalar, effect, and values-arity facts.
    #[must_use]
    pub fn analyze(function: &Function) -> Self {
        let proven_fixnums = proven_fixnum_values(function);
        let mut facts = Self::default();
        facts.values.extend(
            proven_fixnums
                .iter()
                .copied()
                .map(|value| (value, proven(ScalarType::Fixnum))),
        );
        for block in &function.blocks {
            for (index, op) in block.ops.iter().enumerate() {
                record_operation_facts(function, &mut facts, block.id, index, op, &proven_fixnums);
            }
            let terminator_index = block.ops.len();
            let terminator_effect = effect_for_terminator(&block.terminator);
            if terminator_effect != EffectSet::default() {
                facts
                    .effects
                    .insert((block.id, terminator_index), terminator_effect);
            }
            match &block.terminator {
                ncl_ir::Terminator::Return { .. }
                | ncl_ir::Terminator::CallReturn { .. }
                | ncl_ir::Terminator::TailCall { .. } => {
                    facts
                        .values_arity
                        .insert((block.id, terminator_index), ValuesArity::Unknown);
                }
                // check-added-lines: allow(wildcard) other terminators do not establish a returned-values arity.
                _ => {}
            }
        }
        facts
    }
}

fn record_operation_facts(
    function: &Function,
    facts: &mut FunctionFacts,
    block_id: BlockId,
    index: usize,
    op: &ncl_ir::Op,
    proven_fixnums: &std::collections::HashSet<ValueId>,
) {
    let effect = effect_for(&op.kind);
    if effect != EffectSet::default() {
        facts.effects.insert((block_id, index), effect);
    }
    record_scalar_facts(function, &mut facts.values, op, proven_fixnums);
    match &op.kind {
        OpKind::SetMultipleValues { values } => {
            facts
                .values_arity
                .insert((block_id, index), ValuesArity::Exact(values.len()));
        }
        OpKind::Call { .. }
        | OpKind::CallIndirect { .. }
        | OpKind::CallClosure { .. }
        | OpKind::Builtin { .. } => {
            facts
                .values_arity
                .insert((block_id, index), ValuesArity::Unknown);
        }
        // check-added-lines: allow(wildcard) only explicit values and calls establish arity facts.
        _ => {}
    }
}

fn record_scalar_facts(
    function: &Function,
    values: &mut HashMap<ValueId, ScalarFact>,
    op: &ncl_ir::Op,
    proven_fixnums: &std::collections::HashSet<ValueId>,
) {
    match &op.kind {
        OpKind::Const { result } => {
            let constant = usize::try_from(result.0)
                .ok()
                .and_then(|index| function.constants.get(index));
            let scalar = constant.map_or(ScalarType::Unknown, scalar_constant);
            if scalar != ScalarType::Unknown {
                for (value, _) in &op.results {
                    values.insert(*value, proven(scalar));
                }
            }
        }
        OpKind::Move { value } => {
            let scalar = values.get(value).copied().unwrap_or(ScalarFact::Unknown);
            if scalar != ScalarFact::Unknown {
                for (result, _) in &op.results {
                    values.insert(*result, scalar);
                }
            }
        }
        OpKind::TypeHint { value, ty } => {
            let fact = match values.get(value).copied() {
                Some(
                    fact @ ScalarFact::Known {
                        evidence: Evidence::Proven,
                        ..
                    },
                ) => fact,
                Some(
                    ScalarFact::Known {
                        evidence: Evidence::Declared,
                        ..
                    }
                    | ScalarFact::Unknown,
                )
                | None => ScalarFact::Known {
                    ty: match ty {
                        ncl_ir::ScalarTypeHint::Fixnum => ScalarType::Fixnum,
                        ncl_ir::ScalarTypeHint::Integer => ScalarType::Integer,
                        ncl_ir::ScalarTypeHint::Number => ScalarType::Number,
                        ncl_ir::ScalarTypeHint::Boolean => ScalarType::Boolean,
                        ncl_ir::ScalarTypeHint::NonNumber => ScalarType::NonNumber,
                    },
                    evidence: Evidence::Declared,
                },
            };
            for (result, _) in &op.results {
                values.insert(*result, fact);
            }
        }
        OpKind::Convert {
            op: ncl_ir::Convert::I64ToWord,
            ..
        } => {
            for (result, _) in &op.results {
                if proven_fixnums.contains(result) {
                    values.insert(*result, proven(ScalarType::Fixnum));
                }
            }
        }
        OpKind::Compare { .. }
        | OpKind::Prim {
            op:
                Prim::FixnumLt
                | Prim::FixnumLe
                | Prim::FixnumEq
                | Prim::Eq
                | Prim::Eql
                | Prim::Typep
                | Prim::CharacterPredicate(_),
            ..
        } => {
            for (result, _) in &op.results {
                values.insert(*result, proven(ScalarType::Boolean));
            }
        }
        // check-added-lines: allow(wildcard) unhandled operations establish no scalar result facts.
        _ => {}
    }
}

/// Adds checked fixnum metadata to directly lowered arithmetic builtins.
#[derive(Clone, Debug, Default)]
pub struct TypeEffect;

impl FunctionPass for TypeEffect {
    fn name(&self) -> &'static str {
        "type-effect"
    }

    fn run(&mut self, function: &mut Function, _module: &Module) -> PassResult {
        let facts = FunctionFacts::analyze(function);
        let mut changed = false;
        for block in &mut function.blocks {
            for op in &mut block.ops {
                let OpKind::Builtin {
                    name,
                    args,
                    specialization,
                    specialization_disabled,
                } = &mut op.kind
                else {
                    continue;
                };
                let Some(operation) = FixnumOp::from_builtin(name, args.len()) else {
                    continue;
                };
                if *specialization_disabled {
                    if specialization.take().is_some() {
                        changed = true;
                    }
                    continue;
                }
                let guards = args
                    .iter()
                    .map(|value| {
                        !matches!(
                            facts.values.get(value),
                            Some(ScalarFact::Known {
                                ty: ScalarType::Fixnum,
                                evidence: Evidence::Proven,
                            })
                        )
                    })
                    .collect::<Vec<_>>();
                let next = FixnumSpecialization::new(operation, &guards);
                if *specialization != next {
                    *specialization = next;
                    changed = true;
                }
            }
        }
        Ok(changed)
    }
}

const fn proven(ty: ScalarType) -> ScalarFact {
    ScalarFact::Known {
        ty,
        evidence: Evidence::Proven,
    }
}

fn scalar_constant(constant: &Constant) -> ScalarType {
    match constant {
        Constant::Fixnum(value) if (i64::MIN / 2..=i64::MAX / 2).contains(value) => {
            ScalarType::Fixnum
        }
        Constant::SingleFloat(_)
        | Constant::DoubleFloat(_)
        | Constant::Ratio { .. }
        | Constant::Complex { .. } => ScalarType::Number,
        Constant::Bignum { .. } | Constant::Fixnum(_) => ScalarType::Integer,
        Constant::Nil
        | Constant::T
        | Constant::Character(_)
        | Constant::Symbol { .. }
        | Constant::UninternedSymbol { .. }
        | Constant::StringBytes(_)
        | Constant::Structure { .. }
        | Constant::LiteralObject { .. }
        | Constant::FunctionEntry(_)
        | Constant::Unbound => ScalarType::NonNumber,
        Constant::Object(_) => ScalarType::Unknown,
    }
}

fn effect_for_terminator(terminator: &ncl_ir::Terminator) -> EffectSet {
    match terminator {
        ncl_ir::Terminator::CallReturn { .. } | ncl_ir::Terminator::TailCall { .. } => {
            EffectSet::UNKNOWN_CALL
        }
        ncl_ir::Terminator::Throw { .. } => EffectSet::default().with(Effect::Signal),
        ncl_ir::Terminator::Jump { .. }
        | ncl_ir::Terminator::Branch { .. }
        | ncl_ir::Terminator::Switch { .. }
        | ncl_ir::Terminator::Return { .. }
        | ncl_ir::Terminator::Unreachable => EffectSet::default(),
    }
}

fn effect_for(kind: &OpKind) -> EffectSet {
    let mut effects = EffectSet::default();
    match kind {
        OpKind::Load { .. } | OpKind::LoadField { .. } => {
            effects = effects.with(Effect::ReadMemory);
        }
        OpKind::Store { .. } | OpKind::SetMultipleValues { .. } => {
            effects = effects.with(Effect::WriteMemory);
        }
        OpKind::Alloc { .. } => {
            effects = effects
                .with(Effect::Call)
                .with(Effect::Allocate)
                .with(Effect::Signal);
        }
        OpKind::Call { .. }
        | OpKind::CallIndirect { .. }
        | OpKind::CallClosure { .. }
        | OpKind::MakeClosure { .. }
        | OpKind::MakeValueCell { .. }
        | OpKind::Builtin { .. }
        | OpKind::Safepoint
        | OpKind::EnterHandler { .. }
        | OpKind::LeaveHandler { .. }
        | OpKind::StoreField { .. } => effects = EffectSet::UNKNOWN_CALL,
        OpKind::Prim { op, .. } => {
            effects = match op {
                Prim::Car | Prim::Cdr | Prim::Svref | Prim::Aref => {
                    effects.with(Effect::ReadMemory)
                }
                Prim::Rplaca | Prim::Rplacd | Prim::Aset => EffectSet::UNKNOWN_CALL,
                Prim::FixnumDiv => effects.with(Effect::Signal),
                Prim::Typep | Prim::CharacterPredicate(_) | Prim::StructureSlot(_) => {
                    EffectSet::UNKNOWN_CALL
                }
                Prim::FixnumAdd
                | Prim::FixnumSub
                | Prim::FixnumMul
                | Prim::FixnumLt
                | Prim::FixnumLe
                | Prim::FixnumEq
                | Prim::Eq
                | Prim::Eql => effects,
            };
        }
        OpKind::Const { .. }
        | OpKind::Move { .. }
        | OpKind::TypeHint { .. }
        | OpKind::LoadArg { .. }
        | OpKind::LoadCapture { .. }
        | OpKind::LoadFunctionObject
        | OpKind::Compare { .. }
        | OpKind::Convert { .. } => {}
    }
    effects
}

#[cfg(test)]
#[path = "tests_type_effect.rs"]
mod tests;
