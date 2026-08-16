use std::cell::RefCell;
use std::fmt;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::rc::Rc;

use ncl_compiler::{FunctionId, Program};
use ncl_syntax::{
    Form, LambdaListAuxiliaryParameter, LambdaListOptionalParameter, OrdinaryLambdaList,
};

use crate::environment::Environment;
use crate::error::{ReturnValue, RuntimeError};

mod conditions;
mod models;
mod numbers;
mod streams;

pub use conditions::{ConditionData, RestartData};
pub use models::{
    ClassDefinition, Function, Instance, MacroLambdaList, MethodDefinition, StructureSlot,
};
pub use numbers::Rational;
pub use streams::Stream;

pub(crate) use models::{
    ClassSlot, ClosureData, ConditionDefinition, ConditionSlot, MacroAuxiliaryParameter,
    MacroKeywordParameter, MacroOptionalParameter, MacroPattern, MethodSpecializer,
    StructureDefinition,
};

pub type Builtin = fn(&[Value]) -> Result<Value, RuntimeError>;
type SlotStorage = Rc<RefCell<Vec<(Rc<str>, Value)>>>;

#[derive(Clone)]
pub enum Value {
    Nil,
    Unbound,
    Boolean(bool),
    Integer(i64),
    Rational(Rational),
    Float(f64),
    Complex {
        real: Rc<Value>,
        imag: Rc<Value>,
    },
    String(Rc<str>),
    Character(char),
    Stream(Rc<RefCell<Stream>>),
    Package(Rc<str>),
    Environment(Environment),
    Symbol(Rc<str>),
    SymbolExact(Rc<str>),
    UninternedSymbol(Rc<str>),
    Keyword(Rc<str>),
    KeywordExact(Rc<str>),
    List(Rc<Vec<Value>>),
    DottedList {
        items: Rc<Vec<Value>>,
        tail: Rc<Value>,
    },
    Vector {
        elements: Rc<RefCell<Vec<Value>>>,
        length: usize,
        fill_pointer: Option<usize>,
        element_type: Rc<Value>,
        adjustable: bool,
        displaced_to: Option<Rc<Value>>,
        displaced_index_offset: usize,
    },
    Array {
        dimensions: Rc<Vec<usize>>,
        elements: Rc<RefCell<Vec<Value>>>,
        element_type: Rc<Value>,
        adjustable: bool,
        displaced_to: Option<Rc<Value>>,
        displaced_index_offset: usize,
    },
    HashTable {
        test: Rc<str>,
        entries: Rc<RefCell<Vec<(Value, Value)>>>,
    },
    Values(Rc<Vec<Value>>),
    Condition(Rc<ConditionData>),
    Restart(Rc<RestartData>),
    Structure {
        name: Rc<str>,
        types: Rc<Vec<Rc<str>>>,
        slots: SlotStorage,
    },
    Class(Rc<ClassDefinition>),
    Instance(Instance),
    Method(Rc<MethodDefinition>),
    Function(Rc<Function>),
}

impl Value {
    pub fn boolean(value: bool) -> Self {
        if value {
            Self::Boolean(true)
        } else {
            Self::Nil
        }
    }

    pub fn string(value: impl Into<Rc<str>>) -> Self {
        Self::String(value.into())
    }

    pub(crate) fn rational(numerator: i128, denominator: i128) -> Result<Self, RuntimeError> {
        let rational = Rational::new(numerator, denominator)?;
        if rational.denominator() == 1 {
            Ok(Self::Integer(rational.numerator()))
        } else {
            Ok(Self::Rational(rational))
        }
    }

    pub(crate) fn complex(real: Self, imag: Self) -> Self {
        Self::Complex {
            real: Rc::new(real),
            imag: Rc::new(imag),
        }
    }

