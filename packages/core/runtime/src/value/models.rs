use std::cell::RefCell;
use std::rc::Rc;

use ncl_compiler::{FunctionId, Program};
use ncl_syntax::{
    Form, LambdaListAuxiliaryParameter, LambdaListKeywordParameter, LambdaListOptionalParameter,
    OrdinaryLambdaList,
};

use crate::environment::Environment;

use super::{Builtin, SlotStorage, Value};

pub(crate) struct ClosureData {
    pub(crate) parameters: Vec<String>,
    pub(crate) required_escaped: Vec<bool>,
    pub(crate) optional: Vec<LambdaListOptionalParameter>,
    pub(crate) rest: Option<String>,
    pub(crate) rest_escaped: bool,
    pub(crate) keywords: Vec<LambdaListKeywordParameter>,
    pub(crate) has_keyword_section: bool,
    pub(crate) allow_other_keys: bool,
    pub(crate) auxiliary: Vec<LambdaListAuxiliaryParameter>,
    pub(crate) body: Vec<Form>,
    pub(crate) environment: Environment,
}

#[derive(Clone)]
pub(crate) enum MacroPattern {
    Name(String),
    List(Vec<MacroPattern>),
    LambdaList(MacroLambdaList),
    Dotted {
        items: Vec<MacroPattern>,
        tail: Box<MacroPattern>,
    },
}

#[derive(Clone)]
pub(crate) struct MacroOptionalParameter {
    pub(crate) pattern: MacroPattern,
    pub(crate) init_form: Form,
    pub(crate) supplied_p: Option<String>,
}

#[derive(Clone)]
pub(crate) struct MacroKeywordParameter {
    pub(crate) keyword_name: String,
    pub(crate) pattern: MacroPattern,
    pub(crate) init_form: Form,
    pub(crate) supplied_p: Option<String>,
}

#[derive(Clone)]
pub(crate) struct MacroAuxiliaryParameter {
    pub(crate) name: String,
    pub(crate) init_form: Form,
}

#[derive(Clone)]
pub struct MacroLambdaList {
    pub(crate) whole: Option<String>,
    pub(crate) environment: Option<String>,
    pub(crate) required: Vec<MacroPattern>,
    pub(crate) optional: Vec<MacroOptionalParameter>,
    pub(crate) rest: Option<String>,
    pub(crate) keywords: Vec<MacroKeywordParameter>,
    pub(crate) has_keyword_section: bool,
    pub(crate) allow_other_keys: bool,
    pub(crate) auxiliary: Vec<MacroAuxiliaryParameter>,
}

#[derive(Clone)]
pub struct StructureSlot {
    pub(crate) name: String,
    pub(crate) init_form: Option<Form>,
    pub(crate) read_only: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructureRepresentation {
    Record,
    List { named: bool },
    Vector { named: bool },
}

impl StructureRepresentation {
    pub(crate) fn is_typed(self) -> bool {
        !matches!(self, Self::Record)
    }

    pub(crate) fn is_named(self) -> bool {
        matches!(
            self,
            Self::List { named: true } | Self::Vector { named: true }
        )
    }
}

#[derive(Clone)]
pub(crate) struct StructureDefinition {
    pub(crate) slots: Vec<StructureSlot>,
    pub(crate) type_names: Vec<String>,
    pub(crate) representation: StructureRepresentation,
}

#[derive(Clone)]
pub(crate) struct ConditionSlot {
    pub(crate) name: String,
    pub(crate) initarg: Option<String>,
    pub(crate) init_form: Option<Form>,
    pub(crate) readers: Vec<String>,
    pub(crate) writers: Vec<String>,
}

#[derive(Clone)]
pub(crate) struct ConditionDefinition {
    pub(crate) precedence: Vec<String>,
    pub(crate) slots: Vec<ConditionSlot>,
    pub(crate) report: Option<String>,
}

#[derive(Clone)]
pub(crate) struct ClassSlot {
    pub(crate) name: String,
    pub(crate) initarg: Option<String>,
    pub(crate) init_form: Option<Form>,
    pub(crate) class_value: Option<Rc<RefCell<Value>>>,
}

#[derive(Clone)]
pub struct ClassDefinition {
    pub(crate) name: String,
    pub(crate) precedence: Vec<String>,
    pub(crate) slots: Vec<ClassSlot>,
    pub(crate) default_initargs: Vec<(String, Form)>,
    pub(crate) documentation: Rc<RefCell<Option<String>>>,
}

#[derive(Clone)]
pub(crate) enum MethodSpecializer {
    Class(String),
    Eql(Value),
}

#[derive(Clone)]
pub struct MethodDefinition {
    pub(crate) id: u64,
    pub(crate) generic_function: String,
    pub(crate) lambda_list: Value,
    pub(crate) qualifiers: Vec<String>,
    pub(crate) specializers: Vec<MethodSpecializer>,
    pub(crate) function: Value,
}

#[derive(Clone)]
pub struct Instance {
    pub(crate) class: Rc<RefCell<Rc<ClassDefinition>>>,
    pub(crate) slots: SlotStorage,
}

#[derive(Clone)]
pub enum Function {
    Builtin {
        name: &'static str,
        function: Builtin,
    },
    Primitive {
        name: &'static str,
    },
    StructureConstructor {
        name: String,
        slots: Vec<StructureSlot>,
        structure_types: Vec<String>,
        representation: StructureRepresentation,
        constructor_lambda_list: Option<OrdinaryLambdaList>,
        environment: Environment,
    },
    StructurePredicate {
        name: String,
    },
    StructureAccessor {
        structure_name: String,
        slot_name: String,
        slot_index: usize,
        read_only: bool,
    },
    StructureCopier {
        name: String,
    },
    Generic {
        name: String,
        lambda_list: OrdinaryLambdaList,
        methods: Rc<RefCell<Vec<MethodDefinition>>>,
    },
    SlotReader {
        class_name: String,
        slot_name: String,
    },
    SlotWriter {
        class_name: String,
        slot_name: String,
    },
    ConditionReader {
        condition_name: String,
        slot_name: String,
    },
    ConditionWriter {
        condition_name: String,
        slot_name: String,
    },
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
    Macro {
        lambda_list: MacroLambdaList,
        body: Vec<Form>,
        environment: Environment,
    },
    LongDefsetf {
        lambda_list: MacroLambdaList,
        store_variable: String,
        body: Vec<Form>,
        environment: Environment,
    },
    ModifyMacro {
        lambda_list: MacroLambdaList,
        function: Form,
        environment: Environment,
    },
    Compiled {
        program: Rc<Program>,
        function: FunctionId,
        environment: Environment,
    },
}
