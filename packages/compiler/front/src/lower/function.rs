//! Per-function lowering state and the module-level function registry.

use ncl_ir::{
    BlockId, Constant, Function, FunctionBuilder, FunctionId, OpKind, Param, Terminator, Ty,
    ValueId,
};

use crate::symbols::SymbolRef;

use super::env::LowerEnv;
use super::error::LowerError;

/// The functions produced while lowering one form.
///
/// `ncl-ir` has no constant or op that names a function entry, so a lambda
/// cannot be linked into its enclosing function. The nested functions are
/// collected here and returned alongside the entry function for the runtime to
/// register.
#[derive(Debug, Default)]
pub(super) struct Lowerer {
    next_function: u32,
    functions: Vec<Function>,
}

impl Lowerer {
    /// Create an empty registry.
    pub(super) const fn new() -> Self {
        Self {
            next_function: 0,
            functions: Vec::new(),
        }
    }

    /// Allocate the next function identity.
    pub(super) const fn fresh_function(&mut self) -> FunctionId {
        let id = FunctionId(self.next_function);
        self.next_function += 1;
        id
    }

    /// Register a lowered nested function.
    pub(super) fn push_function(&mut self, function: Function) {
        self.functions.push(function);
    }

    /// Consume the registry and return the nested functions.
    pub(super) fn into_functions(self) -> Vec<Function> {
        self.functions
    }
}

/// The lowering state for one generated function.
#[derive(Debug)]
pub(super) struct FunctionLowerer {
    builder: FunctionBuilder,
    env: LowerEnv,
    current: BlockId,
    terminated: bool,
}

impl FunctionLowerer {
    /// Create a lowerer for a function with the given signature.
    pub(super) fn new(
        id: FunctionId,
        name: impl Into<String>,
        params: Vec<Param>,
        return_types: Vec<Ty>,
    ) -> Self {
        Self {
            builder: FunctionBuilder::new(id, name, params, return_types),
            env: LowerEnv::new(),
            current: BlockId(0),
            terminated: false,
        }
    }

    /// Create the entry lowerer for a top-level form.
    pub(super) fn entry(name: &str) -> Self {
        Self::new(FunctionId(0), name, Vec::new(), vec![Ty::Word])
    }

    /// The environment of this function.
    pub(super) const fn env(&mut self) -> &mut LowerEnv {
        &mut self.env
    }

    /// The block operations are currently appended to.
    pub(super) const fn current_block(&self) -> BlockId {
        self.current
    }

    /// Whether the current block already has a terminator.
    pub(super) const fn is_terminated(&self) -> bool {
        self.terminated
    }

    /// Allocate a fresh SSA value identity.
    pub(super) const fn fresh_value(&mut self) -> ValueId {
        self.builder.fresh_value()
    }

    /// Append an operation with the given result types.
    pub(super) fn push(
        &mut self,
        kind: OpKind,
        results: &[Ty],
    ) -> Result<Vec<ValueId>, LowerError> {
        self.ensure_open();
        Ok(self.builder.push_op(kind, results)?)
    }

    /// Append an operation with one result.
    pub(super) fn one(&mut self, kind: OpKind, ty: Ty) -> Result<ValueId, LowerError> {
        let mut results = self.push(kind, &[ty])?;
        results.pop().ok_or_else(|| LowerError::Ir {
            detail: "operation produced no result".to_owned(),
        })
    }

    /// Append an operation with no result.
    pub(super) fn none(&mut self, kind: OpKind) -> Result<(), LowerError> {
        self.push(kind, &[]).map(|_| ())
    }

    /// Append a constant and return its value.
    pub(super) fn constant(&mut self, constant: Constant) -> Result<ValueId, LowerError> {
        let ty = constant_type(&constant);
        let index = self.builder.add_constant(constant);
        self.one(OpKind::Const { result: index }, ty)
    }

    /// Append an `I64` fixnum constant.
    pub(super) fn fixnum(&mut self, value: i64) -> Result<ValueId, LowerError> {
        self.constant(Constant::Fixnum(value))
    }

    /// Append a `Word` constant.
    pub(super) fn word_constant(&mut self, constant: Constant) -> Result<ValueId, LowerError> {
        self.constant(constant)
    }

    /// Append `nil` as a `Word`.
    pub(super) fn nil(&mut self) -> Result<ValueId, LowerError> {
        self.word_constant(Constant::Nil)
    }

    /// Append the `Word` descriptor for a symbol.
    pub(super) fn symbol(&mut self, symbol: &SymbolRef) -> Result<ValueId, LowerError> {
        self.word_constant(Constant::Symbol {
            package: symbol
                .package
                .clone()
                .unwrap_or_else(|| "NCL-UNINTERNED".to_owned()),
            name: symbol.name.clone(),
        })
    }

    /// Append a GC safepoint.
    pub(super) fn safepoint(&mut self) -> Result<(), LowerError> {
        self.none(OpKind::Safepoint)
    }

    /// Create a block with the given parameters and select it.
    pub(super) fn create_block(&mut self, params: Vec<(Ty, ValueId)>) -> BlockId {
        let id = self.builder.create_block(params);
        self.current = id;
        self.terminated = false;
        id
    }

    /// Select an existing block.
    pub(super) fn position(&mut self, block: BlockId) -> Result<(), LowerError> {
        self.builder.position_at(block)?;
        self.current = block;
        self.terminated = false;
        Ok(())
    }

    /// Set the current block's terminator.
    pub(super) fn terminate(&mut self, terminator: Terminator) -> Result<(), LowerError> {
        self.builder.terminate(terminator)?;
        self.terminated = true;
        Ok(())
    }

    /// Finish the current block with a `Return` of one value.
    pub(super) fn return_value(&mut self, value: ValueId) -> Result<(), LowerError> {
        self.ensure_open();
        self.terminate(Terminator::Return {
            values: vec![value],
        })
    }

    /// Consume the lowerer and return the built function.
    pub(super) fn into_function(self) -> Function {
        self.builder.finish()
    }

    /// Open a fresh block when the current one is already terminated.
    fn ensure_open(&mut self) {
        if self.terminated {
            self.current = self.builder.create_block(Vec::new());
            self.terminated = false;
        }
    }
}

/// The IR type a constant evaluates to.
const fn constant_type(constant: &Constant) -> Ty {
    match constant {
        Constant::Fixnum(_) => Ty::I64,
        Constant::SingleFloat(_) | Constant::DoubleFloat(_) => Ty::F64,
        Constant::Character(_)
        | Constant::Symbol { .. }
        | Constant::Object(_)
        | Constant::StringBytes(_)
        | Constant::Nil
        | Constant::T
        | Constant::Unbound
        | Constant::FunctionEntry(_) => Ty::Word,
    }
}
