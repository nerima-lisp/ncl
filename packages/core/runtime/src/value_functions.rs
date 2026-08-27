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
#[allow(missing_docs)]
pub enum Function {
    /// A primitive implemented as a Rust function.
    Builtin {
        name: &'static str,
        function: Builtin,
    },
    /// A named primitive resolved by the evaluator.
    Primitive { name: &'static str },
    /// A constructor for a structure type.
    StructureConstructor {
        name: String,
        slots: Vec<super::StructureSlot>,
        structure_types: Vec<String>,
        constructor_lambda_list: Option<OrdinaryLambdaList>,
        environment: Environment,
    },
    /// A predicate for a structure type.
    StructurePredicate { name: String },
    /// An accessor for a structure slot.
    StructureAccessor {
        structure_name: String,
        slot_name: String,
        slot_index: usize,
        read_only: bool,
    },
    /// A copier for a structure value.
    StructureCopier { name: String },
    /// A generic function with an extensible method set.
    Generic {
        name: String,
        methods: Rc<RefCell<Vec<MethodDefinition>>>,
    },
    /// A reader method for a class slot.
    SlotReader {
        class_name: String,
        slot_name: String,
    },
    /// A writer method for a class slot.
    SlotWriter {
        class_name: String,
        slot_name: String,
    },
    /// A reader method for a condition slot.
    ConditionReader {
        condition_name: String,
        slot_name: String,
    },
    /// A writer method for a condition slot.
    ConditionWriter {
        condition_name: String,
        slot_name: String,
    },
    /// A lexical closure.
    Closure {
        parameters: Vec<String>,
        required_escaped: Vec<bool>,
        optional: Vec<LambdaListOptionalParameter>,
        rest: Option<String>,
        rest_escaped: bool,
        keywords: Vec<LambdaListKeywordParameter>,
        has_keyword_section: bool,
        allow_other_keys: bool,
        auxiliary: Vec<LambdaListAuxiliaryParameter>,
        body: Vec<Form>,
        environment: Environment,
    },
    /// A macro closure.
    Macro {
        lambda_list: MacroLambdaList,
        body: Vec<Form>,
        environment: Environment,
    },
    /// A modifying macro closure.
    ModifyMacro {
        lambda_list: MacroLambdaList,
        function: Form,
        environment: Environment,
    },
    /// A function compiled into bytecode.
    Compiled {
        program: Rc<Program>,
        function: FunctionId,
        environment: Environment,
    },
}
