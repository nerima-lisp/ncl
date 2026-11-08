//! Semantic intermediate representation for Common Lisp compilation.
//!
//! `ncl-hir` stores Lisp-level operations, lexical identity, control flow, type
//! facts, and source origins before machine representation is selected.

/// Checked append-only arenas for form-local identities.
pub mod arena;
/// Lexical bindings and function namespaces for a HIR form.
pub mod binding;
pub mod cl_type;
pub mod control;
pub mod form;
pub mod ids;
pub mod lower;
pub mod node;
pub mod policy;
pub mod source;
pub mod symbols;
pub mod verify;

pub use binding::{
    Binding, BindingEnvironment, BindingError, BindingKind, BindingNamespace, LocalFunctionGroup,
    LocalFunctionKind, ScopeId,
};
pub use cl_type::{
    ArrayType, BuiltinType, ClType, ClTypeError, ClTypeKind, FunctionParameters, FunctionType,
    MemberValue, NumericRange, OpaqueOperand, SubtypeRelation, TypeArena,
};
pub use control::{
    BasicBlock, ControlError, ControlFlowBuilder, ControlFlowGraph, ExitTarget, FunctionFlow,
    Terminator,
};
pub use form::{
    FormError, HirDeclaration, HirForm, HirFormBuilder, HirQuality, LambdaAux, LambdaKey,
    LambdaOptional, LambdaSignature, LambdaSignatureKey, OptimizationSetting, ScopeMetadata,
    TypeSpecConstantArena,
};
pub use ids::{
    ArenaIndex, BindingId, BlockId, CleanupId, CompilationUnitId, FormId, IdError, NodeId,
    SourceOriginId, SymbolId, TypeId, TypeSpecConstId, ValueId,
};
pub use node::{
    CallTarget, CompileTimeSituation, DefinitionKind, LetBinding, LetKind, Literal,
    LiteralElementType, LocalFunctionDefinition, MultipleValueOperation, Node, NodeArena, NodeKind,
    NumberLiteral, RuntimeCheckKind, TagbodyTag, VariableTarget,
};
pub use policy::{OptimizationPolicy, OptimizationQuality, OptimizationQualityError};
pub use source::{
    SourceError, SourceFile, SourceFileId, SourceFiles, SourceLocation, SourceOrigin,
    SourceOriginKind, SourceOrigins, SourcePosition, SourceSpan, SourceTransform,
    SourceTransformKind,
};
pub use symbols::{HirSymbol, SymbolError, SymbolTable};
pub use verify::{
    FactAuthorization, FactCertainty, FactDomain, FactEpochs, FactId, FactProvenance, FactRecord,
    FactStore, PassInvalidations, VerificationBuild, VerificationConfig, VerifyError,
};
