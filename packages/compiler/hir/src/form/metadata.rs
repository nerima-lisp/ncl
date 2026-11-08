//! Source declarations and function signatures retained by the HIR bridge.

use std::collections::HashMap;

use crate::binding::ScopeId;
use crate::ids::{BindingId, NodeId, SymbolId, TypeId, TypeSpecConstId};
use crate::node::Literal;
use crate::policy::OptimizationPolicy;

/// Declarations and the effective policy for one lexical scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopeMetadata {
    /// Source declarations retained at this scope.
    pub declarations: Vec<HirDeclaration>,
    /// Effective optimization policy for this scope.
    pub policy: OptimizationPolicy,
}

/// A declaration preserved independently of runtime type objects.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum HirDeclaration {
    /// Names declared special.
    Special {
        /// Variables declared special.
        names: Vec<SymbolId>,
    },
    /// A variable type declaration and its original source specifier.
    Type {
        /// Parsed HIR type meaning.
        type_id: TypeId,
        /// Complete source type-specifier datum.
        source_specifier: TypeSpecConstId,
        /// Variables governed by the declaration.
        names: Vec<SymbolId>,
    },
    /// A function type declaration and its original source specifier.
    Ftype {
        /// Parsed HIR type meaning.
        type_id: TypeId,
        /// Complete source type-specifier datum.
        source_specifier: TypeSpecConstId,
        /// Function governed by the declaration.
        name: SymbolId,
    },
    /// Names declared inline.
    Inline {
        /// Function names declared inline.
        names: Vec<SymbolId>,
    },
    /// Names declared notinline.
    Notinline {
        /// Function names declared notinline.
        names: Vec<SymbolId>,
    },
    /// Optimization quality declarations in source order.
    Optimize {
        /// Quality declarations in source order.
        settings: Vec<OptimizationSetting>,
    },
    /// Names declared dynamic extent.
    DynamicExtent {
        /// Names with dynamic extent.
        names: Vec<SymbolId>,
    },
    /// Names declared ignored.
    Ignore {
        /// Names declared ignored.
        names: Vec<SymbolId>,
    },
    /// Names declared ignorable.
    Ignorable {
        /// Names declared ignorable.
        names: Vec<SymbolId>,
    },
    /// A declaration retained without interpretation.
    Unknown {
        /// Declaration specifier name.
        name: SymbolId,
        /// Original declaration operands.
        arguments: Vec<Literal>,
    },
}

/// One optimization quality declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OptimizationSetting {
    /// The quality named by the source declaration.
    pub quality: HirQuality,
    /// The requested value in the Common Lisp range 0 through 3.
    pub value: u8,
}

/// The optimization quality named by an `OPTIMIZE` declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum HirQuality {
    /// `SPEED`.
    Speed,
    /// `SPACE`.
    Space,
    /// `SAFETY`.
    Safety,
    /// `DEBUG`.
    Debug,
    /// `COMPILATION-SPEED`.
    CompilationSpeed,
    /// A quality the front end does not interpret.
    Unknown(SymbolId),
}

/// The variable forms accepted by an ordinary lambda list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LambdaSignature {
    /// Required arguments in declaration order.
    pub required: Vec<BindingId>,
    /// Optional arguments in declaration order.
    pub optional: Vec<LambdaOptional>,
    /// The `&REST` or `&BODY` binding, if present.
    pub rest: Option<BindingId>,
    /// Keyword arguments in declaration order.
    pub keys: Vec<LambdaKey>,
    /// Whether `&ALLOW-OTHER-KEYS` was present.
    pub allow_other_keys: bool,
    /// Auxiliary bindings in declaration order.
    pub aux: Vec<LambdaAux>,
}

/// One optional argument and its initializer scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LambdaOptional {
    /// The variable receiving the argument.
    pub binding: BindingId,
    /// The initializer form; `None` denotes NIL.
    pub default: Option<NodeId>,
    /// The scope visible while evaluating the initializer.
    pub initializer_scope: ScopeId,
    /// The optional supplied-p variable.
    pub supplied_p: Option<BindingId>,
}

/// One keyword argument and its initializer scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LambdaKey {
    /// The keyword designator accepted by the parameter.
    pub keyword: SymbolId,
    /// The variable receiving the argument.
    pub binding: BindingId,
    /// The initializer form; `None` denotes NIL.
    pub default: Option<NodeId>,
    /// The scope visible while evaluating the initializer.
    pub initializer_scope: ScopeId,
    /// The optional supplied-p variable.
    pub supplied_p: Option<BindingId>,
}

/// One auxiliary binding and its initializer scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LambdaAux {
    /// The variable introduced by this parameter.
    pub binding: BindingId,
    /// The initializer form; `None` denotes NIL.
    pub default: Option<NodeId>,
    /// The scope visible while evaluating the initializer.
    pub initializer_scope: ScopeId,
}

/// A lambda signature is attached either to a lambda node or local function.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum LambdaSignatureKey {
    /// Signature for an anonymous lambda node.
    Lambda(NodeId),
    /// Signature for a named FLET or LABELS function.
    LocalFunction(BindingId),
}

/// HIR metadata indexed by lexical scope and function identity.
#[derive(Clone, Debug, Default)]
pub struct FormMetadata {
    pub(super) scopes: HashMap<ScopeId, ScopeMetadata>,
    pub(super) lambda_signatures: HashMap<LambdaSignatureKey, LambdaSignature>,
}
