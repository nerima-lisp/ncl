use std::cell::RefCell;
use std::rc::Rc;

use ncl_compiler::{FunctionId, Program};
use ncl_syntax::{
    Form, LambdaListAuxiliaryParameter, LambdaListKeywordParameter, LambdaListOptionalParameter,
};

use crate::environment::Environment;
use crate::error::{ReturnValue, RuntimeError};

mod value_conditions;
mod value_display;
mod value_rational;
pub use value_conditions::{ConditionData, RestartData};
pub use value_rational::Rational;

mod value_comparison;
mod value_models;
use value_models::SlotValues;
pub use value_models::{
    ClassDefinition, ClassSlot, ClosureOptions, Instance, MacroAuxiliaryParameter,
    MacroKeywordParameter, MacroLambdaList, MacroOptionalParameter, MacroPattern, MethodDefinition,
    StructureDefinition, StructureSlot,
};

mod value_functions;
pub use value_functions::{Builtin, Function};

mod value_constructors;
mod value_stream;
mod value_stream_impl;
pub use value_stream::Stream;

mod random_state;
pub use random_state::RandomState;

#[derive(Clone)]
/// A dynamically typed NCL value.
pub enum Value {
    /// The canonical empty value.
    Nil,
    /// A variable marker indicating that no value is bound.
    Unbound,
    /// A boolean value.
    Boolean(bool),
    /// A signed machine integer.
    Integer(i64),
    /// An exact rational number.
    Rational(Rational),
    /// An IEEE-754 floating-point number.
    Float(f64),
    /// A string value.
    String(Rc<str>),
    /// A character value.
    Character(char),
    /// A stream backed by runtime state.
    Stream(Rc<RefCell<Stream>>),
    /// A `RANDOM-STATE` object backing the `RANDOM` family of functions.
    RandomState(Rc<RefCell<RandomState>>),
    /// A package name.
    Package(Rc<str>),
    /// A lexical environment.
    Environment(Environment),
    /// A case-insensitive symbol.
    Symbol(Rc<str>),
    /// A case-sensitive symbol.
    SymbolExact(Rc<str>),
    /// A symbol that is not interned in a package.
    UninternedSymbol(Rc<str>),
    /// A case-insensitive keyword symbol.
    Keyword(Rc<str>),
    /// A case-sensitive keyword symbol.
    KeywordExact(Rc<str>),
    /// A proper list.
    List(Rc<Vec<Self>>),
    /// A list with an explicit non-list tail.
    DottedList {
        /// Elements preceding the dotted tail.
        items: Rc<Vec<Self>>,
        /// The explicit non-list tail.
        tail: Rc<Self>,
    },
    /// A one-dimensional vector.
    Vector(Rc<Vec<Self>>),
    /// A multidimensional array.
    Array {
        /// Dimensions in row-major order.
        dimensions: Rc<Vec<usize>>,
        /// Elements stored in row-major order.
        elements: Rc<Vec<Self>>,
    },
    /// A mutable association table.
    HashTable {
        /// Equality predicate used by the table.
        test: Rc<str>,
        /// Mutable key/value entries.
        entries: Rc<RefCell<Vec<(Self, Self)>>>,
    },
    /// Multiple return values.
    Values(Rc<Vec<Self>>),
    /// A condition object.
    Condition(Rc<ConditionData>),
    /// A restart object.
    Restart(Rc<RestartData>),
    /// A structure instance.
    Structure {
        /// Structure name.
        name: Rc<str>,
        /// Included structure types.
        types: Rc<Vec<Rc<str>>>,
        /// Slot values in declaration order.
        slots: SlotValues,
    },
    /// A class definition.
    Class(Rc<ClassDefinition>),
    /// An instance of a class.
    Instance(Instance),
    /// A callable function.
    Function(Rc<Function>),
}

