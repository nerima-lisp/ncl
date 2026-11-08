use std::error::Error;
use std::fmt::{Display, Formatter};

use ncl_hir::{
    BindingError, ClTypeError, ControlError, FormError, IdError, OptimizationQualityError,
    SymbolError, VerifyError,
};

/// A front AST feature that the current HIR schema cannot preserve.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum UnsupportedHirForm {
    /// A lambda list uses macro-only destructuring parameters.
    LambdaPattern,
    /// A lambda expression carries a docstring with no HIR metadata slot.
    Docstring,
    /// The source type cannot be translated into the HIR lattice.
    TypeSpecifier,
    /// A lexical block reference is unresolved in the current builder context.
    UnknownBlock,
    /// A GO reference is unresolved in the current TAGBODY.
    UnknownTag,
    /// A local macro remains in the expanded AST.
    Macrolet,
    /// A local symbol macro remains in the expanded AST.
    SymbolMacrolet,
    /// A TAGBODY contains duplicate tag names.
    DuplicateTag,
    /// The frozen front AST lost an unknown optimization-quality name.
    UnknownOptimizationQuality,
    /// `FUNCALL` has no function designator argument.
    MalformedCall,
    /// A function reference resolves to a non-function lexical binding.
    FunctionBinding,
}

/// A typed failure while building HIR from a front AST.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum HirBuildError {
    /// The form or source metadata could not be constructed.
    Form(FormError),
    /// Symbol interning failed.
    Symbol(SymbolError),
    /// A lexical scope or binding was invalid.
    Binding(BindingError),
    /// A control-flow graph could not be built.
    Control(ControlError),
    /// A Common Lisp type node could not be represented.
    Type(ClTypeError),
    /// An optimization quality value is outside its valid range.
    Policy(OptimizationQualityError),
    /// The completed form did not satisfy the HIR verifier.
    Verification(VerifyError),
    /// A typed identity index overflowed.
    Id(IdError),
    /// The current HIR schema cannot preserve this source form.
    Unsupported(UnsupportedHirForm),
}

impl Display for HirBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "front-to-HIR bridge failed: {self:?}")
    }
}

impl Error for HirBuildError {}

impl From<FormError> for HirBuildError {
    fn from(error: FormError) -> Self {
        Self::Form(error)
    }
}

impl From<SymbolError> for HirBuildError {
    fn from(error: SymbolError) -> Self {
        Self::Symbol(error)
    }
}

impl From<BindingError> for HirBuildError {
    fn from(error: BindingError) -> Self {
        Self::Binding(error)
    }
}

impl From<ControlError> for HirBuildError {
    fn from(error: ControlError) -> Self {
        Self::Control(error)
    }
}

impl From<ClTypeError> for HirBuildError {
    fn from(error: ClTypeError) -> Self {
        Self::Type(error)
    }
}

impl From<OptimizationQualityError> for HirBuildError {
    fn from(error: OptimizationQualityError) -> Self {
        Self::Policy(error)
    }
}

impl From<IdError> for HirBuildError {
    fn from(error: IdError) -> Self {
        Self::Id(error)
    }
}
