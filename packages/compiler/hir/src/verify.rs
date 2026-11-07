//! Structural verification for one form of semantic HIR.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::binding::{BindingEnvironment, ScopeId};
use crate::cl_type::TypeArena;
use crate::control::ControlFlowGraph;
use crate::ids::{
    BindingId, BlockId, CleanupId, CompilationUnitId, FormId, IdError, NodeId, SourceOriginId,
    SymbolId, TypeId, TypeSpecConstId, ValueId,
};
use crate::node::NodeArena;
use crate::policy::OptimizationPolicy;
use crate::source::SourceOrigins;

mod control;
mod facts;
mod references;
mod structure;

pub use facts::*;

/// Exact identity indices supplied by the compilation-unit and form tables.
#[derive(Clone, Copy, Debug)]
pub struct VerifyResources<'a> {
    compilation_unit: CompilationUnitId,
    symbols: &'a HashSet<SymbolId>,
    values: &'a HashSet<ValueId>,
    type_spec_constants: &'a HashSet<TypeSpecConstId>,
}

impl<'a> VerifyResources<'a> {
    /// Supplies the exact interned IDs present in the related identity tables.
    #[must_use]
    pub const fn new(
        compilation_unit: CompilationUnitId,
        symbols: &'a HashSet<SymbolId>,
        values: &'a HashSet<ValueId>,
        type_spec_constants: &'a HashSet<TypeSpecConstId>,
    ) -> Self {
        Self {
            compilation_unit,
            symbols,
            values,
            type_spec_constants,
        }
    }
}

/// HIR data and resource bounds for one form.
pub struct VerifyInput<'a> {
    /// The top-level form being checked.
    pub form: FormId,
    /// Semantic node arena for the form.
    pub nodes: &'a NodeArena,
    /// Type lattice for the form.
    pub types: &'a TypeArena,
    /// Lexical bindings and scopes for the form.
    pub bindings: &'a BindingEnvironment,
    /// Control-flow graph for the form.
    pub control_flow: &'a ControlFlowGraph,
    /// Source origins for the form.
    pub origins: &'a SourceOrigins,
    /// Cached facts and current source epochs.
    pub facts: &'a FactStore,
    /// Epochs after all preceding passes.
    pub epochs: FactEpochs,
    /// Active check policy.
    pub policy: OptimizationPolicy,
    /// Exact IDs present in the compilation-unit and form-local identity tables.
    pub resources: VerifyResources<'a>,
}

/// A structural invariant rejected by the HIR verifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum VerifyError {
    /// A typed identity index exceeded its supported range.
    Id(IdError),
    /// An arena or resource table has a different owner.
    WrongForm,
    /// A symbol is foreign or outside the symbol table.
    InvalidSymbol(SymbolId),
    /// A node identity does not resolve.
    InvalidNode(NodeId),
    /// A type identity does not resolve.
    InvalidType(TypeId),
    /// A lexical binding identity does not resolve.
    InvalidBinding(BindingId),
    /// A scope does not resolve or is not a valid descendant.
    InvalidScope(ScopeId),
    /// A scope parent is not earlier than its child.
    InvalidScopeTree(ScopeId),
    /// A basic block does not resolve.
    InvalidBlock(BlockId),
    /// A source origin does not resolve.
    InvalidOrigin(SourceOriginId),
    /// An SSA value is outside the form-local table.
    InvalidValue(ValueId),
    /// A type-specifier constant is outside the form-local pool.
    InvalidTypeSpecConstant(TypeSpecConstId),
    /// A cleanup has no matching UNWIND-PROTECT node.
    InvalidCleanup(CleanupId),
    /// A variable reference does not resolve to the visible binding.
    InvisibleVariable { node: NodeId, binding: BindingId },
    /// A local function reference does not resolve to the visible binding.
    InvisibleFunction { node: NodeId, binding: BindingId },
    /// Scope metadata is inconsistent with lexical bindings.
    InvalidBindingScopes(NodeId),
    /// A non-local exit names a node of the wrong semantic kind.
    InvalidExitTarget(NodeId),
    /// GO names a block that is not a TAGBODY tag.
    InvalidTagTarget(BlockId),
    /// A function flow or its entry declaration is inconsistent.
    InvalidFunctionFlow(NodeId),
    /// A block is duplicated or owned by another function.
    InvalidFunctionBlock(BlockId),
    /// A successor is absent or belongs to another function.
    InvalidSuccessor { source: BlockId, target: BlockId },
    /// A check has no unique success/failure conditional edge.
    InvalidCheckEdges(NodeId),
    /// CheckType diagnostic origin differs from the node origin.
    CheckTypeOriginMismatch(NodeId),
    /// A cached fact was made against an older source epoch.
    StaleFact(FactId),
    /// A cached fact has an invalid provenance origin.
    InvalidFactOrigin(FactId),
    /// Unknown or unproven evidence claims to remove a type check.
    UnprovenCheckElision(FactId),
    /// A check-elision proof contradicts policy or the type lattice.
    InvalidCheckElision(FactId),
    /// Advancing a source epoch would overflow.
    EpochOverflow(FactDomain),
}

impl Display for VerifyError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "HIR verification failed: {self:?}")
    }
}

impl Error for VerifyError {}

impl From<IdError> for VerifyError {
    fn from(error: IdError) -> Self {
        Self::Id(error)
    }
}

/// Verifies identities, scopes, CFG edges, source origins, checks, and facts.
///
/// Symbol, value, and type-specifier tables use dense append-only identities.
/// A CheckType true edge is success; its false edge is the runtime type-error path.
///
/// # Errors
///
/// Returns the first invalid identity, lexical reference, edge, or stale fact.
pub fn verify(input: VerifyInput<'_>) -> Result<(), VerifyError> {
    let mut verifier = Verifier {
        input,
        cleanups: HashSet::new(),
        tag_blocks: HashSet::new(),
        check_edges: HashMap::new(),
    };
    verifier.owners()?;
    verifier.origins()?;
    verifier.types()?;
    verifier.prepare_cleanups()?;
    verifier.nodes()?;
    let check_edges = verifier.cfg()?;
    verifier.check_edges = check_edges;
    verifier.check_edges()?;
    verifier.verify_facts()
}

struct Verifier<'a> {
    input: VerifyInput<'a>,
    cleanups: HashSet<CleanupId>,
    tag_blocks: HashSet<BlockId>,
    check_edges: HashMap<NodeId, usize>,
}