impl Value {
    pub(crate) fn condition(error: &RuntimeError) -> Self {
        let (actual_type, type_names, message, format_control, format_arguments) = match error {
            RuntimeError::Signaled(error) => (
                if error.warning {
                    "SIMPLE-WARNING".to_owned()
                } else {
                    error.condition.clone()
                },
                if error.condition_types.is_empty() {
                    vec![error.condition.clone()]
                } else {
                    error.condition_types.to_vec()
                },
                error.message.clone(),
                error.format_control.clone(),
                error
                    .format_arguments
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
        format_arguments: Vec<Self>,
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

    fn condition_from_parts_with_types(
        actual_type: String,
        type_names: Vec<String>,
        slots: Vec<(String, Self)>,
        message: String,
        format_control: Option<String>,
        format_arguments: Vec<Self>,
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
            message: Rc::from(message),
            format_control: format_control.map(|value| Rc::from(value.as_str())),
            format_arguments,
        }))
    }

    pub(crate) fn restart(name: impl AsRef<str>) -> Self {
        Self::Restart(Rc::new(RestartData {
            name: Rc::from(name.as_ref()),
        }))
    }

    /// Creates a callable value backed by a runtime builtin.
    pub fn builtin(name: &'static str, function: Builtin) -> Self {
        Self::Function(Rc::new(Function::Builtin { name, function }))
    }

    pub(crate) fn primitive(name: &'static str) -> Self {
        Self::Function(Rc::new(Function::Primitive { name }))
    }

