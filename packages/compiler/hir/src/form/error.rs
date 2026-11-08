use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::binding::ScopeId;
use crate::cl_type::ClTypeError;
use crate::control::ControlError;
use crate::ids::{
    BindingId, FormId, IdError, NodeId, SourceOriginId, SymbolId, TypeId, TypeSpecConstId, ValueId,
};

/// An invalid form construction request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FormError {
    /// An arena index space is exhausted.
    Id(IdError),
    /// The type lattice could not be initialized.
    Type(ClTypeError),
    /// A control-flow graph is incomplete or inconsistent.
    Control(ControlError),
    /// A source-origin table or identity belongs to another form.
    ForeignForm(FormId),
    /// A source origin does not exist in this form.
    InvalidOrigin(SourceOriginId),
    /// A scope belongs to another form.
    ForeignScope(ScopeId),
    /// A scope does not exist in this form.
    InvalidScope(ScopeId),
    /// A node belongs to another form.
    ForeignNode(NodeId),
    /// A node does not exist in this form.
    InvalidNode(NodeId),
    /// A symbol belongs to another compilation unit.
    ForeignSymbol(SymbolId),
    /// An SSA value belongs to another form.
    ForeignValue(ValueId),
    /// An SSA value already has a producer node.
    DuplicateValueProducer(ValueId),
    /// A binding belongs to another form.
    ForeignBinding(BindingId),
    /// A binding or scope does not fit the function signature.
    InvalidFunctionSignature,
    /// A type belongs to another form.
    ForeignType(TypeId),
    /// A type does not exist in this form.
    InvalidType(TypeId),
    /// A type-specifier constant belongs to another form.
    ForeignTypeSpecConstant(TypeSpecConstId),
    /// A type-specifier constant does not exist in this form.
    InvalidTypeSpecConstant(TypeSpecConstId),
}

impl Display for FormError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "HIR form error: {self:?}")
    }
}

impl Error for FormError {}
