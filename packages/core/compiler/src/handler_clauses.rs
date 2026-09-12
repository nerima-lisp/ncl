//! Clause metadata for the condition-system compiled instructions.

use std::rc::Rc;

use crate::FunctionId;

/// Interned condition type name carried by compiled condition clauses.
pub type ConditionName = Rc<str>;

/// One compiled `HANDLER-CASE` clause.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandlerCaseClause {
    /// Condition type name.
    pub condition: ConditionName,
    /// Optional handler variable name.
    pub variable: Option<String>,
    /// Function containing the handler body.
    pub function: FunctionId,
}

/// One compiled `HANDLER-BIND` clause.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandlerBindClause {
    /// Condition type name.
    pub condition: ConditionName,
    /// Function containing the handler body.
    pub function: FunctionId,
}

/// One compiled `RESTART-BIND` clause.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestartBindClause {
    /// Restart name.
    pub name: String,
    /// Function containing the restart body.
    pub function: FunctionId,
}

/// One compiled `RESTART-CASE` clause.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestartCaseClause {
    /// Restart name.
    pub name: String,
    /// Function containing the restart body.
    pub function: FunctionId,
}
