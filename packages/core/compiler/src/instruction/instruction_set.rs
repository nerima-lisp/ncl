use crate::{
    Constant, DestructureSpec, FunctionId, HandlerBindClause, HandlerCaseClause, RestartBindClause,
    RestartCaseClause,
};
use ncl_syntax::{Form, Span};

#[derive(Clone, Debug, PartialEq)]
/// A stack-bytecode instruction emitted by the compiler.
#[rustfmt::skip]
pub enum Instruction {
    #[doc = "Push a literal constant."] Constant(Constant),
    #[doc = "Push a quoted form."] Quote(Form),
    #[doc = "Push a quasiquoted form."] QuasiQuote(Form),
    #[doc = "Load a symbol by normal name resolution."] Load(String),
    #[doc = "Load an escaped symbol."] LoadExact(String),
    #[doc = "Load a function by normal name resolution."] FunctionLoad(String),
    #[doc = "Load an escaped function name."] FunctionLoadExact(String),
    #[doc = "Test whether a variable is bound."] IsBound(String),
    #[doc = "Test whether an escaped variable is bound."] IsBoundExact(String),
    #[doc = "Define a variable."] Define(String),
    #[doc = "Define an escaped variable."] DefineExact(String),
    #[doc = "Define a function."] DefineFunction(String),
    #[doc = "Define an escaped function name."] DefineFunctionExact(String),
    #[doc = "Define a special variable."] DefineSpecial {
        /// Variable name.
        name: String,
        /// Whether to force special binding semantics.
        force: bool,
    },
    #[doc = "Define an escaped special variable."] DefineSpecialExact {
        /// Escaped variable name.
        name: String,
        /// Whether to force special binding semantics.
        force: bool,
    },
    #[doc = "Define multiple values."] DefineValues(String),
    #[doc = "Define multiple values using escaped names."] DefineValuesExact(String),
    #[doc = "Set a variable."] Set(String),
    #[doc = "Set an escaped variable."] SetExact(String),
    #[doc = "Perform a `SETF` update."] Setf(Form),
    #[doc = "Update a list-valued symbol through CAR or CDR."] SetfList {
        /// The list accessor name.
        operator: String,
        /// The symbol holding the list.
        name: String,
        /// Whether the symbol name is escaped.
        escaped: bool,
    },
    #[doc = "Update an indexed element of a list-valued symbol through NTH."] SetfNth {
        /// Zero-based list index.
        index: usize,
        /// The symbol holding the list.
        name: String,
        /// Whether the symbol name is escaped.
        escaped: bool,
    },
    #[doc = "Update a dynamically indexed element of a list-valued symbol through NTH."] SetfNthDynamic {
        /// The symbol holding the list.
        name: String,
        /// Whether the symbol name is escaped.
        escaped: bool,
    },
    #[doc = "Update a dynamically indexed vector or array-valued symbol through an array accessor."] SetfArefDynamic {
        /// The number of subscripts.
        rank: usize,
        /// The accessor name.
        operator: String,
        /// The symbol holding the array.
        name: String,
        /// Whether the symbol name is escaped.
        escaped: bool,
    },
    #[doc = "Update a dynamically indexed bit vector or array-valued symbol through BIT."] SetfBitDynamic {
        /// The number of subscripts.
        rank: usize,
        /// The symbol holding the bit array.
        name: String,
        /// Whether the symbol name is escaped.
        escaped: bool,
    },
    #[doc = "Update a dynamically indexed sequence or string-valued symbol through an element accessor."] SetfElementDynamic {
        operator: String,
        name: String,
        escaped: bool,
    },
    #[doc = "Update a dynamically bounded subsequence-valued symbol through SUBSEQ."] SetfSubseqDynamic {
        /// Whether an explicit end bound is present.
        has_end: bool,
        /// The symbol holding the sequence.
        name: String,
        /// Whether the symbol name is escaped.
        escaped: bool,
    },
    #[doc = "Update a property-list-valued symbol through GETF."] SetfGetfDynamic {
        /// The symbol holding the property list.
        name: String,
        /// Whether the symbol name is escaped.
        escaped: bool,
    },
    #[doc = "Update a symbol property through GET."] SetfGetDynamic,
    #[doc = "Update a hash-table-valued place through GETHASH."] SetfGethashDynamic,
    #[doc = "Push a value onto a list-valued symbol."] PushList {
        /// The symbol holding the list.
        name: String,
        /// Whether the symbol name is escaped.
        escaped: bool,
    },
    #[doc = "Push a value onto a list-valued symbol when absent by EQL."] PushNewList {
        /// The symbol holding the list.
        name: String,
        /// Whether the symbol name is escaped.
        escaped: bool,
    },
    #[doc = "Pop the first value from a list-valued symbol."] PopList {
        /// The symbol holding the list.
        name: String,
        /// Whether the symbol name is escaped.
        escaped: bool,
    },
    #[doc = "Update a symbol with the result of `MAP-INTO`."] MapIntoSetfSymbol {
        /// The symbol receiving the mapped sequence.
        name: String,
        /// Whether the symbol name is escaped.
        escaped: bool,
    },
    #[doc = "Perform a place update with `MAP-INTO` semantics."] MapIntoSetf(Form),
    #[doc = "Perform parallel assignment."] Psetq(Vec<String>),
    #[doc = "Perform escaped parallel assignment."] PsetqExact(Vec<(String, bool)>),
    #[doc = "Bind multiple-value assignment targets."] MultipleValueSetq(Vec<String>),
    #[doc = "Bind escaped multiple-value assignment targets."] MultipleValueSetqExact(Vec<(String, bool)>),
    #[doc = "Enter a lexical scope."] EnterScope,
    #[doc = "Exit a lexical scope."] ExitScope,
    #[doc = "Discard the top stack value."] Pop,
    #[doc = "Duplicate the top stack value."] Dup,
    #[doc = "Replace the stack with the primary value."] Primary,
    #[doc = "Construct a multiple-value carrier."] Values(usize),
    #[doc = "Convert a multiple-value carrier to a list."] MultipleValueList,
    #[doc = "Bind multiple values to names."] BindValues(Vec<String>),
    #[doc = "Bind multiple values to escaped names."] BindValuesExact(Vec<(String, bool)>),
    #[doc = "Destructure a value."] Destructure(DestructureSpec),
    #[doc = "Branch when the top value is false."] JumpIfFalse(usize),
    #[doc = "Unconditional branch."] Jump(usize),
    #[doc = "Create a closure for a nested function."] MakeClosure(FunctionId),
    #[doc = "Evaluate a function while ignoring conditions."] IgnoreErrors(FunctionId),
    #[doc = "Run a body with condition handlers selected by type."] HandlerCase {
        /// Protected function.
        protected: FunctionId,
        /// Handler clauses.
        clauses: Vec<HandlerCaseClause>,
    },
    #[doc = "Install dynamically scoped handlers around a body."] HandlerBind {
        /// Body function.
        body: FunctionId,
        /// Handler clauses.
        handlers: Vec<HandlerBindClause>,
    },
    #[doc = "Install dynamically scoped restarts around a body."] RestartBind {
        /// Body function.
        body: FunctionId,
        /// Restart bindings.
        bindings: Vec<RestartBindClause>,
    },
    #[doc = "Catch a matching tag from a body."] Catch {
        /// Tag-producing function.
        tag: FunctionId,
        /// Body function.
        body: FunctionId,
    },
    #[doc = "Establish a simple restart around a body."] WithSimpleRestart {
        /// Restart name.
        name: String,
        /// Body function.
        body: FunctionId,
    },
    #[doc = "Establish restarts associated with a condition."] WithConditionRestarts {
        /// Condition function.
        condition: FunctionId,
        /// Restart list function.
        restarts: FunctionId,
        /// Body function.
        body: FunctionId,
    },
    #[doc = "Run a body with restart-case clauses."] RestartCase {
        /// Protected function.
        protected: FunctionId,
        /// Restart clauses.
        clauses: Vec<RestartCaseClause>,
    },
    #[doc = "Bind a dynamic set of special variables around a body."] Progv {
        /// Symbols function.
        symbols: FunctionId,
        /// Values function.
        values: FunctionId,
        /// Body function.
        body: FunctionId,
    },
    #[doc = "Throw the current tag and values."] Throw,
    #[doc = "Establish a named non-local return target."] Block {
        /// Body function.
        function: FunctionId,
        /// Block name.
        name: String,
    },
    #[doc = "Establish a tagbody control-flow region."] TagBody {
        /// Body function.
        function: FunctionId,
        /// Tag-to-offset mapping.
        tags: Vec<(String, usize)>,
    },
    #[doc = "Run cleanup even when protected evaluation exits non-locally."] UnwindProtect {
        /// Protected function.
        protected: FunctionId,
        /// Cleanup function.
        cleanup: FunctionId,
    },
    #[doc = "Return from a named block."] ReturnFrom {
        /// Block name.
        name: String,
    },
    #[doc = "Transfer control to a tagbody tag."] Go {
        /// Tag name.
        tag: String,
    },
    #[doc = "Evaluate a compiled source span."] Eval(Span),
    #[doc = "Call a function with positional arguments."] Call(usize),
    #[doc = "Apply a final list of arguments."] Apply(usize),
    #[doc = "Map a function over one or more sequences."] MapCar(usize),
    #[doc = "Call a function with multiple-value arguments."] MultipleValueCall(usize),
    #[doc = "Return from the current function."] Return,
}
