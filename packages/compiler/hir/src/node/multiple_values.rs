use crate::binding::ScopeId;
use crate::ids::{BindingId, NodeId};

/// Multiple-value operations preserved until a calling convention is chosen.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MultipleValueOperation {
    /// Produce values in the listed order.
    Produce {
        /// Values in source order.
        values: Vec<NodeId>,
    },
    /// Bind the returned values to lexical bindings for a body.
    Bind {
        /// The form producing the values.
        value: NodeId,
        /// Bindings receiving the values, in order.
        bindings: Vec<BindingId>,
        /// Scope containing these bindings and the body.
        body_scope: ScopeId,
        /// The body evaluated with the bindings.
        body: Vec<NodeId>,
    },
    /// Return values in order.
    Return {
        /// Values in source order.
        values: Vec<NodeId>,
    },
    /// Call a function with values from each argument form.
    Call {
        /// The function form.
        function: NodeId,
        /// Argument forms evaluated for their values.
        arguments: Vec<NodeId>,
    },
    /// Preserve the values of the first form while evaluating the remaining forms.
    Prog1 {
        /// The form whose values are returned.
        first: NodeId,
        /// Forms evaluated after `first`, in source order.
        rest: Vec<NodeId>,
    },
}