    pub(crate) fn string_input_stream(source: &str, start: usize, end: usize) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::input(source, start, end))))
    }

    pub(crate) fn string_output_stream() -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::output())))
    }

    pub(crate) fn file_input_stream(source: String) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::file_input(source))))
    }

    pub(crate) fn file_output_stream(path: PathBuf, initial: String) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::file_output(path, initial))))
    }

    pub(crate) fn file_io_stream(path: PathBuf, source: String, append: bool) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::file_io(path, source, append))))
    }

    pub fn package(value: impl AsRef<str>) -> Self {
        Self::Package(Rc::from(value.as_ref()))
    }

    pub(crate) fn environment(value: Environment) -> Self {
        Self::Environment(value)
    }

    pub fn symbol(value: impl AsRef<str>) -> Self {
        Self::Symbol(Rc::from(value.as_ref().to_ascii_uppercase().as_str()))
    }

    pub fn symbol_exact(value: impl AsRef<str>) -> Self {
        Self::SymbolExact(Rc::from(value.as_ref()))
    }

    pub fn uninterned_symbol(value: impl AsRef<str>) -> Self {
        Self::UninternedSymbol(Rc::from(value.as_ref()))
    }

    pub fn keyword(value: impl AsRef<str>) -> Self {
        let value = value.as_ref().trim_start_matches(':').to_ascii_uppercase();
        Self::Keyword(Rc::from(value))
    }

    pub fn keyword_exact(value: impl AsRef<str>) -> Self {
        Self::KeywordExact(Rc::from(value.as_ref().trim_start_matches(':')))
    }

    pub fn list(values: Vec<Self>) -> Self {
        if values.is_empty() {
            Self::Nil
        } else {
            Self::List(Rc::new(values))
        }
    }

    pub fn dotted_list(items: Vec<Self>, tail: Self) -> Self {
        Self::DottedList {
            items: Rc::new(items),
            tail: Rc::new(tail),
        }
    }

    pub fn vector(values: Vec<Self>) -> Self {
        Self::vector_with_fill_pointer_element_type_adjustable_and_displacement(
            values,
            None,
            Self::symbol("T"),
            false,
            None,
            0,
        )
    }

    pub fn vector_with_fill_pointer(values: Vec<Self>, fill_pointer: usize) -> Self {
        Self::vector_with_fill_pointer_element_type_adjustable_and_displacement(
            values,
            Some(fill_pointer),
            Self::symbol("T"),
            false,
            None,
            0,
        )
    }

    pub fn array(dimensions: Vec<usize>, elements: Vec<Self>) -> Self {
        Self::array_with_element_type_adjustable_and_displacement(
            dimensions,
            elements,
            Self::symbol("T"),
            false,
            None,
            0,
        )
    }

    pub fn vector_with_fill_pointer_and_element_type(
        values: Vec<Self>,
        fill_pointer: Option<usize>,
        element_type: Self,
    ) -> Self {
        Self::vector_with_fill_pointer_element_type_adjustable_and_displacement(
            values,
            fill_pointer,
            element_type,
            false,
            None,
            0,
        )
    }

    pub fn vector_with_fill_pointer_element_type_and_adjustable(
        values: Vec<Self>,
        fill_pointer: Option<usize>,
        element_type: Self,
        adjustable: bool,
    ) -> Self {
        Self::vector_with_fill_pointer_element_type_adjustable_and_displacement(
            values,
            fill_pointer,
            element_type,
            adjustable,
            None,
            0,
        )
    }

    pub fn vector_with_fill_pointer_element_type_adjustable_and_displacement(
        values: Vec<Self>,
        fill_pointer: Option<usize>,
        element_type: Self,
        adjustable: bool,
        displaced_to: Option<Self>,
        displaced_index_offset: usize,
    ) -> Self {
        let length = values.len();
        Self::vector_with_storage_fill_pointer_element_type_adjustable_and_displacement(
            Rc::new(RefCell::new(values)),
            length,
            fill_pointer,
            element_type,
            adjustable,
            displaced_to,
            displaced_index_offset,
        )
    }

    pub(crate) fn vector_with_storage_fill_pointer_element_type_adjustable_and_displacement(
        elements: Rc<RefCell<Vec<Self>>>,
        length: usize,
        fill_pointer: Option<usize>,
        element_type: Self,
        adjustable: bool,
        displaced_to: Option<Self>,
        displaced_index_offset: usize,
    ) -> Self {
        Self::Vector {
            length,
            elements,
            fill_pointer,
            element_type: Rc::new(element_type),
            adjustable,
            displaced_to: displaced_to.map(Rc::new),
            displaced_index_offset,
        }
    }

    pub fn array_with_element_type(
        dimensions: Vec<usize>,
        elements: Vec<Self>,
        element_type: Self,
    ) -> Self {
        Self::array_with_element_type_adjustable_and_displacement(
            dimensions,
            elements,
            element_type,
            false,
            None,
            0,
        )
    }

    pub fn array_with_element_type_and_adjustable(
        dimensions: Vec<usize>,
        elements: Vec<Self>,
        element_type: Self,
        adjustable: bool,
    ) -> Self {
        Self::array_with_element_type_adjustable_and_displacement(
            dimensions,
            elements,
            element_type,
            adjustable,
            None,
            0,
        )
    }

    pub fn array_with_element_type_adjustable_and_displacement(
        dimensions: Vec<usize>,
        elements: Vec<Self>,
        element_type: Self,
        adjustable: bool,
        displaced_to: Option<Self>,
        displaced_index_offset: usize,
    ) -> Self {
        Self::array_with_storage_element_type_adjustable_and_displacement(
            dimensions,
            Rc::new(RefCell::new(elements)),
            element_type,
            adjustable,
            displaced_to,
            displaced_index_offset,
        )
    }

    pub(crate) fn array_with_storage_element_type_adjustable_and_displacement(
        dimensions: Vec<usize>,
        elements: Rc<RefCell<Vec<Self>>>,
        element_type: Self,
        adjustable: bool,
        displaced_to: Option<Self>,
        displaced_index_offset: usize,
    ) -> Self {
        Self::Array {
            dimensions: Rc::new(dimensions),
            elements,
            element_type: Rc::new(element_type),
            adjustable,
            displaced_to: displaced_to.map(Rc::new),
            displaced_index_offset,
        }
    }

    pub(crate) fn hash_table(test: impl AsRef<str>) -> Self {
        Self::HashTable {
            test: Rc::from(test.as_ref()),
            entries: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub(crate) fn values(values: Vec<Self>) -> Self {
        Self::Values(Rc::new(values))
    }

    pub(crate) fn condition(error: &RuntimeError) -> Self {
        let (actual_type, type_names, message, format_control, format_arguments) = match error {
            RuntimeError::Signaled {
                condition,
                condition_types,
                message,
                format_control,
                format_arguments,
                ..
            } => (
                error.condition_type_name(),
                if condition_types.is_empty() {
                    vec![condition.clone()]
                } else {
                    condition_types.to_vec()
                },
                message.clone(),
                format_control.clone(),
                format_arguments
                    .iter()
                    .cloned()
                    .map(ReturnValue::into_value)
                    .collect(),
            ),
            _ => (
                error.condition_type_name(),
                vec![error.condition_type_name()],
                error.to_string(),
                None,
                Vec::new(),
            ),
        };
        Self::condition_from_parts_with_types(
            actual_type,
            type_names,
            Vec::new(),
            message,
            format_control,
            format_arguments,
        )
    }

    pub(crate) fn condition_from_parts(
        actual_type: String,
        message: String,
        format_control: Option<String>,
        format_arguments: Vec<Value>,
    ) -> Self {
        Self::condition_from_parts_with_types(
            actual_type.clone(),
            vec![actual_type],
            Vec::new(),
            message,
            format_control,
            format_arguments,
        )
    }

    pub(crate) fn condition_from_definition(
        actual_type: String,
        type_names: Vec<String>,
        slots: Vec<(String, Value)>,
        message: String,
        format_control: Option<String>,
        format_arguments: Vec<Value>,
    ) -> Self {
        Self::condition_from_parts_with_types(
            actual_type,
            type_names,
            slots,
            message,
            format_control,
            format_arguments,
        )
    }

    fn condition_from_parts_with_types(
        actual_type: String,
        type_names: Vec<String>,
        slots: Vec<(String, Value)>,
        message: String,
        format_control: Option<String>,
        format_arguments: Vec<Value>,
    ) -> Self {
        Self::Condition(Rc::new(ConditionData {
            actual_type,
            type_names: Rc::new(type_names),
            slots: Rc::new(RefCell::new(
                slots
                    .into_iter()
                    .map(|(name, value)| (Rc::from(name.as_str()), value))
                    .collect(),
            )),
            message: Rc::from(message.as_str()),
            format_control: format_control.map(|value| Rc::from(value.as_str())),
            format_arguments,
        }))
    }

    pub(crate) fn restart(name: impl AsRef<str>) -> Self {
        Self::Restart(Rc::new(RestartData {
            name: Rc::from(name.as_ref()),
        }))
    }

    pub fn builtin(name: &'static str, function: Builtin) -> Self {
        Self::Function(Rc::new(Function::Builtin { name, function }))
    }

    pub(crate) fn primitive(name: &'static str) -> Self {
        Self::Function(Rc::new(Function::Primitive { name }))
    }

    pub(crate) fn generic(name: impl Into<String>, lambda_list: OrdinaryLambdaList) -> Self {
        Self::Function(Rc::new(Function::Generic {
            name: name.into(),
            lambda_list,
            methods: Rc::new(RefCell::new(Vec::new())),
        }))
    }

    pub(crate) fn slot_reader(class_name: impl Into<String>, slot_name: impl Into<String>) -> Self {
        Self::Function(Rc::new(Function::SlotReader {
            class_name: class_name.into(),
            slot_name: slot_name.into(),
        }))
    }

    pub(crate) fn slot_writer(class_name: impl Into<String>, slot_name: impl Into<String>) -> Self {
        Self::Function(Rc::new(Function::SlotWriter {
            class_name: class_name.into(),
            slot_name: slot_name.into(),
        }))
    }

    pub(crate) fn condition_reader(
        condition_name: impl Into<String>,
        slot_name: impl Into<String>,
    ) -> Self {
        Self::Function(Rc::new(Function::ConditionReader {
            condition_name: condition_name.into(),
            slot_name: slot_name.into(),
        }))
    }

    pub(crate) fn condition_writer(
        condition_name: impl Into<String>,
        slot_name: impl Into<String>,
    ) -> Self {
        Self::Function(Rc::new(Function::ConditionWriter {
            condition_name: condition_name.into(),
            slot_name: slot_name.into(),
        }))
    }

    pub fn closure(parameters: Vec<String>, body: Vec<Form>, environment: Environment) -> Self {
        Self::closure_with_optional(parameters, Vec::new(), None, body, environment)
    }

    pub(crate) fn closure_with_optional(
        parameters: Vec<String>,
        optional: Vec<LambdaListOptionalParameter>,
        rest: Option<String>,
        body: Vec<Form>,
        environment: Environment,
    ) -> Self {
        Self::closure_with_auxiliary(parameters, optional, rest, Vec::new(), body, environment)
    }

    pub(crate) fn closure_with_auxiliary(
        parameters: Vec<String>,
        optional: Vec<LambdaListOptionalParameter>,
        rest: Option<String>,
        auxiliary: Vec<LambdaListAuxiliaryParameter>,
        body: Vec<Form>,
        environment: Environment,
    ) -> Self {
        let required_escaped = vec![false; parameters.len()];
        Self::closure_with_keywords(ClosureData {
            parameters,
            required_escaped,
            optional,
            rest,
            rest_escaped: false,
            keywords: Vec::new(),
            has_keyword_section: false,
            allow_other_keys: false,
            auxiliary,
            body,
            environment,
        })
    }

    pub(crate) fn closure_with_keywords(data: ClosureData) -> Self {
        let ClosureData {
            parameters,
            required_escaped,
            optional,
            rest,
            rest_escaped,
            keywords,
            has_keyword_section,
            allow_other_keys,
            auxiliary,
            body,
            environment,
        } = data;
        Self::Function(Rc::new(Function::Closure {
            parameters,
            required_escaped,
            optional,
            rest,
            rest_escaped,
            keywords,
            has_keyword_section,
            allow_other_keys,
            auxiliary,
            body,
            environment,
        }))
    }

    pub(crate) fn macro_function(
        lambda_list: MacroLambdaList,
        body: Vec<Form>,
        environment: Environment,
    ) -> Self {
        Self::Function(Rc::new(Function::Macro {
            lambda_list,
            body,
            environment,
        }))
    }

    pub(crate) fn modify_macro_function(
        lambda_list: MacroLambdaList,
        function: Form,
        environment: Environment,
    ) -> Self {
        Self::Function(Rc::new(Function::ModifyMacro {
            lambda_list,
            function,
            environment,
        }))
    }

    pub(crate) fn long_defsetf_function(
        lambda_list: MacroLambdaList,
        store_variable: String,
        body: Vec<Form>,
        environment: Environment,
    ) -> Self {
        Self::Function(Rc::new(Function::LongDefsetf {
            lambda_list,
            store_variable,
            body,
            environment,
        }))
    }

    pub(crate) fn compiled(
        program: Rc<Program>,
        function: FunctionId,
        environment: Environment,
    ) -> Self {
        Self::Function(Rc::new(Function::Compiled {
            program,
            function,
            environment,
        }))
    }

    pub(crate) fn structure_with_types(
        name: impl AsRef<str>,
        slots: Vec<(String, Value)>,
        mut type_names: Vec<String>,
    ) -> Self {
        let name = name.as_ref().to_string();
        if !type_names
            .iter()
            .any(|type_name| type_name.eq_ignore_ascii_case(&name))
        {
            type_names.insert(0, name.clone());
        }
        Self::Structure {
            name: Rc::from(name),
            types: Rc::new(type_names.into_iter().map(Rc::<str>::from).collect()),
            slots: Rc::new(RefCell::new(
                slots
                    .into_iter()
                    .map(|(slot_name, value)| (Rc::from(slot_name), value))
                    .collect(),
            )),
        }
    }

    pub(crate) fn class_object(definition: Rc<ClassDefinition>) -> Self {
        Self::Class(definition)
    }

    pub(crate) fn instance(definition: Rc<ClassDefinition>, slots: Vec<(String, Value)>) -> Self {
        Self::Instance(Instance {
            class: Rc::new(RefCell::new(definition)),
            slots: Rc::new(RefCell::new(
                slots
                    .into_iter()
                    .map(|(slot_name, value)| (Rc::from(slot_name), value))
                    .collect(),
            )),
        })
    }

    pub(crate) fn instance_class_definition(&self) -> Option<Rc<ClassDefinition>> {
        match self {
            Self::Instance(instance) => Some(instance.class.borrow().clone()),
            _ => None,
        }
    }

    pub(crate) fn replace_instance_layout(
        &self,
        class: Rc<ClassDefinition>,
        slots: Vec<(String, Value)>,
    ) -> bool {
        let Self::Instance(instance) = self else {
            return false;
        };
        *instance.class.borrow_mut() = class;
        *instance.slots.borrow_mut() = slots
            .into_iter()
            .map(|(slot_name, value)| (Rc::<str>::from(slot_name), value))
            .collect();
        true
    }

    pub(crate) fn instance_is_type(&self, expected: &str) -> bool {
        let Self::Instance(instance) = self else {
            return false;
        };
        instance
            .class
            .borrow()
            .precedence
            .iter()
            .any(|class_name| class_name.eq_ignore_ascii_case(expected))
    }

    pub(crate) fn instance_slot(&self, slot_name: &str) -> Option<Value> {
        let Self::Instance(instance) = self else {
            return None;
        };
        let class = instance.class.borrow();
        if let Some(slot) = class
            .slots
            .iter()
            .find(|slot| slot.name.eq_ignore_ascii_case(slot_name))
            && let Some(class_value) = &slot.class_value
        {
            return Some(class_value.borrow().clone());
        }
        instance
            .slots
            .borrow()
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(slot_name))
            .map(|(_, value)| value.clone())
    }

    pub(crate) fn instance_slot_exists(&self, slot_name: &str) -> bool {
        let Self::Instance(instance) = self else {
            return false;
        };
        instance
            .slots
            .borrow()
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case(slot_name))
    }

    pub(crate) fn instance_slot_is_bound(&self, slot_name: &str) -> Option<bool> {
        self.instance_slot(slot_name)
            .map(|value| !matches!(value, Self::Unbound))
    }

    pub(crate) fn set_instance_slot(
        &self,
        class_name: &str,
        slot_name: &str,
        value: Value,
    ) -> bool {
        let Self::Instance(instance) = self else {
            return false;
        };
        if !self.instance_is_type(class_name) {
            return false;
        }
        let class = instance.class.borrow();
        if let Some(slot) = class
            .slots
            .iter()
            .find(|slot| slot.name.eq_ignore_ascii_case(slot_name))
            && let Some(class_value) = &slot.class_value
        {
            *class_value.borrow_mut() = value;
            return true;
        }
        let mut slots = instance.slots.borrow_mut();
        let Some((_, slot_value)) = slots
            .iter_mut()
            .find(|(name, _)| name.eq_ignore_ascii_case(slot_name))
        else {
            return false;
        };
        *slot_value = value;
        true
    }

    pub(crate) fn structure_name(&self) -> Option<&str> {
        match self {
            Self::Structure { name, .. } => Some(name),
            _ => None,
        }
    }

    pub(crate) fn structure_is_type(&self, expected: &str) -> bool {
        match self {
            Self::Structure { types, .. } => types
                .iter()
                .any(|type_name| type_name.eq_ignore_ascii_case(expected)),
            _ => false,
        }
    }

    pub(crate) fn structure_slot(&self, index: usize) -> Option<Value> {
        match self {
            Self::Structure { slots, .. } => {
                slots.borrow().get(index).map(|(_, value)| value.clone())
            }
            _ => None,
        }
    }

    pub(crate) fn set_structure_slot(
        &self,
        structure_name: &str,
        index: usize,
        value: Value,
    ) -> bool {
        let Self::Structure { slots, .. } = self else {
            return false;
        };
        if !self.structure_is_type(structure_name) {
            return false;
        }
        let mut slots = slots.borrow_mut();
        let Some((_, slot_value)) = slots.get_mut(index) else {
            return false;
        };
        *slot_value = value;
        true
    }

    pub(crate) fn copy_structure(&self) -> Option<Self> {
        let Self::Structure { name, types, slots } = self else {
            return None;
        };
        Some(Self::Structure {
            name: name.clone(),
            types: types.clone(),
            slots: Rc::new(RefCell::new(slots.borrow().clone())),
        })
    }

    pub(crate) fn primary_value(&self) -> Self {
        match self {
            Self::Values(values) => values.first().cloned().unwrap_or(Self::Nil),
            _ => self.clone(),
        }
    }

    pub(crate) fn multiple_values(&self) -> Vec<Self> {
        match self {
            Self::Values(values) => values.as_ref().clone(),
            _ => vec![self.clone()],
        }
    }

    pub fn is_truthy(&self) -> bool {
        !matches!(self.primary_value(), Self::Nil | Self::Boolean(false))
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Nil => "NIL",
            Self::Unbound => "UNBOUND",
            Self::Boolean(_) => "BOOLEAN",
            Self::Integer(_) => "INTEGER",
            Self::Rational(_) => "RATIO",
            Self::Float(_) => "FLOAT",
            Self::Complex { .. } => "COMPLEX",
            Self::String(_) => "STRING",
            Self::Character(_) => "CHARACTER",
            Self::Stream(_) => "STREAM",
            Self::Package(_) => "PACKAGE",
            Self::Environment(_) => "ENVIRONMENT",
            Self::Symbol(_) | Self::SymbolExact(_) | Self::UninternedSymbol(_) => "SYMBOL",
            Self::Keyword(_) | Self::KeywordExact(_) => "KEYWORD",
            Self::List(_) | Self::DottedList { .. } => "LIST",
            Self::Vector { .. } => "VECTOR",
            Self::Array { .. } => "ARRAY",
            Self::HashTable { .. } => "HASH-TABLE",
            Self::Values(_) => "VALUES",
            Self::Condition(_) => "CONDITION",
            Self::Restart(_) => "RESTART",
            Self::Structure { .. } => "STRUCTURE",
            Self::Class(_) => "CLASS",
            Self::Instance(_) => "STANDARD-OBJECT",
            Self::Method(_) => "METHOD",
            Self::Function(_) => "FUNCTION",
        }
    }

    pub(crate) fn condition_is_type(&self, expected: &str) -> bool {
        let Self::Condition(condition) = self else {
            return false;
        };
        let expected = expected.trim_start_matches(':').to_ascii_uppercase();
        if condition.actual_type.eq_ignore_ascii_case(&expected) {
            return true;
        }
        if condition
            .type_names
            .iter()
            .any(|type_name| type_name.eq_ignore_ascii_case(&expected))
        {
            return true;
        }
        if expected == "CONDITION" {
            return true;
        }
        match condition.actual_type.as_str() {
            "SIMPLE-ERROR" => matches!(
                expected.as_str(),
                "CONDITION" | "ERROR" | "SERIOUS-CONDITION" | "SIMPLE-CONDITION"
            ),
            "SIMPLE-WARNING" => matches!(
                expected.as_str(),
                "CONDITION" | "WARNING" | "SIMPLE-CONDITION"
            ),
            "SIMPLE-CONDITION" => expected == "CONDITION",
            "DIVISION-BY-ZERO" => matches!(
                expected.as_str(),
                "CONDITION" | "ERROR" | "SERIOUS-CONDITION" | "ARITHMETIC-ERROR"
            ),
            "ARITHMETIC-ERROR" => {
                matches!(
                    expected.as_str(),
                    "CONDITION" | "ERROR" | "SERIOUS-CONDITION"
                )
            }
            "TYPE-ERROR" | "PROGRAM-ERROR" | "PACKAGE-ERROR" | "READER-ERROR"
            | "COMPILER-ERROR" | "FILE-ERROR" | "UNBOUND-VARIABLE" => {
                matches!(
                    expected.as_str(),
                    "CONDITION" | "ERROR" | "SERIOUS-CONDITION"
                )
            }
            "CONTROL-ERROR" => matches!(expected.as_str(), "CONDITION"),
            _ => false,
        }
    }

    pub(crate) fn condition_type_names(&self) -> Option<Vec<String>> {
        match self {
            Self::Condition(condition) => Some(condition.type_names.as_ref().clone()),
            _ => None,
        }
    }

    pub(crate) fn condition_slot(&self, condition_name: &str, slot_name: &str) -> Option<Value> {
        let Self::Condition(condition) = self else {
            return None;
        };
        if !self.condition_is_type(condition_name) {
            return None;
        }
        condition
            .slots
            .borrow()
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(slot_name))
            .map(|(_, value)| value.clone())
    }

    pub(crate) fn set_condition_slot(
        &self,
        condition_name: &str,
        slot_name: &str,
        value: Value,
    ) -> bool {
        let Self::Condition(condition) = self else {
            return false;
        };
        if !self.condition_is_type(condition_name) {
            return false;
        }
        let mut slots = condition.slots.borrow_mut();
        if let Some((_, slot_value)) = slots
            .iter_mut()
            .find(|(name, _)| name.eq_ignore_ascii_case(slot_name))
        {
            *slot_value = value;
            true
        } else {
            false
        }
    }

    pub(crate) fn simple_condition_format_control(&self) -> Option<&str> {
        match self {
            Self::Condition(condition) => condition.format_control.as_deref(),
            _ => None,
        }
    }

    pub(crate) fn condition_type_name(&self) -> Option<&str> {
        match self {
            Self::Condition(condition) => Some(condition.actual_type.as_str()),
            _ => None,
        }
    }

    pub(crate) fn condition_message(&self) -> Option<&str> {
        match self {
            Self::Condition(condition) => Some(condition.message.as_ref()),
            _ => None,
        }
    }

    pub(crate) fn restart_name(&self) -> Option<&str> {
        match self {
            Self::Restart(restart) => Some(restart.name()),
            _ => None,
        }
    }

    pub(crate) fn simple_condition_format_arguments(&self) -> Option<Vec<Value>> {
        match self {
            Self::Condition(condition) if condition.format_control.is_some() => {
                Some(condition.format_arguments.clone())
            }
            _ => None,
        }
    }

    pub fn list_items(&self) -> Option<Vec<Value>> {
        match self {
            Self::Nil => Some(Vec::new()),
            Self::List(items) => Some(items.as_ref().clone()),
            _ => None,
        }
    }

    pub fn vector_items(&self) -> Option<Vec<Value>> {
        match self {
            Self::Vector {
                elements,
                length,
                displaced_index_offset,
                ..
            } => {
                let elements = elements.borrow();
                let end = displaced_index_offset.checked_add(*length)?;
                Some(elements[*displaced_index_offset..end].to_vec())
            }
            _ => None,
        }
    }

    pub fn vector_length(&self) -> Option<usize> {
        match self {
            Self::Vector { length, .. } => Some(*length),
            _ => None,
        }
    }

    pub fn vector_fill_pointer(&self) -> Option<usize> {
        match self {
            Self::Vector { fill_pointer, .. } => *fill_pointer,
            _ => None,
        }
    }

    pub fn array_element_type_value(&self) -> Option<Self> {
        match self {
            Self::Vector { element_type, .. } | Self::Array { element_type, .. } => {
                Some(element_type.as_ref().clone())
            }
            _ => None,
        }
    }

    pub fn is_simple_vector(&self) -> bool {
        matches!(
            self,
            Self::Vector {
                fill_pointer: None,
                adjustable: false,
                displaced_to: None,
                ..
            }
        )
    }

    pub fn is_adjustable_array(&self) -> bool {
        match self {
            Self::Vector { adjustable, .. } | Self::Array { adjustable, .. } => *adjustable,
            _ => false,
        }
    }

    pub fn array_dimensions(&self) -> Option<Vec<usize>> {
        match self {
            Self::Array { dimensions, .. } => Some(dimensions.as_ref().clone()),
            _ => None,
        }
    }

    pub fn array_items(&self) -> Option<Vec<Value>> {
        match self {
            Self::Array {
                dimensions,
                elements,
                displaced_index_offset,
                ..
            } => {
                let total_size = dimensions.iter().copied().product::<usize>();
                let elements = elements.borrow();
                let end = displaced_index_offset.checked_add(total_size)?;
                Some(elements[*displaced_index_offset..end].to_vec())
            }
            _ => None,
        }
    }

    pub fn array_storage(&self) -> Option<Rc<RefCell<Vec<Value>>>> {
        match self {
            Self::Vector { elements, .. } | Self::Array { elements, .. } => Some(elements.clone()),
            _ => None,
        }
    }

    pub fn array_displacement_value(&self) -> Option<(Self, usize)> {
        match self {
            Self::Vector {
                displaced_to,
                displaced_index_offset,
                ..
            }
            | Self::Array {
                displaced_to,
                displaced_index_offset,
                ..
            } => displaced_to
                .as_ref()
                .map(|displaced_to| (displaced_to.as_ref().clone(), *displaced_index_offset)),
            _ => None,
        }
    }

    pub fn with_array_displacement(
        self,
        displaced_to: Option<Self>,
        displaced_index_offset: usize,
    ) -> Self {
        match self {
            Self::Vector {
                elements,
                length,
                fill_pointer,
                element_type,
                adjustable,
                ..
            } => Self::Vector {
                elements,
                length,
                fill_pointer,
                element_type,
                adjustable,
                displaced_to: displaced_to.map(Rc::new),
                displaced_index_offset,
            },
            Self::Array {
                dimensions,
                elements,
                element_type,
                adjustable,
                ..
            } => Self::Array {
                dimensions,
                elements,
                element_type,
                adjustable,
                displaced_to: displaced_to.map(Rc::new),
                displaced_index_offset,
            },
            value => value,
        }
    }

    pub fn with_array_displacement_value(self, displacement: Option<(Self, usize)>) -> Self {
        match displacement {
            Some((displaced_to, displaced_index_offset)) => {
                self.with_array_displacement(Some(displaced_to), displaced_index_offset)
            }
            None => self.with_array_displacement(None, 0),
        }
    }

    pub(crate) fn hash_table_test(&self) -> Option<&str> {
        match self {
            Self::HashTable { test, .. } => Some(test),
            _ => None,
        }
    }

    pub(crate) fn hash_table_entries(&self) -> Option<&RefCell<Vec<(Value, Value)>>> {
        match self {
            Self::HashTable { entries, .. } => Some(entries),
            _ => None,
        }
    }

    pub fn symbol_name(&self) -> Option<&str> {
        match self {
            Self::Symbol(name)
            | Self::SymbolExact(name)
            | Self::UninternedSymbol(name)
            | Self::Keyword(name)
            | Self::KeywordExact(name) => Some(name),
            Self::Nil | Self::Boolean(false) => Some("NIL"),
            Self::Boolean(true) => Some("T"),
            _ => None,
        }
    }

    pub fn symbol_reference(&self) -> Option<(&str, bool)> {
        match self {
            Self::Symbol(name) | Self::UninternedSymbol(name) | Self::Keyword(name) => {
                Some((name, false))
            }
            Self::SymbolExact(name) | Self::KeywordExact(name) => Some((name, true)),
            Self::Nil | Self::Boolean(false) => Some(("NIL", false)),
            Self::Boolean(true) => Some(("T", false)),
            _ => None,
        }
    }

    pub fn eq_value(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Nil, Self::Nil) => true,
            (Self::Unbound, Self::Unbound) => true,
            (Self::Nil, Self::Boolean(false)) | (Self::Boolean(false), Self::Nil) => true,
            (Self::Boolean(left), Self::Boolean(right)) => left == right,
            (Self::Integer(left), Self::Integer(right)) => left == right,
            (Self::Rational(left), Self::Rational(right)) => left == right,
            (Self::Float(left), Self::Float(right)) => left == right,
            (
                Self::Complex {
                    real: left_real,
                    imag: left_imag,
                },
                Self::Complex {
                    real: right_real,
                    imag: right_imag,
                },
            ) => left_real.eq_value(right_real) && left_imag.eq_value(right_imag),
            (Self::Character(left), Self::Character(right)) => left == right,
            (Self::Stream(left), Self::Stream(right)) => Rc::ptr_eq(left, right),
            (Self::Package(left), Self::Package(right)) => left == right,
            (Self::String(left), Self::String(right)) => Rc::ptr_eq(left, right),
            (Self::Symbol(left), Self::Symbol(right))
            | (Self::Keyword(left), Self::Keyword(right)) => left == right,
            (Self::SymbolExact(left), Self::SymbolExact(right))
            | (Self::KeywordExact(left), Self::KeywordExact(right)) => left == right,
            (Self::UninternedSymbol(left), Self::UninternedSymbol(right)) => {
                Rc::ptr_eq(left, right)
            }
            (Self::List(left), Self::List(right)) => Rc::ptr_eq(left, right),
            (
                Self::Vector {
                    elements: left_elements,
                    length: left_length,
                    fill_pointer: left_fill_pointer,
                    element_type: left_element_type,
                    adjustable: left_adjustable,
                    displaced_to: left_displaced_to,
                    displaced_index_offset: left_displaced_index_offset,
                },
                Self::Vector {
                    elements: right_elements,
                    length: right_length,
                    fill_pointer: right_fill_pointer,
                    element_type: right_element_type,
                    adjustable: right_adjustable,
                    displaced_to: right_displaced_to,
                    displaced_index_offset: right_displaced_index_offset,
                },
            ) => {
                Rc::ptr_eq(left_elements, right_elements)
                    && left_length == right_length
                    && left_fill_pointer == right_fill_pointer
                    && Rc::ptr_eq(left_element_type, right_element_type)
                    && left_adjustable == right_adjustable
                    && left_displaced_index_offset == right_displaced_index_offset
                    && match (left_displaced_to, right_displaced_to) {
                        (Some(left), Some(right)) => Rc::ptr_eq(left, right),
                        (None, None) => true,
                        _ => false,
                    }
            }
            (
                Self::Array {
                    dimensions: left_dimensions,
                    elements: left_elements,
                    element_type: left_element_type,
                    adjustable: left_adjustable,
                    displaced_to: left_displaced_to,
                    displaced_index_offset: left_displaced_index_offset,
                },
                Self::Array {
                    dimensions: right_dimensions,
                    elements: right_elements,
                    element_type: right_element_type,
                    adjustable: right_adjustable,
                    displaced_to: right_displaced_to,
                    displaced_index_offset: right_displaced_index_offset,
                },
            ) => {
                Rc::ptr_eq(left_dimensions, right_dimensions)
                    && Rc::ptr_eq(left_elements, right_elements)
                    && Rc::ptr_eq(left_element_type, right_element_type)
                    && left_adjustable == right_adjustable
                    && left_displaced_index_offset == right_displaced_index_offset
                    && match (left_displaced_to, right_displaced_to) {
                        (Some(left), Some(right)) => Rc::ptr_eq(left, right),
                        (None, None) => true,
                        _ => false,
                    }
            }
            (
                Self::HashTable {
                    entries: left_entries,
                    ..
                },
                Self::HashTable {
                    entries: right_entries,
                    ..
                },
            ) => Rc::ptr_eq(left_entries, right_entries),
            (Self::Values(left), Self::Values(right)) => Rc::ptr_eq(left, right),
            (Self::Condition(left), Self::Condition(right)) => Rc::ptr_eq(left, right),
            (Self::Restart(left), Self::Restart(right)) => Rc::ptr_eq(left, right),
            (
                Self::Structure {
                    name: left_name,
                    slots: left_slots,
                    ..
                },
                Self::Structure {
                    name: right_name,
                    slots: right_slots,
                    ..
                },
            ) => Rc::ptr_eq(left_name, right_name) && Rc::ptr_eq(left_slots, right_slots),
            (Self::Class(left), Self::Class(right)) => Rc::ptr_eq(left, right),
            (Self::Environment(left), Self::Environment(right)) => left.same(right),
            (Self::Instance(left), Self::Instance(right)) => {
                Rc::ptr_eq(&left.class, &right.class) && Rc::ptr_eq(&left.slots, &right.slots)
            }
            (Self::Method(left), Self::Method(right)) => left.id == right.id,
            (
                Self::DottedList {
                    items: left,
                    tail: left_tail,
                },
                Self::DottedList {
                    items: right,
                    tail: right_tail,
                },
            ) => Rc::ptr_eq(left, right) && Rc::ptr_eq(left_tail, right_tail),
            (Self::Function(left), Self::Function(right)) => Rc::ptr_eq(left, right),
            _ => false,
        }
    }

    pub fn equal_value(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Complex {
                    real: left_real,
                    imag: left_imag,
                },
                Self::Complex {
                    real: right_real,
                    imag: right_imag,
                },
            ) => left_real.equal_value(right_real) && left_imag.equal_value(right_imag),
            (Self::String(left), Self::String(right)) => left == right,
            (Self::List(left), Self::List(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right.iter())
                        .all(|(left, right)| left.equal_value(right))
            }
            (
                Self::Vector {
                    elements: left_elements,
                    length: left_length,
                    fill_pointer: left_fill_pointer,
                    element_type: left_element_type,
                    adjustable: left_adjustable,
                    displaced_to: left_displaced_to,
                    displaced_index_offset: left_displaced_index_offset,
                },
                Self::Vector {
                    elements: right_elements,
                    length: right_length,
                    fill_pointer: right_fill_pointer,
                    element_type: right_element_type,
                    adjustable: right_adjustable,
                    displaced_to: right_displaced_to,
                    displaced_index_offset: right_displaced_index_offset,
                },
            ) => {
                let left_items = {
                    let end = left_displaced_index_offset + left_length;
                    left_elements.borrow()[*left_displaced_index_offset..end].to_vec()
                };
                let right_items = {
                    let end = right_displaced_index_offset + right_length;
                    right_elements.borrow()[*right_displaced_index_offset..end].to_vec()
                };
                left_fill_pointer == right_fill_pointer
                    && left_length == right_length
                    && left_adjustable == right_adjustable
                    && left_displaced_index_offset == right_displaced_index_offset
                    && left_element_type.equal_value(right_element_type)
                    && match (left_displaced_to, right_displaced_to) {
                        (Some(left), Some(right)) => left.equal_value(right),
                        (None, None) => true,
                        _ => false,
                    }
                    && left_items.len() == right_items.len()
                    && left_items
                        .iter()
                        .zip(right_items.iter())
                        .all(|(left, right)| left.equal_value(right))
            }
            (
                Self::Array {
                    dimensions: left_dimensions,
                    element_type: left_element_type,
                    adjustable: left_adjustable,
                    displaced_to: left_displaced_to,
                    displaced_index_offset: left_displaced_index_offset,
                    ..
                },
                Self::Array {
                    dimensions: right_dimensions,
                    element_type: right_element_type,
                    adjustable: right_adjustable,
                    displaced_to: right_displaced_to,
                    displaced_index_offset: right_displaced_index_offset,
                    ..
                },
            ) => {
                let Some(left_items) = self.array_items() else {
                    return false;
                };
                let Some(right_items) = other.array_items() else {
                    return false;
                };
                left_dimensions == right_dimensions
                    && left_adjustable == right_adjustable
                    && left_displaced_index_offset == right_displaced_index_offset
                    && left_element_type.equal_value(right_element_type)
                    && match (left_displaced_to, right_displaced_to) {
                        (Some(left), Some(right)) => left.equal_value(right),
                        (None, None) => true,
                        _ => false,
                    }
                    && left_items.len() == right_items.len()
                    && left_items
                        .iter()
                        .zip(right_items.iter())
                        .all(|(left, right)| left.equal_value(right))
            }
            (Self::Values(left), Self::Values(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right.iter())
                        .all(|(left, right)| left.equal_value(right))
            }
            (Self::Condition(left), Self::Condition(right)) => left.equal_value(right),
            (Self::Restart(left), Self::Restart(right)) => Rc::ptr_eq(left, right),
            (
                Self::Structure {
                    name: left_name,
                    slots: left_slots,
                    ..
                },
                Self::Structure {
                    name: right_name,
                    slots: right_slots,
                    ..
                },
            ) => {
                if left_name != right_name {
                    return false;
                }
                let left_slots = left_slots.borrow();
                let right_slots = right_slots.borrow();
                left_slots.len() == right_slots.len()
                    && left_slots.iter().zip(right_slots.iter()).all(
                        |((left_name, left_value), (right_name, right_value))| {
                            left_name == right_name && left_value.equal_value(right_value)
                        },
                    )
            }
            (Self::Class(left), Self::Class(right)) => left.name.eq_ignore_ascii_case(&right.name),
            (Self::Instance(left), Self::Instance(right)) => {
                let left_class = left.class.borrow();
                let right_class = right.class.borrow();
                if !left_class.name.eq_ignore_ascii_case(&right_class.name) {
                    return false;
                }
                let left_slots = left.slots.borrow();
                let right_slots = right.slots.borrow();
                left_slots.len() == right_slots.len()
                    && left_slots.iter().zip(right_slots.iter()).all(
                        |((left_name, left_value), (right_name, right_value))| {
                            left_name.eq_ignore_ascii_case(right_name)
                                && left_value.equal_value(right_value)
                        },
                    )
            }
            (
                Self::DottedList {
                    items: left,
                    tail: left_tail,
                },
                Self::DottedList {
                    items: right,
                    tail: right_tail,
                },
            ) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right.iter())
                        .all(|(left, right)| left.equal_value(right))
                    && left_tail.equal_value(right_tail)
            }
            _ => self.eq_value(other),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nil => formatter.write_str("NIL"),
            Self::Unbound => formatter.write_str("#<UNBOUND>"),
            Self::Boolean(true) => formatter.write_str("T"),
            Self::Boolean(false) => formatter.write_str("NIL"),
            Self::Integer(value) => value.fmt(formatter),
            Self::Rational(value) => {
                write!(formatter, "{}/{}", value.numerator(), value.denominator())
            }
            Self::Float(value) => {
                if value.fract() == 0.0 {
                    write!(formatter, "{value:.1}")
                } else {
                    value.fmt(formatter)
                }
            }
            Self::Complex { real, imag } => write!(formatter, "#C({real} {imag})"),
            Self::String(value) => write!(formatter, "{value:?}"),
            Self::Character(value) => match value {
                ' ' => formatter.write_str("#\\SPACE"),
                '\n' => formatter.write_str("#\\NEWLINE"),
                '\t' => formatter.write_str("#\\TAB"),
                '\r' => formatter.write_str("#\\RETURN"),
                value => write!(formatter, "#\\{value}"),
            },
            Self::Stream(stream) => write!(formatter, "#<{}>", stream.borrow().kind_name()),
            Self::Package(value) => write!(formatter, "#<PACKAGE \"{value}\">"),
            Self::Environment(_) => formatter.write_str("#<ENVIRONMENT>"),
            Self::Symbol(value) => formatter.write_str(value),
            Self::SymbolExact(value) => write_escaped_symbol(formatter, value),
            Self::UninternedSymbol(value) => write!(formatter, "#:{value}"),
            Self::Keyword(value) => write!(formatter, ":{value}"),
            Self::KeywordExact(value) => {
                formatter.write_char(':')?;
                write_escaped_symbol(formatter, value)
            }
            Self::List(values) => {
                formatter.write_str("(")?;
                write_sequence(formatter, values)?;
                formatter.write_str(")")
            }
            Self::DottedList { items, tail } => {
                formatter.write_str("(")?;
                write_sequence(formatter, items)?;
                if !items.is_empty() {
                    formatter.write_str(" ")?;
                }
                write!(formatter, ". {tail})")
            }
            Self::Vector { .. } => {
                let values = self.vector_items().unwrap_or_default();
                formatter.write_str("#(")?;
                write_sequence(formatter, &values)?;
                formatter.write_str(")")
            }
            Self::Array { dimensions, .. } => write!(formatter, "#<ARRAY {dimensions:?}>"),
            Self::HashTable { test, .. } => write!(formatter, "#<HASH-TABLE {test}>"),
            Self::Method(_) => formatter.write_str("#<METHOD>"),
            Self::Values(values) => {
                formatter.write_str("#<VALUES")?;
                if !values.is_empty() {
                    formatter.write_str(" ")?;
                    write_sequence(formatter, values)?;
                }
                formatter.write_str(">")
            }
            Self::Condition(condition) => write!(formatter, "#<CONDITION {}>", condition.message),
            Self::Restart(restart) => write!(formatter, "#<RESTART {}>", restart.name()),
            Self::Structure { name, slots, .. } => {
                write!(formatter, "#S({name}")?;
                for (slot_name, value) in slots.borrow().iter() {
                    write!(formatter, " :{slot_name} {value}")?;
                }
                formatter.write_char(')')
            }
            Self::Class(definition) => write!(formatter, "#<CLASS {}>", definition.name),
            Self::Instance(instance) => {
                let class = instance.class.borrow();
                write!(formatter, "#<{} INSTANCE>", class.name)
            }
            Self::Function(function) => match function.as_ref() {
                Function::Builtin { name, .. } => write!(formatter, "#<BUILTIN {name}>"),
                Function::Primitive { name } => write!(formatter, "#<PRIMITIVE {name}>"),
                Function::StructureConstructor { name, .. } => {
                    write!(formatter, "#<STRUCTURE-CONSTRUCTOR {name}>")
                }
                Function::StructurePredicate { name } => {
                    write!(formatter, "#<STRUCTURE-PREDICATE {name}>")
                }
                Function::StructureAccessor {
                    structure_name,
                    slot_name,
                    ..
                } => write!(
                    formatter,
                    "#<STRUCTURE-ACCESSOR {structure_name}-{slot_name}>"
                ),
                Function::StructureCopier { name } => {
                    write!(formatter, "#<STRUCTURE-COPIER {name}>")
                }
                Function::Generic { name, .. } => write!(formatter, "#<GENERIC-FUNCTION {name}>"),
                Function::SlotReader {
                    class_name,
                    slot_name,
                } => write!(formatter, "#<SLOT-READER {class_name}-{slot_name}>"),
                Function::SlotWriter {
                    class_name,
                    slot_name,
                } => write!(formatter, "#<SLOT-WRITER {class_name}-{slot_name}>"),
                Function::ConditionReader {
                    condition_name,
                    slot_name,
                } => write!(
                    formatter,
                    "#<CONDITION-READER {condition_name}-{slot_name}>"
                ),
                Function::ConditionWriter {
                    condition_name,
                    slot_name,
                } => write!(
                    formatter,
                    "#<CONDITION-WRITER {condition_name}-{slot_name}>"
                ),
                Function::Closure { .. } | Function::Compiled { .. } => {
                    formatter.write_str("#<FUNCTION>")
                }
                Function::Macro { .. }
                | Function::LongDefsetf { .. }
                | Function::ModifyMacro { .. } => formatter.write_str("#<MACRO>"),
            },
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Value(")?;
        fmt::Display::fmt(self, formatter)?;
        formatter.write_str(")")
    }
}

fn write_sequence(formatter: &mut fmt::Formatter<'_>, values: &[Value]) -> fmt::Result {
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            formatter.write_str(" ")?;
        }
        fmt::Display::fmt(value, formatter)?;
    }
    Ok(())
}

fn write_escaped_symbol(formatter: &mut fmt::Formatter<'_>, value: &str) -> fmt::Result {
    formatter.write_char('|')?;
    for character in value.chars() {
        if matches!(character, '|' | '\\') {
            formatter.write_char('\\')?;
        }
        formatter.write_char(character)?;
    }
    formatter.write_char('|')
}
