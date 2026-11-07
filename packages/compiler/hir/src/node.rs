//! Semantic HIR nodes that retain Lisp-level identity and control semantics.

use crate::arena::Arena;
use crate::binding::{LocalFunctionKind, ScopeId};
use crate::control::ExitTarget;
use crate::ids::{
    BindingId, BlockId, CleanupId, NodeId, SourceOriginId, SymbolId, TypeId, TypeSpecConstId,
    ValueId,
};

/// Storage for nodes in one top-level form.
pub type NodeArena = Arena<Node, NodeId>;

/// One semantic operation and its source origin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Node {
    /// The lexical scope in which this operation is evaluated.
    pub scope: ScopeId,
    /// The source or expansion origin that produced this node.
    pub origin: SourceOriginId,
    /// The operation represented by this node.
    pub kind: NodeKind,
}

/// A Lisp operation before machine representation selection.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum NodeKind {
    /// A constant datum.
    Literal(Literal),
    /// A variable reference that retains its lexical, special, or global identity.
    VariableReference {
        /// The lexical binding or symbol identity for this reference.
        target: VariableTarget,
    },
    /// A function value that keeps local, global, and intrinsic identity distinct.
    FunctionReference {
        /// The local, global, intrinsic, or computed function target.
        target: CallTarget,
    },
    /// A closure value for a lambda or local function body.
    ClosureReference {
        /// The HIR node identifying the function body.
        function: NodeId,
    },
    /// An assignment to a resolved variable binding.
    Assignment {
        /// The lexical binding or symbol identity being assigned.
        target: VariableTarget,
        /// The assigned value form.
        value: NodeId,
    },
    /// A parallel or sequential lexical binding form.
    Let {
        /// Whether initializers are evaluated sequentially.
        kind: LetKind,
        /// The bindings and their optional initializer forms.
        bindings: Vec<LetBinding>,
        /// Scope used for each initializer, aligned with `bindings`.
        initializer_scopes: Vec<ScopeId>,
        /// Scope after each binding, aligned with `bindings`.
        binding_scopes: Vec<ScopeId>,
        /// Scope visible to the body forms.
        body_scope: ScopeId,
        /// The body forms.
        body: Vec<NodeId>,
    },
    /// A PROGV form that dynamically binds symbol and value sequences.
    DynamicBinding {
        /// The form producing the symbols to bind.
        symbols: NodeId,
        /// The form producing the values to bind.
        values: NodeId,
        /// The body forms evaluated under the dynamic bindings.
        body: Vec<NodeId>,
    },
    /// An anonymous function.
    Lambda {
        /// The lexical parameters.
        parameters: Vec<BindingId>,
        /// Scope containing the parameters and body.
        body_scope: ScopeId,
        /// The function body forms.
        body: Vec<NodeId>,
    },
    /// Local function definitions with their FLET or LABELS scope rule.
    LocalFunctions {
        /// Whether definitions are mutually recursive.
        kind: LocalFunctionKind,
        /// Scope in which the group body sees all local function names.
        body_scope: ScopeId,
        /// The local function bodies.
        definitions: Vec<LocalFunctionDefinition>,
        /// The body where all local function names are visible.
        body: Vec<NodeId>,
    },
    /// A call whose operator identity is explicit.
    Call {
        /// The local, global, intrinsic, or computed function target.
        target: CallTarget,
        /// Arguments in source order.
        arguments: Vec<NodeId>,
    },
    /// A conditional expression.
    If {
        /// The test form.
        test: NodeId,
        /// The true branch.
        then_branch: NodeId,
        /// The false branch, absent when it produces NIL.
        else_branch: Option<NodeId>,
    },
    /// A named non-local return target.
    Block {
        /// The interned block name.
        name: SymbolId,
        /// The body forms.
        body: Vec<NodeId>,
    },
    /// A return from the block identified by its node.
    ReturnFrom {
        /// The target block node.
        target: NodeId,
        /// The returned form, absent when it produces NIL.
        value: Option<NodeId>,
    },
    /// A TAGBODY whose tags identify local CFG blocks.
    Tagbody {
        /// The tag names and their corresponding CFG blocks.
        tags: Vec<TagbodyTag>,
        /// The forms between tags, in source order.
        forms: Vec<NodeId>,
    },
    /// A GO to a local TAGBODY block.
    Go {
        /// The target TAGBODY block.
        target: BlockId,
    },
    /// A dynamically tagged exit region.
    Catch {
        /// The runtime tag expression.
        tag: NodeId,
        /// The body forms.
        body: Vec<NodeId>,
    },
    /// A dynamically resolved THROW.
    Throw {
        /// The runtime catch tag.
        tag: NodeId,
        /// The thrown value.
        value: NodeId,
    },
    /// A protected form and its cleanup sequence.
    UnwindProtect {
        /// The cleanup metadata identity.
        cleanup: CleanupId,
        /// The protected form.
        protected: NodeId,
        /// Cleanup forms in source order.
        cleanup_forms: Vec<NodeId>,
    },
    /// An exit whose target and applicable cleanups remain explicit.
    NonLocalExit {
        /// The semantic exit target.
        target: ExitTarget,
        /// The values carried by this exit, in order.
        values: Vec<NodeId>,
        /// Cleanup records to run before transferring control.
        cleanups: Vec<CleanupId>,
    },
    /// An operation that produces, binds, returns, or forwards multiple values.
    MultipleValues(MultipleValueOperation),
    /// A source-level type assertion.
    TypeAssertion {
        /// The asserted value.
        value: NodeId,
        /// The asserted Common Lisp type.
        asserted_type: TypeId,
    },
    /// A recognized operation that still retains its function identity.
    KnownFunctionOperation {
        /// The known function name.
        function: SymbolId,
        /// Arguments in source order.
        arguments: Vec<NodeId>,
    },
    /// A generic sequence operation whose operator remains identifiable.
    SequenceOperation {
        /// The sequence function name.
        function: SymbolId,
        /// Arguments in source order.
        arguments: Vec<NodeId>,
    },
    /// A generic array operation whose operator remains identifiable.
    ArrayOperation {
        /// The array function name.
        function: SymbolId,
        /// Arguments in source order.
        arguments: Vec<NodeId>,
    },
    /// An explicit runtime check inserted by a transform.
    RuntimeCheck {
        /// The check being performed.
        kind: RuntimeCheckKind,
        /// The value checked.
        value: NodeId,
    },
    /// A type check with the complete source type form retained for diagnostics.
    CheckType {
        /// The SSA value checked.
        value: ValueId,
        /// The HIR type used to prove that the check is redundant.
        accepted_type: TypeId,
        /// The full type-specifier constant passed to the runtime condition.
        expected_type: TypeSpecConstId,
        /// The source origin used to report a failed check.
        source_origin: SourceOriginId,
    },
    /// A top-level definition retained by the compilation unit.
    TopLevelDefinition {
        /// The defined name.
        name: SymbolId,
        /// The definition namespace.
        kind: DefinitionKind,
        /// The definition value or body.
        value: NodeId,
    },
    /// A form scheduled for compile-time, load-time, or execution-time processing.
    CompileTimeForm {
        /// The situations in which the form is active.
        situations: Vec<CompileTimeSituation>,
        /// The contained form.
        form: NodeId,
    },
}