    pub(crate) fn generic(name: impl Into<String>) -> Self {
        Self::Function(Rc::new(Function::Generic {
            name: name.into(),
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

    /// Creates a closure with required parameters and a lexical environment.
    #[must_use]
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
        Self::closure_with_keywords(
            ClosureOptions {
                parameters,
                required_escaped,
                optional,
                rest,
                rest_escaped: false,
                keywords: Vec::new(),
                has_keyword_section: false,
                allow_other_keys: false,
                auxiliary,
            },
            body,
            environment,
        )
    }

    pub(crate) fn closure_with_keywords(
        options: ClosureOptions,
        body: Vec<Form>,
        environment: Environment,
    ) -> Self {
        Self::Function(Rc::new(Function::Closure {
            parameters: options.parameters,
            required_escaped: options.required_escaped,
            optional: options.optional,
            rest: options.rest,
            rest_escaped: options.rest_escaped,
            keywords: options.keywords,
            has_keyword_section: options.has_keyword_section,
            allow_other_keys: options.allow_other_keys,
            auxiliary: options.auxiliary,
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
        slots: Vec<(String, Self)>,
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

    pub(crate) const fn class_object(definition: Rc<ClassDefinition>) -> Self {
        Self::Class(definition)
    }

    pub(crate) fn instance(definition: Rc<ClassDefinition>, slots: Vec<(String, Self)>) -> Self {
        Self::Instance(Instance {
            class: definition,
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
            Self::Instance(instance) => Some(instance.class.clone()),
            _ => None,
        }
    }

    pub(crate) fn instance_is_type(&self, expected: &str) -> bool {
        let Self::Instance(instance) = self else {
            return false;
        };
        instance
            .class
            .precedence
            .iter()
            .any(|class_name| class_name.eq_ignore_ascii_case(expected))
    }

    pub(crate) fn instance_slot(&self, slot_name: &str) -> Option<Self> {
        let Self::Instance(instance) = self else {
            return None;
        };
        if let Some(slot) = instance
            .class
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

    pub(crate) fn set_instance_slot(&self, class_name: &str, slot_name: &str, value: Self) -> bool {
        let Self::Instance(instance) = self else {
            return false;
        };
        if !self.instance_is_type(class_name) {
            return false;
        }
        if let Some(slot) = instance
            .class
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

    pub(crate) fn structure_slot(&self, index: usize) -> Option<Self> {
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
        value: Self,
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

    /// Returns whether this value is true in Lisp conditional contexts.
    #[must_use]
    pub fn is_truthy(&self) -> bool {
        !matches!(self.primary_value(), Self::Nil | Self::Boolean(false))
    }

    /// Returns the implementation's canonical Lisp type name.
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::Nil => "NIL",
            Self::Unbound => "UNBOUND",
            Self::Boolean(_) => "BOOLEAN",
            Self::Integer(_) => "INTEGER",
            Self::Rational(_) => "RATIO",
            Self::Float(_) => "FLOAT",
            Self::String(_) => "STRING",
            Self::Character(_) => "CHARACTER",
            Self::Stream(_) => "STREAM",
            Self::RandomState(_) => "RANDOM-STATE",
            Self::Package(_) => "PACKAGE",
            Self::Environment(_) => "ENVIRONMENT",
            Self::Symbol(_) | Self::SymbolExact(_) | Self::UninternedSymbol(_) => "SYMBOL",
            Self::Keyword(_) | Self::KeywordExact(_) => "KEYWORD",
            Self::List(_) | Self::DottedList { .. } => "LIST",
            Self::Vector(_) => "VECTOR",
            Self::Array { .. } => "ARRAY",
            Self::HashTable { .. } => "HASH-TABLE",
            Self::Values(_) => "VALUES",
            Self::Condition(_) => "CONDITION",
            Self::Restart(_) => "RESTART",
            Self::Structure { .. } => "STRUCTURE",
            Self::Class(_) => "CLASS",
            Self::Instance(_) => "STANDARD-OBJECT",
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

    pub(crate) fn condition_slot(&self, condition_name: &str, slot_name: &str) -> Option<Self> {
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
        value: Self,
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
            Self::Restart(restart) => Some(restart.name.as_ref()),
            _ => None,
        }
    }

    pub(crate) fn simple_condition_format_arguments(&self) -> Option<Vec<Self>> {
        match self {
            Self::Condition(condition) if condition.format_control.is_some() => {
                Some(condition.format_arguments.clone())
            }
            _ => None,
        }
    }

    /// Returns a copied proper-list payload when this value is a list.
    #[must_use]
    pub fn list_items(&self) -> Option<Vec<Self>> {
        match self {
            Self::Nil => Some(Vec::new()),
            Self::List(items) => Some(items.as_ref().clone()),
            _ => None,
        }
    }

    /// Returns a copied vector payload when this value is a vector.
    #[must_use]
    pub fn vector_items(&self) -> Option<Vec<Self>> {
        match self {
            Self::Vector(items) => Some(items.as_ref().clone()),
            _ => None,
        }
    }

    /// Returns copied array dimensions when this value is an array.
    #[must_use]
    pub fn array_dimensions(&self) -> Option<Vec<usize>> {
        match self {
            Self::Array { dimensions, .. } => Some(dimensions.as_ref().clone()),
            _ => None,
        }
    }

    /// Returns copied row-major array elements when this value is an array.
    #[must_use]
    pub fn array_items(&self) -> Option<Vec<Self>> {
        match self {
            Self::Array { elements, .. } => Some(elements.as_ref().clone()),
            _ => None,
        }
    }

    pub(crate) fn hash_table_test(&self) -> Option<&str> {
        match self {
            Self::HashTable { test, .. } => Some(test),
            _ => None,
        }
    }

    pub(crate) fn hash_table_entries(&self) -> Option<&RefCell<Vec<(Self, Self)>>> {
        match self {
            Self::HashTable { entries, .. } => Some(entries),
            _ => None,
        }
    }

    /// Returns the symbol-like name represented by this value, if any.
    #[must_use]
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

    /// Returns a symbol name and whether its spelling is exact.
    #[must_use]
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
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn streams_cover_input_pushback_output_and_close_state() {
        let mut input = Stream::input("ab\ncd", 0, 5);
        assert_eq!(input.kind_name(), "STRING-INPUT-STREAM");
        assert!(input.is_input());
        assert!(!input.is_output());
        assert_eq!(input.peek_char(), Some('a'));
        assert_eq!(input.read_char(), Some('a'));
        assert!(!input.unread_char('x'));
        assert!(input.unread_char('a'));
        assert!(!input.unread_char('a'));
        assert_eq!(input.read_line(), Some(("ab".to_owned(), false)));
        assert_eq!(input.remaining_input(), Some("cd".to_owned()));
        assert!(input.consume_input(2));
        assert_eq!(input.read_char(), None);
        assert!(!input.consume_input(1));
        if let Err(error) = input.close(true) {
            panic!("expected input close to succeed, got {error:?}");
        }
        assert!(input.peek_char().is_none());
        assert!(!input.unread_char('c'));
        assert!(input.close(false).is_ok());

        let mut output = Stream::output();
        assert_eq!(output.kind_name(), "STRING-OUTPUT-STREAM");
        assert!(!output.is_input());
        assert!(output.is_output());
        assert!(output.fresh_line().is_some());
        assert!(output.write("text"));
        assert_eq!(output.fresh_line(), Some(true));
        assert_eq!(output.take_output(), Some("text\n".to_owned()));
        assert_eq!(output.take_output(), Some(String::new()));
        assert!(output.read_char().is_none());
        if let Err(error) = output.close(true) {
            panic!("expected output close to succeed, got {error:?}");
        }
        assert!(!output.write("closed"));

        let mut io = Stream::file_io(PathBuf::from("unused"), "abc\n", true);
        assert_eq!(io.kind_name(), "FILE-IO-STREAM");
        assert!(io.is_input() && io.is_output());
        assert_eq!(io.peek_char(), None);
        assert_eq!(io.fresh_line(), Some(false));
        assert!(io.write("z"));
        if let Err(error) = io.close(true) {
            panic!("expected io close to succeed, got {error:?}");
        }
    }

    #[test]
    fn value_containers_and_conditions_have_stable_boundaries() {
        let list = Value::list(vec![Value::Integer(1), Value::symbol("x")]);
        assert_eq!(list.type_name(), "LIST");
        let Some(list_items) = list.list_items() else {
            panic!("expected list value");
        };
        assert_eq!(list_items.len(), 2);
        assert!(Value::list(Vec::new()).equal_value(&Value::Nil));
        let Some(vector_items) = Value::vector(vec![Value::Nil]).vector_items() else {
            panic!("expected vector value");
        };
        assert_eq!(vector_items.len(), 1);
        let array = Value::array(vec![2], vec![Value::Integer(1), Value::Integer(2)]);
        assert_eq!(array.array_dimensions(), Some(vec![2]));
        let Some(array_items) = array.array_items() else {
            panic!("expected array value");
        };
        assert_eq!(array_items.len(), 2);
        assert!(!Value::Nil.is_truthy());
        assert!(Value::values(vec![Value::Integer(7)]).is_truthy());
        assert!(
            Value::values(vec![])
                .primary_value()
                .equal_value(&Value::Nil)
        );

        let condition = Value::condition_from_parts_with_types(
            "SIMPLE-ERROR".to_owned(),
            vec!["SIMPLE-ERROR".to_owned()],
            vec![("DETAIL".to_owned(), Value::Integer(1))],
            "failed".to_owned(),
            Some("~A".to_owned()),
            vec![Value::Integer(1)],
        );
        assert!(condition.condition_is_type(":error"));
        assert!(
            condition
                .condition_slot("condition", "detail")
                .is_some_and(|value| value.equal_value(&Value::Integer(1)))
        );
        assert!(condition.set_condition_slot("error", "detail", Value::Integer(2)));
        assert!(
            condition
                .condition_slot("error", "detail")
                .is_some_and(|value| value.equal_value(&Value::Integer(2)))
        );
        assert!(condition.condition_slot("unrelated", "detail").is_none());
        assert!(!condition.set_condition_slot("error", "missing", Value::Nil));
        assert!(!Value::Nil.set_condition_slot("error", "detail", Value::Nil));
        assert_eq!(condition.condition_message(), Some("failed"));
        assert_eq!(condition.simple_condition_format_control(), Some("~A"));
        assert!(!Value::Nil.condition_is_type("error"));
    }

    #[test]
    fn condition_equality_checks_all_data_fields() {
        let condition = |actual_type: &str,
                         type_names: Vec<&str>,
                         slots: Vec<(&str, Value)>,
                         message: &str,
                         format_control: Option<&str>,
                         arguments: Vec<Value>| {
            Value::condition_from_parts_with_types(
                actual_type.to_owned(),
                type_names.into_iter().map(str::to_owned).collect(),
                slots
                    .into_iter()
                    .map(|(name, value)| (name.to_owned(), value))
                    .collect(),
                message.to_owned(),
                format_control.map(str::to_owned),
                arguments,
            )
        };
        let base = condition(
            "SIMPLE-ERROR",
            vec!["SIMPLE-ERROR"],
            vec![("DETAIL", Value::Integer(2))],
            "failed",
            Some("~A"),
            vec![Value::Integer(1)],
        );
        assert!(base.equal_value(&condition(
            "SIMPLE-ERROR",
            vec!["SIMPLE-ERROR"],
            vec![("DETAIL", Value::Integer(2))],
            "failed",
            Some("~A"),
            vec![Value::Integer(1)],
        )));

        let differences = [
            condition(
                "SIMPLE-WARNING",
                vec!["SIMPLE-ERROR"],
                vec![("DETAIL", Value::Integer(2))],
                "failed",
                Some("~A"),
                vec![Value::Integer(1)],
            ),
            condition(
                "SIMPLE-ERROR",
                vec!["ERROR"],
                vec![("DETAIL", Value::Integer(2))],
                "failed",
                Some("~A"),
                vec![Value::Integer(1)],
            ),
            condition(
                "SIMPLE-ERROR",
                vec!["SIMPLE-ERROR"],
                vec![("DETAIL", Value::Integer(2))],
                "changed",
                Some("~A"),
                vec![Value::Integer(1)],
            ),
            condition(
                "SIMPLE-ERROR",
                vec!["SIMPLE-ERROR"],
                vec![("DETAIL", Value::Integer(2))],
                "failed",
                None,
                vec![Value::Integer(1)],
            ),
            condition(
                "SIMPLE-ERROR",
                vec!["SIMPLE-ERROR"],
                vec![("DETAIL", Value::Integer(2))],
                "failed",
                Some("~A"),
                vec![Value::Integer(2)],
            ),
            condition(
                "SIMPLE-ERROR",
                vec!["SIMPLE-ERROR"],
                vec![("OTHER", Value::Integer(2))],
                "failed",
                Some("~A"),
                vec![Value::Integer(1)],
            ),
        ];
        assert!(
            differences
                .iter()
                .all(|different| !base.equal_value(different))
        );
    }

    #[test]
    fn values_have_stable_display_and_debug_forms() {
        #[allow(clippy::unnecessary_wraps)]
        fn no_op(_: &[Value]) -> Result<Value, RuntimeError> {
            Ok(Value::Nil)
        }

        let rational = match Value::rational(3, 2) {
            Ok(value) => value,
            Err(error) => panic!("expected valid rational, got {error:?}"),
        };
        let cases = [
            (Value::Nil, "NIL"),
            (Value::Unbound, "#<UNBOUND>"),
            (Value::Boolean(true), "T"),
            (Value::Boolean(false), "NIL"),
            (Value::Integer(7), "7"),
            (rational, "3/2"),
            (Value::Float(2.0), "2.0"),
            (Value::Float(2.5), "2.5"),
            (Value::string("line\n"), "\"line\\n\""),
            (Value::Character(' '), "#\\SPACE"),
            (Value::Character('\n'), "#\\NEWLINE"),
            (Value::Character('\t'), "#\\TAB"),
            (Value::Character('\r'), "#\\RETURN"),
            (Value::Character('x'), "#\\x"),
            (Value::package("USER"), "#<PACKAGE \"USER\">"),
            (Value::symbol("name"), "NAME"),
            (Value::symbol_exact("a|b\\c"), "|a\\|b\\\\c|"),
            (Value::uninterned_symbol("x"), "#:x"),
            (Value::keyword("key"), ":KEY"),
            (Value::keyword_exact("a|b"), ":|a\\|b|"),
            (
                Value::list(vec![Value::Integer(1), Value::Integer(2)]),
                "(1 2)",
            ),
            (Value::list(Vec::new()), "NIL"),
            (
                Value::dotted_list(vec![Value::Integer(1)], Value::Integer(2)),
                "(1 . 2)",
            ),
            (Value::dotted_list(Vec::new(), Value::Integer(2)), "(. 2)"),
            (Value::vector(vec![Value::Integer(1)]), "#(1)"),
            (
                Value::array(vec![2], vec![Value::Nil, Value::Nil]),
                "#<ARRAY [2]>",
            ),
            (Value::hash_table("EQ"), "#<HASH-TABLE EQ>"),
            (Value::values(vec![Value::Integer(1)]), "#<VALUES 1>"),
            (Value::values(Vec::new()), "#<VALUES>"),
            (Value::restart("retry"), "#<RESTART retry>"),
            (Value::builtin("NO-OP", no_op), "#<BUILTIN NO-OP>"),
            (Value::primitive("PRIMITIVE"), "#<PRIMITIVE PRIMITIVE>"),
            (Value::generic("combine"), "#<GENERIC-FUNCTION combine>"),
            (
                Value::slot_reader("person", "name"),
                "#<SLOT-READER person-name>",
            ),
            (
                Value::slot_writer("person", "name"),
                "#<SLOT-WRITER person-name>",
            ),
            (
                Value::closure(Vec::new(), Vec::new(), Environment::new()),
                "#<FUNCTION>",
            ),
            (
                Value::structure_with_types("point", Vec::new(), Vec::new()),
                "#S(point)",
            ),
        ];

        for (value, expected) in cases {
            assert_eq!(value.to_string(), expected);
            assert_eq!(format!("{value:?}"), format!("Value({expected})"));
        }
    }

    #[test]
    fn function_display_forms_cover_generated_function_kinds() {
        let cases = [
            (
                Function::StructureConstructor {
                    name: "MAKE-POINT".to_string(),
                    slots: Vec::new(),
                    structure_types: Vec::new(),
                    constructor_lambda_list: None,
                    environment: Environment::new(),
                },
                "#<STRUCTURE-CONSTRUCTOR MAKE-POINT>",
            ),
            (
                Function::StructurePredicate {
                    name: "POINT-P".to_string(),
                },
                "#<STRUCTURE-PREDICATE POINT-P>",
            ),
            (
                Function::StructureAccessor {
                    structure_name: "POINT".to_string(),
                    slot_name: "X".to_string(),
                    slot_index: 0,
                    read_only: false,
                },
                "#<STRUCTURE-ACCESSOR POINT-X>",
            ),
            (
                Function::StructureCopier {
                    name: "COPY-POINT".to_string(),
                },
                "#<STRUCTURE-COPIER COPY-POINT>",
            ),
            (
                Function::ConditionReader {
                    condition_name: "ERROR".to_string(),
                    slot_name: "MESSAGE".to_string(),
                },
                "#<CONDITION-READER ERROR-MESSAGE>",
            ),
            (
                Function::ConditionWriter {
                    condition_name: "ERROR".to_string(),
                    slot_name: "MESSAGE".to_string(),
                },
                "#<CONDITION-WRITER ERROR-MESSAGE>",
            ),
        ];

        for (function, expected) in cases {
            assert_eq!(Value::Function(Rc::new(function)).to_string(), expected);
        }
    }

    #[test]
    fn display_covers_exact_numeric_and_runtime_boundaries() {
        let cases = [
            (
                Value::rational(3, 2)
                    .unwrap_or_else(|error| panic!("rational should be valid: {error}")),
                "3/2",
            ),
            (Value::Character('\t'), "#\\TAB"),
            (Value::Character('\r'), "#\\RETURN"),
            (Value::keyword_exact("a|b"), ":|a\\|b|"),
            (Value::list(vec![Value::Integer(1)]), "(1)"),
            (
                Value::vector(vec![Value::Integer(1), Value::Integer(2)]),
                "#(1 2)",
            ),
            (Value::Environment(Environment::new()), "#<ENVIRONMENT>"),
            (
                Value::condition_from_parts(
                    "ERROR".to_owned(),
                    "failure".to_owned(),
                    None,
                    Vec::new(),
                ),
                "#<CONDITION failure>",
            ),
        ];

        for (value, expected) in cases {
            assert_eq!(value.to_string(), expected);
        }
    }
}
