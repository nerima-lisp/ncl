use crate::{
    Constant, DestructureSpec, FunctionId, HandlerBindClause, HandlerCaseClause, RestartBindClause,
    RestartCaseClause, RotateShiftPlace,
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
    #[doc = "Push a dynamically bound special variable."] DefineDynamicSpecial(String),
    #[doc = "Push an escaped dynamically bound special variable."] DefineDynamicSpecialExact(String),
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
    #[doc = "Update a nested CAR/CDR list place rooted at a symbol."] SetfNestedList {
        /// Accessors from the symbol outward to the updated value.
        accessors: Vec<String>,
        /// The symbol holding the outer list.
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
        /// The sequence element accessor name.
        operator: String,
        /// The symbol holding the sequence.
        name: String,
        /// Whether the symbol name is escaped.
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
    #[doc = "Update a CLOS instance slot through SLOT-VALUE."] SetfSlotValueDynamic,
    #[doc = "Update a dynamically selected symbol value or function cell."] SetfSymbolCellDynamic {
        /// The symbol-cell accessor name.
        operator: String,
    },
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
    #[doc = "Push a value onto a list-valued symbol with PUSHNEW comparison options."] PushNewListOptions {
        /// The symbol holding the list.
        name: String,
        /// Whether the symbol name is escaped.
        escaped: bool,
        /// Whether the comparison function came from :TEST-NOT.
        test_not: bool,
        /// Whether a key function value is present on the stack.
        has_key: bool,
        /// Whether the source form evaluates :KEY before the test designator.
        key_before_test: bool,
    },
    #[doc = "Pop the first value from a list-valued symbol."] PopList {
        /// The symbol holding the list.
        name: String,
        /// Whether the symbol name is escaped.
        escaped: bool,
    },
    #[doc = "Push a value onto a list-valued GETHASH place."] PushGethash,
    #[doc = "Push a value onto a list-valued GETHASH place when absent by EQL."] PushNewGethash,
    #[doc = "Push a value onto a list-valued GETHASH place with PUSHNEW comparison options."] PushNewGethashOptions {
        /// Whether the comparison function came from :TEST-NOT.
        test_not: bool,
        /// Whether a key function value is present on the stack.
        has_key: bool,
        /// Whether the source form evaluates :KEY before the test designator.
        key_before_test: bool,
    },
    #[doc = "Pop the first value from a list-valued GETHASH place."] PopGethash,
    #[doc = "Push onto or pop from a CAR/CDR list place held by a symbol."] ListPlaceMutation {
        /// The mutation operator.
        operator: String,
        /// The list accessor name.
        accessor: String,
        /// The symbol holding the outer list.
        name: String,
        /// Whether the symbol name is escaped.
        escaped: bool,
    },
    #[doc = "Push onto or pop from a nested CAR/CDR list place rooted at a symbol."] NestedListPlaceMutation {
        /// Accessors from the symbol outward to the mutated list.
        accessors: Vec<String>,
        /// The mutation operator.
        operator: String,
        /// The symbol holding the outer list.
        name: String,
        /// Whether the symbol name is escaped.
        escaped: bool,
    },
    #[doc = "Pushnew onto a CAR/CDR list place with comparison options."] ListPlacePushNewOptions {
        /// The list accessor name.
        accessor: String,
        /// The symbol holding the outer list.
        name: String,
        /// Whether the symbol name is escaped.
        escaped: bool,
        /// Whether the comparison function came from :TEST-NOT.
        test_not: bool,
        /// Whether a key function value is present on the stack.
        has_key: bool,
        /// Whether the source form evaluates :KEY before the test designator.
        key_before_test: bool,
    },
    #[doc = "Pushnew onto a nested CAR/CDR list place with comparison options."] NestedListPlacePushNewOptions {
        /// Accessors from the symbol outward to the mutated list.
        accessors: Vec<String>,
        /// The symbol holding the outer list.
        name: String,
        /// Whether the symbol name is escaped.
        escaped: bool,
        /// Whether the comparison function came from :TEST-NOT.
        test_not: bool,
        /// Whether a key function value is present on the stack.
        has_key: bool,
        /// Whether the source form evaluates :KEY before the test designator.
        key_before_test: bool,
    },
    #[doc = "Rotate values among symbol places."] RotatefSymbols(Vec<(String, bool)>),
    #[doc = "Shift values through symbol places and return the first old value."] ShiftfSymbols(Vec<(String, bool)>),
    #[doc = "Rotate values among nested CAR/CDR list places."] RotatefNestedList(Vec<(Vec<String>, String, bool)>),
    #[doc = "Shift values through nested CAR/CDR list places."] ShiftfNestedList(Vec<(Vec<String>, String, bool)>),
    #[doc = "Rotate values among symbol and nested CAR/CDR list places."] RotatefMixed(Vec<RotateShiftPlace>),
    #[doc = "Shift values through symbol and nested CAR/CDR list places."] ShiftfMixed(Vec<RotateShiftPlace>),
    #[doc = "Execute a mutation special form through the runtime's direct implementation."] RuntimeMutation(Form),
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
    #[doc = "Select one value from a multiple-value carrier."] NthValue,
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
    #[doc = "Define a structure through the runtime structure registry."] Defstruct(Form),
    #[doc = "Define a class through the runtime class registry."] Defclass(Form),
    #[doc = "Define a generic function through the runtime method registry."] Defgeneric(Form),
    #[doc = "Define a method through the runtime method registry."] Defmethod(Form),
    #[doc = "Define a SETF function through the runtime place registry."] Defsetf(Form),
    #[doc = "Define a constant through the runtime constant registry."] Defconstant(Form),
    #[doc = "Define a symbol macro through the runtime macro registry."] DefineSymbolMacro(Form),
    #[doc = "Define a modifying macro through the runtime macro registry."] DefineModifyMacro(Form),
    #[doc = "Define a SETF expander through the runtime macro registry."] DefineSetfExpander(Form),
    #[doc = "Compute a generalized-place expansion through the runtime SETF registry."] GetSetfExpansion(Form),
    #[doc = "Perform parallel generalized-place assignments through the runtime SETF machinery."] Psetf(Form),
    #[doc = "Perform parallel assignment to symbol places after evaluating all values."] PsetfSymbols(Vec<(String, bool)>),
    #[doc = "Evaluate and cache a load-time value through the runtime evaluator."] LoadTimeValue(Form),
    #[doc = "Evaluate a compiled source span."] Eval(Span),
    #[doc = "Call a function with positional arguments."] Call(usize),
    #[doc = "Apply a final list of arguments."] Apply(usize),
    #[doc = "Map a function over one or more lists."] ListMapping {
        /// Mapping operation name.
        operation: String,
        /// Number of list arguments.
        sequence_count: usize,
    },
    #[doc = "Apply a predicate across one or more sequences."] SequenceQuantifier {
        /// The quantifier operation.
        operation: String,
        /// Number of sequences consumed after the predicate.
        sequence_count: usize,
    },
    #[doc = "Map a function over sequences into a requested result type."] SequenceMapping {
        /// Number of sequences consumed after the function.
        sequence_count: usize,
    },
    #[doc = "Map a function into a destination sequence."] SequenceMapInto {
        /// Number of source sequences consumed after the function and destination.
        sequence_count: usize,
    },
    #[doc = "Reduce a sequence with a function and keyword options."] SequenceReduce {
        /// Number of option values following the sequence.
        option_count: usize,
    },
    #[doc = "Merge two sequences with a predicate and keyword options."] SequenceMerge {
        /// Number of option values following the predicate.
        option_count: usize,
    },
    #[doc = "Sort a sequence with a predicate and keyword options."] SequenceSort {
        /// Sorting operation name.
        operation: String,
        /// Number of option values following the predicate.
        option_count: usize,
    },
    #[doc = "Search a sequence with a value or predicate and keyword options."] SequenceSearch {
        /// Search operation name.
        operation: String,
        /// Whether the first argument is a predicate.
        predicate: bool,
        /// Number of option values following the sequence arguments.
        option_count: usize,
    },
    #[doc = "Search two sequences with keyword options."] SequencePairSearch {
        /// Search operation name.
        operation: String,
        /// Number of option values following the sequences.
        option_count: usize,
    },
    #[doc = "Search or update a list with membership options."] ListMembership {
        /// Membership operation name.
        operation: String,
        /// Whether the first argument is a predicate.
        predicate: bool,
        /// Number of option values following the list.
        option_count: usize,
    },
    #[doc = "Search an association list with keyword options."] AssociationSearch {
        /// Association search operation name.
        operation: String,
        /// Whether the first argument is a predicate.
        predicate: bool,
        /// Number of option values following the alist.
        option_count: usize,
    },
    #[doc = "Remove matching elements from a sequence with keyword options."] SequenceRemoval {
        /// Sequence removal operation name.
        operation: String,
        /// Whether the first argument is a predicate.
        predicate: bool,
        /// Whether matching duplicate elements are removed.
        duplicates: bool,
        /// Number of option values following the sequence.
        option_count: usize,
    },
    #[doc = "Substitute matching elements in a sequence with keyword options."] SequenceSubstitution {
        /// Sequence substitution operation name.
        operation: String,
        /// Whether the old item argument is a predicate.
        predicate: bool,
        /// Number of option values following the sequence.
        option_count: usize,
    },
    #[doc = "Apply a unary tree or sequence operation."] SequenceUnary {
        /// Unary operation name.
        operation: String,
    },
    #[doc = "Apply a unary list access operation."] ListUnary {
        /// Unary list operation name.
        operation: String,
    },
    #[doc = "Apply a unary character conversion operation."] CharacterUnary {
        /// Unary operation name.
        operation: String,
    },
    #[doc = "Apply a unary type predicate operation."] TypePredicate {
        /// Type predicate name.
        operation: String,
    },
    #[doc = "Compare two values with a Common Lisp equality predicate."] Equality {
        /// Equality predicate name.
        operation: String,
    },
    #[doc = "Apply a unary numeric operation."] NumericUnary {
        /// Unary operation name.
        operation: String,
    },
    #[doc = "Compare numeric arguments."] NumericComparison {
        /// Comparison operation name.
        operation: String,
        /// Number of arguments to compare.
        argument_count: usize,
    },
    #[doc = "Apply a variadic numeric operation."] NumericFold {
        /// Numeric operation name.
        operation: String,
        /// Number of arguments supplied to the operation.
        argument_count: usize,
    },
    #[doc = "Apply a binary numeric operation."] NumericBinary {
        /// Numeric operation name.
        operation: String,
    },
    #[doc = "Apply the three-argument BOOLE operation."] NumericBoole,
    #[doc = "Apply a bitfield operation."] NumericBitfield {
        /// Bitfield operation name.
        operation: String,
        /// Number of arguments supplied to the operation.
        argument_count: usize,
    },
    #[doc = "Apply a floating-point operation."] NumericFloat {
        /// Floating-point operation name.
        operation: String,
        /// Number of arguments supplied to the operation.
        argument_count: usize,
    },
    #[doc = "Apply a list tail operation with an optional count."] ListTail {
        /// List operation name.
        operation: String,
        /// Number of optional values following the list.
        option_count: usize,
    },
    #[doc = "Apply a binary list operation."] ListBinary {
        /// List operation name.
        operation: String,
    },
    #[doc = "Compare trees with keyword options."] TreeEqual {
        /// Number of option values following the two trees.
        option_count: usize,
    },
    #[doc = "Return the length of a sequence."] SequenceLength,
    #[doc = "Return an element from a sequence."] SequenceElement,
    #[doc = "Return a subsequence with optional end bound."] SequenceSubseq {
        /// Number of arguments consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Apply a sequence mutation operation with keyword options."] SequenceMutation {
        /// Sequence mutation operation name.
        operation: String,
        /// Number of arguments consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Concatenate one or more sequences into a requested result type."] SequenceConcatenate {
        /// Number of arguments, including the result type.
        argument_count: usize,
    },
    #[doc = "Convert or construct a sequence value."] SequenceConversion {
        /// Sequence conversion operation name.
        operation: String,
        /// Number of arguments consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Construct a vector from evaluated arguments."] VectorConstruction {
        /// Number of arguments consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Construct a list from evaluated arguments."] ListConstruction {
        /// Number of arguments consumed from the stack.
        argument_count: usize,
        /// Whether the final argument is the tail of the list.
        dotted: bool,
    },
    #[doc = "Append evaluated lists using a list operation."] ListAppend {
        /// Name of the list operation to invoke.
        operation: String,
        /// Number of arguments consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Apply a property-list operation."] PropertyList {
        /// Name of the property-list operation to invoke.
        operation: String,
        /// Number of arguments consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Apply a symbol value operation."] SymbolValue {
        /// Name of the symbol value operation to invoke.
        operation: String,
        /// Number of arguments consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Apply a symbol binding operation."] SymbolBinding {
        /// Name of the symbol binding operation to invoke.
        operation: String,
        /// Number of arguments consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Apply a symbol function operation."] SymbolFunction {
        /// Name of the symbol function operation to invoke.
        operation: String,
        /// Number of arguments consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Apply a symbol creation operation."] SymbolCreation {
        /// Name of the symbol creation operation to invoke.
        operation: String,
        /// Number of arguments consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Apply a CLOS class introspection operation."] ClassIntrospection {
        /// Name of the class introspection operation to invoke.
        operation: String,
        /// Number of arguments consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Apply a CLOS slot operation."] SlotOperation {
        /// Name of the slot operation to invoke.
        operation: String,
        /// Number of arguments consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Apply a condition-system operation."] ConditionOperation {
        /// Name of the condition operation to invoke.
        operation: String,
        /// Number of arguments consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Apply a restart operation."] RestartOperation {
        /// Name of the restart operation to invoke.
        operation: String,
        /// Number of arguments consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Apply a method-context operation."] MethodOperation {
        /// Name of the method operation to invoke.
        operation: String,
        /// Number of arguments consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Apply an evaluation operation."] EvaluationOperation {
        /// Name of the evaluation operation to invoke.
        operation: String,
        /// Number of arguments consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Apply a package introspection operation."] PackageIntrospection {
        /// Name of the package introspection operation to invoke.
        operation: String,
        /// Number of arguments consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Apply a package mutation operation."] PackageMutation {
        /// Name of the package mutation operation to invoke.
        operation: String,
        /// Number of arguments consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Apply a package listing operation."] PackageListing {
        /// Name of the package listing operation to invoke.
        operation: String,
        /// Number of arguments consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Apply a hash-table operation."] HashTable {
        /// Name of the hash-table operation to invoke.
        operation: String,
        /// Number of arguments consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Construct an array from evaluated arguments."] ArrayConstruction {
        /// Number of arguments consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Adjust an array from evaluated arguments."] ArrayAdjustment {
        /// Number of arguments consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Construct a list with MAKE-LIST keyword options."] ListConstructionWithOptions {
        /// Number of arguments consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Apply a string case transformation with optional bounds."] StringCase {
        /// String case operation name.
        operation: String,
        /// Number of arguments consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Compare two strings."] StringComparison {
        /// String comparison operation name.
        operation: String,
    },
    #[doc = "Trim characters from a string."] StringTrim {
        /// String trimming operation name.
        operation: String,
    },
    #[doc = "Construct or designate a string."] StringConstruction {
        /// String construction operation name.
        operation: String,
        /// Number of arguments consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Compare two or more characters."] CharacterComparison {
        /// Character comparison operation name.
        operation: String,
        /// Number of characters consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Return a character from a string."] CharacterElement {
        /// Character access operation name.
        operation: String,
    },
    #[doc = "Test whether a character is a digit in an optional radix."] CharacterDigitPredicate {
        /// Number of values consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Return an element from an array or vector."] ArrayElement {
        /// Array access operation name.
        operation: String,
        /// Number of values consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Return metadata about an array."] ArrayMetadata {
        /// Array metadata operation name.
        operation: String,
        /// Number of values consumed from the stack.
        argument_count: usize,
    },
    #[doc = "Apply a list set operation with keyword options."] ListSet {
        /// List set operation name.
        operation: String,
        /// Number of option values following the two lists.
        option_count: usize,
    },
    #[doc = "Call a function with multiple-value arguments."] MultipleValueCall(usize),
    #[doc = "Return from the current function."] Return,
}
