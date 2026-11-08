//! Lower verified HIR to the current machine-independent IR.

mod checks;
mod control;
mod exits;
mod expr;
mod handlers;
mod lambda;
mod literal;
mod local_functions;
mod multiple_values;
mod ops;

use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};

use ncl_ir::{
    Function, FunctionBuilder, FunctionId, HandlerRegion, HandlerRegionId, Param, Terminator, Ty,
    ValueId,
};

use crate::form::HirForm;
use crate::ids::{NodeId, TypeSpecConstId};
use crate::symbols::SymbolTable;
use crate::verify::VerifyError;

/// The functions produced by lowering one HIR form.
#[derive(Clone, Debug)]
pub struct Lowered {
    /// The function for the lowered form itself.
    pub entry: Function,
    /// Nested lambda functions in lowering order.
    pub nested: Vec<Function>,
}

/// An error while translating HIR to the current IR instruction set.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum LowerError {
    /// The HIR form failed its structural verifier.
    InvalidHir(VerifyError),
    /// The form contains no expression roots.
    EmptyForm,
    /// A referenced HIR node is absent from the form arena.
    InvalidNode(NodeId),
    /// A type assertion references a missing type-specifier constant.
    InvalidTypeSpecConstant(TypeSpecConstId),
    /// A lexical value is used outside a lowered binding.
    MissingLexicalValue(crate::ids::BindingId),
    /// A local function is used outside its lowered definition group.
    MissingLocalFunction(crate::ids::BindingId),
    /// A check refers to a HIR value with no lowering-side producer mapping.
    UnmappedHirValue {
        /// The check node requesting the value.
        node: NodeId,
        /// The HIR value identity with no known producer.
        value: crate::ids::ValueId,
    },
    /// A referenced symbol has no spelling in the symbol table.
    InvalidSymbol(crate::ids::SymbolId),
    /// The requested node has no faithful representation in the current IR.
    Unsupported {
        /// The HIR node that cannot be represented.
        node: NodeId,
        /// The unsupported semantic operation.
        operation: &'static str,
    },
    /// A MIR builder operation rejected the constructed function.
    Ir(String),
    /// The generated function failed the MIR verifier.
    InvalidMir(Vec<ncl_ir::VerifyError>),
}

impl Display for LowerError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "HIR lowering failed: {self:?}")
    }
}

impl Error for LowerError {}

/// Lower one verified HIR form into an entry function and its nested functions.
///
/// # Errors
///
/// Returns [`LowerError`] when the HIR is invalid, a node cannot be represented,
/// or the generated MIR fails verification.
pub fn lower(form: &HirForm, symbols: &SymbolTable) -> Result<Lowered, LowerError> {
    form.verify(symbols).map_err(LowerError::InvalidHir)?;
    let mut pool = FunctionPool::default();
    let mut lowering = FunctionLowerer::new(form, symbols, &mut pool);
    let roots = form.roots();
    let Some((last, preceding)) = roots.split_last() else {
        return Err(LowerError::EmptyForm);
    };
    for root in preceding {
        if lowering.is_terminated() {
            break;
        }
        let _ = lowering.node(*root)?;
    }
    if !lowering.is_terminated() {
        let result = lowering.node(*last)?;
        if !lowering.is_terminated() {
            lowering.terminate(ncl_ir::Terminator::Return {
                values: vec![result],
            })?;
        }
    }
    let entry = lowering.finish();
    ncl_ir::verify(&entry).map_err(LowerError::InvalidMir)?;
    pool.functions.sort_by_key(|function| function.id);
    for function in &pool.functions {
        ncl_ir::verify(function).map_err(LowerError::InvalidMir)?;
    }
    Ok(Lowered {
        entry,
        nested: pool.functions,
    })
}

#[derive(Default)]
pub(super) struct FunctionPool {
    next_id: u32,
    functions: Vec<Function>,
}

impl FunctionPool {
    pub(super) fn allocate(&mut self) -> Result<FunctionId, LowerError> {
        let id = self
            .next_id
            .checked_add(1)
            .ok_or_else(|| LowerError::Ir("nested function identity overflow".to_owned()))?;
        self.next_id = id;
        Ok(FunctionId(id))
    }
}

pub(super) struct FunctionLowerer<'a> {
    pub(super) form: &'a HirForm,
    pub(super) symbols: &'a SymbolTable,
    pub(super) pool: &'a mut FunctionPool,
    pub(super) builder: FunctionBuilder,
    pub(super) current_block: ncl_ir::BlockId,
    pub(super) created_blocks: Vec<ncl_ir::BlockId>,
    pub(super) terminators: HashMap<ncl_ir::BlockId, Terminator>,
    pub(super) terminated_blocks: std::collections::HashSet<ncl_ir::BlockId>,
    pub(super) handler_regions: Vec<HandlerRegion>,
    pub(super) active_handlers: Vec<HandlerRegionId>,
    pub(super) next_handler_id: u32,
    pub(super) values: HashMap<NodeId, ValueId>,
    pub(super) lexical_values: HashMap<crate::ids::BindingId, ValueId>,
    pub(super) lexical_cells: std::collections::HashSet<crate::ids::BindingId>,
    pub(super) local_functions: HashMap<crate::ids::BindingId, ValueId>,
    pub(super) local_function_cells: std::collections::HashSet<crate::ids::BindingId>,
    pub(super) block_targets: HashMap<NodeId, BlockTarget>,
    pub(super) tag_targets: HashMap<crate::ids::BlockId, (ncl_ir::BlockId, usize)>,
    pub(super) cleanup_forms: HashMap<crate::ids::CleanupId, Vec<NodeId>>,
    pub(super) literal_objects: HashMap<u32, ncl_ir::ConstantIndex>,
    pub(super) constant_count: usize,
    pub(super) branch_depth: usize,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct BlockTarget {
    pub(super) block: ncl_ir::BlockId,
    pub(super) active_depth: usize,
}

impl<'a> FunctionLowerer<'a> {
    fn new(form: &'a HirForm, symbols: &'a SymbolTable, pool: &'a mut FunctionPool) -> Self {
        Self::for_function(form, symbols, pool, FunctionId(0), "toplevel", Vec::new())
    }

    pub(super) fn for_function(
        form: &'a HirForm,
        symbols: &'a SymbolTable,
        pool: &'a mut FunctionPool,
        id: FunctionId,
        name: impl Into<String>,
        params: Vec<Param>,
    ) -> Self {
        Self {
            form,
            symbols,
            pool,
            builder: FunctionBuilder::new(id, name, params, vec![Ty::Word]),
            current_block: ncl_ir::BlockId(0),
            created_blocks: vec![ncl_ir::BlockId(0)],
            terminators: HashMap::new(),
            terminated_blocks: std::collections::HashSet::new(),
            handler_regions: Vec::new(),
            active_handlers: Vec::new(),
            next_handler_id: 0,
            values: HashMap::new(),
            lexical_values: HashMap::new(),
            lexical_cells: std::collections::HashSet::new(),
            local_functions: HashMap::new(),
            local_function_cells: std::collections::HashSet::new(),
            block_targets: HashMap::new(),
            tag_targets: HashMap::new(),
            cleanup_forms: HashMap::new(),
            literal_objects: HashMap::new(),
            constant_count: 0,
            branch_depth: 0,
        }
    }

    pub(super) fn finish(mut self) -> Function {
        for region in self.handler_regions.drain(..) {
            self.builder.add_handler_region(region);
        }
        self.builder.finish()
    }
}
