use std::cell::RefCell;
use std::rc::Rc;

use ncl_compiler::{FunctionId, Program};
use ncl_syntax::{
    Form, LambdaListAuxiliaryParameter, LambdaListKeywordParameter, LambdaListOptionalParameter,
    OrdinaryLambdaList,
};

use super::{Environment, MacroLambdaList, MethodDefinition, RuntimeError, Value};

/// Function pointer used by a registered runtime primitive.
pub type Builtin = fn(&[Value]) -> Result<Value, RuntimeError>;

#[derive(Clone, Debug)]
/// A callable value implemented by the runtime or compiled from NCL.
pub enum Function {
    /// A primitive implemented as a Rust function.
    Builtin {
        /// The primitive's canonical name.
        name: &'static str,
        /// The Rust function implementing the primitive.
        function: Builtin,
    },
    /// A named primitive resolved by the evaluator.
    Primitive {
        /// The primitive's canonical name.
        name: &'static str,
    },
    /// A constructor for a structure type.
    StructureConstructor {
        /// The structure type name.
        name: String,
        /// Metadata for the structure's slots.
        slots: Vec<super::StructureSlot>,
        /// Structure type names accepted by the constructor.
        structure_types: Vec<String>,
        /// The constructor's lambda list, when available.
        constructor_lambda_list: Option<OrdinaryLambdaList>,
        /// The environment captured by the constructor.
        environment: Environment,
    },
    /// A predicate for a structure type.
    StructurePredicate {
        /// The structure type name.
        name: String,
    },
    /// An accessor for a structure slot.
    StructureAccessor {
        /// The structure type name.
        structure_name: String,
        /// The slot name.
        slot_name: String,
        /// The zero-based slot index.
        slot_index: usize,
        /// Whether the slot cannot be modified.
        read_only: bool,
    },
    /// A copier for a structure value.
    StructureCopier {
        /// The structure type name.
        name: String,
    },
    /// A generic function with an extensible method set.
    Generic {
        /// The generic function name.
        name: String,
        /// The methods currently registered on the generic function.
        methods: Rc<RefCell<Vec<MethodDefinition>>>,
    },
    /// A reader method for a class slot.
    SlotReader {
        /// The class name.
        class_name: String,
        /// The slot name.
        slot_name: String,
    },
    /// A writer method for a class slot.
    SlotWriter {
        /// The class name.
        class_name: String,
        /// The slot name.
        slot_name: String,
    },
    /// A reader method for a condition slot.
    ConditionReader {
        /// The condition name.
        condition_name: String,
        /// The slot name.
        slot_name: String,
    },
    /// A writer method for a condition slot.
    ConditionWriter {
        /// The condition name.
        condition_name: String,
        /// The slot name.
        slot_name: String,
    },
    /// A lexical closure.
    Closure {
        /// Required parameter names.
        parameters: Vec<String>,
        /// Whether each required parameter escapes its lexical scope.
        required_escaped: Vec<bool>,
        /// Optional parameters and their defaults.
        optional: Vec<LambdaListOptionalParameter>,
        /// The rest parameter, if present.
        rest: Option<String>,
        /// Whether the rest parameter escapes its lexical scope.
        rest_escaped: bool,
        /// Keyword parameters and their defaults.
        keywords: Vec<LambdaListKeywordParameter>,
        /// Whether a keyword section was explicitly present.
        has_keyword_section: bool,
        /// Whether unknown keywords are accepted.
        allow_other_keys: bool,
        /// Auxiliary bindings initialized on entry.
        auxiliary: Vec<LambdaListAuxiliaryParameter>,
        /// Function body forms.
        body: Vec<Form>,
        /// Environment captured by the closure.
        environment: Environment,
    },
    /// A macro closure.
    Macro {
        /// Macro parameter specification.
        lambda_list: MacroLambdaList,
        /// Macro expansion body.
        body: Vec<Form>,
        /// Environment captured by the macro.
        environment: Environment,
    },
    /// A modifying macro closure.
    ModifyMacro {
        /// Macro parameter specification.
        lambda_list: MacroLambdaList,
        /// Place form transformed by the modifying macro.
        function: Form,
        /// Environment captured by the modifying macro.
        environment: Environment,
    },
    /// A function compiled into bytecode.
    Compiled {
        /// Compiled bytecode program.
        program: Rc<Program>,
        /// Entry function within the program.
        function: FunctionId,
        /// Environment captured by the compiled function.
        environment: Environment,
    },
}
