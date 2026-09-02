use std::cell::RefCell;
use std::rc::Rc;

use super::{
    Form, LambdaListAuxiliaryParameter, LambdaListKeywordParameter, LambdaListOptionalParameter,
    Value,
};

mod macro_models;
pub use macro_models::{
    MacroAuxiliaryParameter, MacroKeywordParameter, MacroLambdaList, MacroOptionalParameter,
    MacroPattern,
};

#[derive(Clone, Debug)]
pub struct StructureSlot {
    pub(crate) name: String,
    pub(crate) init_form: Option<Form>,
    pub(crate) read_only: bool,
}

#[derive(Clone, Debug)]
pub struct StructureDefinition {
    pub(crate) slots: Vec<StructureSlot>,
    pub(crate) type_names: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ClassSlot {
    pub(crate) name: String,
    pub(crate) initargs: Vec<String>,
    pub(crate) init_form: Option<Form>,
    pub(crate) type_form: Option<Form>,
    pub(crate) class_value: Option<Rc<RefCell<Value>>>,
}

#[derive(Clone, Debug)]
pub struct ClassDefinition {
    pub(crate) name: String,
    pub(crate) direct_superclasses: Vec<Rc<str>>,
    pub(crate) direct_slots: Vec<Rc<str>>,
    pub(crate) direct_default_initargs: Vec<(String, Form)>,
    pub(crate) precedence: Vec<Rc<str>>,
    pub(crate) slots: Vec<ClassSlot>,
    pub(crate) default_initargs: Vec<(String, Form)>,
}

#[derive(Clone, Debug)]
pub struct MethodDefinition {
    pub(crate) qualifiers: Vec<String>,
    pub(crate) specializers: Vec<MethodSpecializer>,
    pub(crate) function: Value,
}

#[derive(Clone, Debug)]
pub enum MethodSpecializer {
    Class(Rc<str>),
    Eql(Value),
}

#[derive(Clone, Debug)]
pub struct Instance {
    pub class: Rc<ClassDefinition>,
    pub slots: SlotValues,
}

pub type SlotValues = Rc<RefCell<Vec<(Rc<str>, Value)>>>;

#[derive(Clone, Debug)]
pub struct ClosureOptions {
    pub(crate) parameters: Vec<String>,
    pub(crate) required_escaped: Vec<bool>,
    pub(crate) optional: Vec<LambdaListOptionalParameter>,
    pub(crate) rest: Option<String>,
    pub(crate) rest_escaped: bool,
    pub(crate) keywords: Vec<LambdaListKeywordParameter>,
    pub(crate) has_keyword_section: bool,
    pub(crate) allow_other_keys: bool,
    pub(crate) auxiliary: Vec<LambdaListAuxiliaryParameter>,
}