/// A literal datum represented without runtime heap pointers.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Literal {
    /// The empty list.
    Nil,
    /// The canonical true symbol.
    True,
    /// A symbol reference.
    Symbol(SymbolId),
    /// A numeric datum.
    Number(NumberLiteral),
    /// A Unicode scalar value stored as its code point.
    Character(u32),
    /// A string as Unicode scalar values.
    String(Vec<char>),
    /// A pathname namestring as Unicode scalar values.
    Pathname(Vec<char>),
    /// A cons datum.
    Cons(Box<Self>, Box<Self>),
    /// A vector datum.
    Vector(Vec<Self>),
    /// A general array datum in row-major order.
    Array {
        /// The identity used to preserve sharing of this source literal object.
        identity: u32,
        /// The extent of each axis, as returned by ARRAY-DIMENSIONS.
        dimensions: Vec<usize>,
        /// The array's storage element representation.
        element_type: LiteralElementType,
        /// Row-major elements.
        elements: Vec<Self>,
    },
    /// A rank-one specialized simple array; its element count is its length.
    SpecializedArray {
        /// The identity used to preserve sharing of this source literal object.
        identity: u32,
        /// The array's storage element representation.
        element_type: LiteralElementType,
        /// Row-major elements.
        elements: Vec<Self>,
    },
}

