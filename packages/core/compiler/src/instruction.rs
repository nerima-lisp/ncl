//! The single stack-bytecode instruction set emitted by the compiler.
//!
//! `Instruction` is one `pub enum`; its variants must stay in one contiguous
//! block, so this file exceeds the usual per-file line convention rather
//! than being split in a way that would change the type's definition.

use crate::{
    Constant, DestructureSpec, FunctionId, HandlerBindClause, HandlerCaseClause, RestartBindClause,
    RestartCaseClause,
};
use ncl_syntax::{Form, Span};

#[derive(Clone, Debug, PartialEq)]
/// A stack-bytecode instruction emitted by the compiler.
pub enum Instruction {
    /// Push a literal constant.
    Constant(Constant),
    /// Push a quoted form.
    Quote(Form),
    /// Push a quasiquoted form.
    QuasiQuote(Form),
    /// Load a symbol by normal name resolution.
    Load(String),
    /// Load an escaped symbol.
    LoadExact(String),
    /// Load a function by normal name resolution.
    FunctionLoad(String),
    /// Load an escaped function name.
    FunctionLoadExact(String),
    /// Test whether a variable is bound.
    IsBound(String),
    /// Test whether an escaped variable is bound.
    IsBoundExact(String),
    /// Define a variable.
    Define(String),
    /// Define an escaped variable.
    DefineExact(String),
    /// Define a function.
    DefineFunction(String),
    /// Define an escaped function name.
    DefineFunctionExact(String),
    /// Define a special variable.
    DefineSpecial {
        /// Variable name.
        name: String,
        /// Whether to force special binding semantics.
        force: bool,
    },
    /// Define an escaped special variable.
    DefineSpecialExact {
        /// Escaped variable name.
        name: String,
        /// Whether to force special binding semantics.
        force: bool,
    },
    /// Define multiple values.
    DefineValues(String),
    /// Define multiple values using escaped names.
    DefineValuesExact(String),
    /// Set a variable.
    Set(String),
    /// Set an escaped variable.
    SetExact(String),
    /// Perform a `SETF` update.
    Setf(Form),
    /// Perform a place update with `MAP-INTO` semantics.
    MapIntoSetf(Form),
    /// Perform parallel assignment.
    Psetq(Vec<String>),
    /// Perform escaped parallel assignment.
    PsetqExact(Vec<(String, bool)>),
    /// Bind multiple-value assignment targets.
    MultipleValueSetq(Vec<String>),
    /// Bind escaped multiple-value assignment targets.
    MultipleValueSetqExact(Vec<(String, bool)>),
    /// Enter a lexical scope.
    EnterScope,
    /// Exit a lexical scope.
    ExitScope,
    /// Discard the top stack value.
    Pop,
    /// Duplicate the top stack value.
    Dup,
    /// Replace the stack with the primary value.
    Primary,
    /// Construct a multiple-value carrier.
    Values(usize),
    /// Convert a multiple-value carrier to a list.
    MultipleValueList,
    /// Bind multiple values to names.
    BindValues(Vec<String>),
    /// Bind multiple values to escaped names.
    BindValuesExact(Vec<(String, bool)>),
    /// Destructure a value.
    Destructure(DestructureSpec),
    /// Branch when the top value is false.
    JumpIfFalse(usize),
    /// Unconditional branch.
    Jump(usize),
    /// Create a closure for a nested function.
    MakeClosure(FunctionId),
    /// Evaluate a function while ignoring conditions.
    IgnoreErrors(FunctionId),
    /// Run a body with condition handlers selected by type.
    HandlerCase {
        /// Protected function.
        protected: FunctionId,
        /// Handler clauses.
        clauses: Vec<HandlerCaseClause>,
    },
    /// Install dynamically scoped handlers around a body.
    HandlerBind {
        /// Body function.
        body: FunctionId,
        /// Handler clauses.
        handlers: Vec<HandlerBindClause>,
    },
    /// Install dynamically scoped restarts around a body.
    RestartBind {
        /// Body function.
        body: FunctionId,
        /// Restart bindings.
        bindings: Vec<RestartBindClause>,
    },
    /// Catch a matching tag from a body.
    Catch {
        /// Tag-producing function.
        tag: FunctionId,
        /// Body function.
        body: FunctionId,
    },
    /// Establish a simple restart around a body.
    WithSimpleRestart {
        /// Restart name.
        name: String,
        /// Body function.
        body: FunctionId,
    },
    /// Establish restarts associated with a condition.
    WithConditionRestarts {
        /// Condition function.
        condition: FunctionId,
        /// Restart list function.
        restarts: FunctionId,
        /// Body function.
        body: FunctionId,
    },
    /// Run a body with restart-case clauses.
    RestartCase {
        /// Protected function.
        protected: FunctionId,
        /// Restart clauses.
        clauses: Vec<RestartCaseClause>,
    },
    /// Bind a dynamic set of special variables around a body.
    Progv {
        /// Symbols function.
        symbols: FunctionId,
        /// Values function.
        values: FunctionId,
        /// Body function.
        body: FunctionId,
    },
    /// Throw the current tag and values.
    Throw,
    /// Establish a named non-local return target.
    Block {
        /// Body function.
        function: FunctionId,
        /// Block name.
        name: String,
    },
    /// Establish a tagbody control-flow region.
    TagBody {
        /// Body function.
        function: FunctionId,
        /// Tag-to-offset mapping.
        tags: Vec<(String, usize)>,
    },
    /// Run cleanup even when protected evaluation exits non-locally.
    UnwindProtect {
        /// Protected function.
        protected: FunctionId,
        /// Cleanup function.
        cleanup: FunctionId,
    },
    /// Return from a named block.
    ReturnFrom {
        /// Block name.
        name: String,
    },
    /// Transfer control to a tagbody tag.
    Go {
        /// Tag name.
        tag: String,
    },
    /// Evaluate a compiled source span.
    Eval(Span),
    /// Call a function with positional arguments.
    Call(usize),
    /// Apply a final list of arguments.
    Apply(usize),
    /// Map a function over one or more sequences.
    MapCar(usize),
    /// Call a function with multiple-value arguments.
    MultipleValueCall(usize),
    /// Return from the current function.
    Return,
}