/// Numeric literal forms supported by the front-end datum model.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum NumberLiteral {
    /// A fixnum.
    Fixnum(i64),
    /// A bignum in little-endian 32-bit limbs.
    Bignum {
        /// Whether the integer is negative.
        negative: bool,
        /// Little-endian 32-bit limbs.
        limbs: Vec<u32>,
    },
    /// A ratio of numeric values.
    Ratio {
        /// The numerator.
        numerator: Box<Self>,
        /// The denominator.
        denominator: Box<Self>,
    },
    /// A single-precision float's IEEE 754 bit pattern.
    SingleFloat(u32),
    /// A double-precision float's IEEE 754 bit pattern.
    DoubleFloat(u64),
    /// A complex number.
    Complex {
        /// The real part.
        real: Box<Self>,
        /// The imaginary part.
        imaginary: Box<Self>,
    },
}

/// Storage representation for literal array elements.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum LiteralElementType {
    /// General Lisp objects.
    T,
    /// One-bit values.
    Bit,
    /// Characters.
    Character,
    /// Base characters.
    BaseChar,
    /// Fixnums.
    Fixnum,
    /// Signed integers.
    Signed,
    /// Unsigned integers.
    Unsigned,
    /// Single-precision floats.
    SingleFloat,
    /// Double-precision floats.
    DoubleFloat,
}

/// The target identity of a function call or function reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CallTarget {
    /// A function with a lexical FLET or LABELS binding.
    Local(BindingId),
    /// A re-definable global function name.
    Global(SymbolId),
    /// A compiler-known intrinsic kept distinct from ordinary global lookup.
    Intrinsic(SymbolId),
    /// A value computed by another HIR node.
    Value(NodeId),
}

/// The identity used by a variable reference or assignment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum VariableTarget {
    /// A lexical variable binding.
    Lexical(BindingId),
    /// A dynamically scoped special variable.
    Special(SymbolId),
    /// A global variable cell.
    Global(SymbolId),
}

/// A LET or LET* binding and its optional initializer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LetBinding {
    /// The variable binding identity.
    pub binding: BindingId,
    /// The initializer form; None means NIL.
    pub initializer: Option<NodeId>,
}

/// A local function body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalFunctionDefinition {
    /// The local function binding identity.
    pub binding: BindingId,
    /// Scope used to resolve names in this function definition.
    pub definition_scope: ScopeId,
    /// The lexical parameter bindings.
    pub parameters: Vec<BindingId>,
    /// Scope containing the parameters and body.
    pub body_scope: ScopeId,
    /// The body forms.
    pub body: Vec<NodeId>,
}

/// A TAGBODY tag mapped to its control-flow block.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TagbodyTag {
    /// The interned tag name.
    pub name: SymbolId,
    /// The CFG block reached by GO.
    pub block: BlockId,
}

/// The binding form used by a lexical variable group.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum LetKind {
    /// Initializers see the surrounding environment.
    Parallel,
    /// Each initializer sees earlier bindings in the group.
    Sequential,
}

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
}

/// A runtime check introduced by lowering or optimization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RuntimeCheckKind {
    /// Validate an array index.
    Bounds,
    /// Validate an array element representation.
    ArrayElement,
    /// Validate a call's arity.
    Arity,
    /// Validate that a value is callable.
    Function,
}

/// The namespace of a top-level definition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DefinitionKind {
    /// A value-binding definition in the variable namespace.
    Variable,
    /// A function-binding definition in the function namespace.
    Function,
    /// A macro definition used during macro expansion.
    Macro,
    /// A named type definition.
    Type,
}

/// A compile-time processing situation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CompileTimeSituation {
    /// Process while compiling the containing form.
    CompileToplevel,
    /// Process while loading the compiled output.
    LoadToplevel,
    /// Process during ordinary evaluation.
    Execute,
}
