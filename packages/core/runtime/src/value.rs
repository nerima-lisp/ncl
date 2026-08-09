use std::cell::RefCell;
use std::fmt;
use std::fmt::Write as _;
use std::rc::Rc;

use ncl_compiler::{FunctionId, Program};
use ncl_syntax::{
    Form, LambdaListAuxiliaryParameter, LambdaListKeywordParameter, LambdaListOptionalParameter,
    OrdinaryLambdaList,
};

use crate::environment::Environment;
use crate::error::RuntimeError;

pub type Builtin = fn(&[Value]) -> Result<Value, RuntimeError>;

#[derive(Clone)]
pub(crate) enum MacroPattern {
    Name(String),
    List(Vec<MacroPattern>),
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
pub(crate) struct MacroLambdaList {
    pub(crate) whole: Option<String>,
    pub(crate) required: Vec<MacroPattern>,
    pub(crate) optional: Vec<MacroOptionalParameter>,
    pub(crate) rest: Option<String>,
    pub(crate) keywords: Vec<MacroKeywordParameter>,
    pub(crate) has_keyword_section: bool,
    pub(crate) allow_other_keys: bool,
    pub(crate) auxiliary: Vec<MacroAuxiliaryParameter>,
}

#[derive(Clone)]
pub(crate) struct StructureSlot {
    pub(crate) name: String,
    pub(crate) init_form: Option<Form>,
    pub(crate) read_only: bool,
}

#[derive(Clone)]
pub(crate) struct StructureDefinition {
    pub(crate) slots: Vec<StructureSlot>,
    pub(crate) type_names: Vec<String>,
}

#[derive(Clone)]
pub(crate) struct ClassSlot {
    pub(crate) name: String,
    pub(crate) initarg: Option<String>,
    pub(crate) init_form: Option<Form>,
}

#[derive(Clone)]
pub(crate) struct ClassDefinition {
    pub(crate) name: String,
    pub(crate) direct_superclasses: Vec<String>,
    pub(crate) precedence: Vec<String>,
    pub(crate) slots: Vec<ClassSlot>,
}

#[derive(Clone)]
pub(crate) struct MethodDefinition {
    pub(crate) qualifiers: Vec<String>,
    pub(crate) specializers: Vec<String>,
    pub(crate) function: Value,
}

#[derive(Clone)]
pub(crate) struct Instance {
    pub(crate) class: Rc<ClassDefinition>,
    pub(crate) slots: Rc<RefCell<Vec<(Rc<str>, Value)>>>,
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
    Compiled {
        program: Rc<Program>,
        function: FunctionId,
        environment: Environment,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rational {
    numerator: i64,
    denominator: i64,
}

impl Rational {
    pub(crate) fn new(numerator: i128, denominator: i128) -> Result<Self, RuntimeError> {
        if denominator == 0 {
            return Err(RuntimeError::DivisionByZero);
        }

        let (numerator, denominator) = if denominator < 0 {
            (
                numerator.checked_neg().ok_or(RuntimeError::NumericOverflow)?,
                denominator
                    .checked_neg()
                    .ok_or(RuntimeError::NumericOverflow)?,
            )
        } else {
            (numerator, denominator)
        };

        let numerator_abs = if numerator < 0 {
            numerator
                .checked_neg()
                .ok_or(RuntimeError::NumericOverflow)? as u128
        } else {
            numerator as u128
        };
        let denominator_abs = denominator as u128;
        let divisor = gcd(numerator_abs, denominator_abs);
        let numerator = i64::try_from(numerator / divisor as i128)
            .map_err(|_| RuntimeError::NumericOverflow)?;
        let denominator = i64::try_from(denominator / divisor as i128)
            .map_err(|_| RuntimeError::NumericOverflow)?;

        Ok(Self {
            numerator,
            denominator,
        })
    }

    pub(crate) fn numerator(self) -> i64 {
        self.numerator
    }

    pub(crate) fn denominator(self) -> i64 {
        self.denominator
    }
}

fn gcd(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

pub struct Stream {
    kind: StreamKind,
    closed: bool,
}

enum StreamKind {
    Input {
        characters: Rc<Vec<char>>,
        position: usize,
        pushback: Option<char>,
    },
    Output {
        buffer: String,
        at_line_start: bool,
    },
}

impl Stream {
    fn input(source: &str, start: usize, end: usize) -> Self {
        Self {
            kind: StreamKind::Input {
                characters: Rc::new(source.chars().skip(start).take(end - start).collect()),
                position: 0,
                pushback: None,
            },
            closed: false,
        }
    }

    fn output() -> Self {
        Self {
            kind: StreamKind::Output {
                buffer: String::new(),
                at_line_start: true,
            },
            closed: false,
        }
    }

    pub(crate) fn kind_name(&self) -> &'static str {
        match &self.kind {
            StreamKind::Input { .. } => "STRING-INPUT-STREAM",
            StreamKind::Output { .. } => "STRING-OUTPUT-STREAM",
        }
    }

    pub(crate) fn is_input(&self) -> bool {
        matches!(&self.kind, StreamKind::Input { .. })
    }

    pub(crate) fn is_output(&self) -> bool {
        matches!(&self.kind, StreamKind::Output { .. })
    }

    pub(crate) fn read_char(&mut self) -> Option<char> {
        if self.closed {
            return None;
        }
        let StreamKind::Input {
            characters,
            position,
            pushback,
        } = &mut self.kind
        else {
            return None;
        };
        if let Some(character) = pushback.take() {
            return Some(character);
        }
        let character = characters.get(*position).copied()?;
        *position += 1;
        Some(character)
    }

    pub(crate) fn peek_char(&self) -> Option<char> {
        if self.closed {
            return None;
        }
        let StreamKind::Input {
            characters,
            position,
            pushback,
        } = &self.kind
        else {
            return None;
        };
        if let Some(character) = pushback {
            return Some(*character);
        }
        characters.get(*position).copied()
    }

    pub(crate) fn unread_char(&mut self, character: char) -> bool {
        if self.closed {
            return false;
        }
        let StreamKind::Input {
            characters,
            position,
            pushback,
        } = &mut self.kind
        else {
            return false;
        };
        if pushback.is_some() || *position == 0 {
            return false;
        }
        if characters.get(*position - 1).copied() != Some(character) {
            return false;
        }
        *pushback = Some(character);
        true
    }

    pub(crate) fn read_line(&mut self) -> Option<(String, bool)> {
        let first = self.read_char()?;
        let mut line = String::new();
        let mut character = first;
        loop {
            if character == '\n' {
                return Some((line, false));
            }
            line.push(character);
            match self.read_char() {
                Some(next) => character = next,
                None => return Some((line, true)),
            }
        }
    }

    pub(crate) fn write(&mut self, text: &str) -> bool {
        if self.closed {
            return false;
        }
        let StreamKind::Output {
            buffer,
            at_line_start,
        } = &mut self.kind
        else {
            return false;
        };
        buffer.push_str(text);
        if let Some(character) = text.chars().last() {
            *at_line_start = character == '\n';
        }
        true
    }

    pub(crate) fn fresh_line(&mut self) -> Option<bool> {
        if self.closed {
            return None;
        }
        let StreamKind::Output {
            buffer,
            at_line_start,
        } = &mut self.kind
        else {
            return None;
        };
        if *at_line_start {
            return Some(false);
        }
        buffer.push('\n');
        *at_line_start = true;
        Some(true)
    }

    pub(crate) fn take_output(&mut self) -> Option<String> {
        let StreamKind::Output { buffer, .. } = &mut self.kind else {
            return None;
        };
        Some(std::mem::take(buffer))
    }

    pub(crate) fn close(&mut self) {
        self.closed = true;
    }
}

#[derive(Clone)]
pub enum Value {
    Nil,
    Unbound,
    Boolean(bool),
    Integer(i64),
    Rational(Rational),
    Float(f64),
    String(Rc<str>),
    Character(char),
    Stream(Rc<RefCell<Stream>>),
    Package(Rc<str>),
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
    Vector(Rc<Vec<Value>>),
    Array {
        dimensions: Rc<Vec<usize>>,
        elements: Rc<Vec<Value>>,
    },
    HashTable {
        test: Rc<str>,
        entries: Rc<RefCell<Vec<(Value, Value)>>>,
    },
    Values(Rc<Vec<Value>>),
    Condition(Rc<str>),
    Structure {
        name: Rc<str>,
        types: Rc<Vec<Rc<str>>>,
        slots: Rc<RefCell<Vec<(Rc<str>, Value)>>>,
    },
    Class(Rc<ClassDefinition>),
    Instance(Instance),
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

    pub(crate) fn string_input_stream(source: &str, start: usize, end: usize) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::input(source, start, end))))
    }

    pub(crate) fn string_output_stream() -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::output())))
    }

    pub fn package(value: impl AsRef<str>) -> Self {
        Self::Package(Rc::from(value.as_ref()))
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
        Self::Vector(Rc::new(values))
    }

    pub fn array(dimensions: Vec<usize>, elements: Vec<Self>) -> Self {
        Self::Array {
            dimensions: Rc::new(dimensions),
            elements: Rc::new(elements),
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
        Self::Condition(Rc::from(error.to_string()))
    }

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

    pub fn closure(parameters: Vec<String>, body: Vec<Form>, environment: Environment) -> Self {
        Self::closure_with_optional(parameters, Vec::new(), None, body, environment)
    }

    pub(crate) fn closure_with_rest(
        parameters: Vec<String>,
        rest: Option<String>,
        body: Vec<Form>,
        environment: Environment,
    ) -> Self {
        Self::closure_with_optional(parameters, Vec::new(), rest, body, environment)
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
            parameters,
            required_escaped,
            optional,
            rest,
            false,
            Vec::new(),
            false,
            false,
            auxiliary,
            body,
            environment,
        )
    }

    pub(crate) fn closure_with_keywords(
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
    ) -> Self {
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

    pub(crate) fn structure(name: impl AsRef<str>, slots: Vec<(String, Value)>) -> Self {
        let name = name.as_ref().to_string();
        Self::structure_with_types(name.clone(), slots, vec![name])
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

    pub(crate) fn instance(
        definition: Rc<ClassDefinition>,
        slots: Vec<(String, Value)>,
    ) -> Self {
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

    pub(crate) fn class_definition(&self) -> Option<Rc<ClassDefinition>> {
        match self {
            Self::Class(definition) => Some(definition.clone()),
            _ => None,
        }
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

    pub(crate) fn instance_slot(&self, slot_name: &str) -> Option<Value> {
        let Self::Instance(instance) = self else {
            return None;
        };
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
            Self::String(_) => "STRING",
            Self::Character(_) => "CHARACTER",
            Self::Stream(_) => "STREAM",
            Self::Package(_) => "PACKAGE",
            Self::Symbol(_)
            | Self::SymbolExact(_)
            | Self::UninternedSymbol(_) => "SYMBOL",
            Self::Keyword(_) | Self::KeywordExact(_) => "KEYWORD",
            Self::List(_) | Self::DottedList { .. } => "LIST",
            Self::Vector(_) => "VECTOR",
            Self::Array { .. } => "ARRAY",
            Self::HashTable { .. } => "HASH-TABLE",
            Self::Values(_) => "VALUES",
            Self::Condition(_) => "CONDITION",
            Self::Structure { .. } => "STRUCTURE",
            Self::Class(_) => "CLASS",
            Self::Instance(_) => "STANDARD-OBJECT",
            Self::Function(_) => "FUNCTION",
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
            Self::Vector(items) => Some(items.as_ref().clone()),
            _ => None,
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
            (Self::List(left), Self::List(right)) | (Self::Vector(left), Self::Vector(right)) => {
                Rc::ptr_eq(left, right)
            }
            (
                Self::Array {
                    dimensions: left_dimensions,
                    elements: left_elements,
                },
                Self::Array {
                    dimensions: right_dimensions,
                    elements: right_elements,
                },
            ) => {
                Rc::ptr_eq(left_dimensions, right_dimensions)
                    && Rc::ptr_eq(left_elements, right_elements)
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
            (Self::Instance(left), Self::Instance(right)) => {
                Rc::ptr_eq(&left.class, &right.class) && Rc::ptr_eq(&left.slots, &right.slots)
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
            ) => Rc::ptr_eq(left, right) && Rc::ptr_eq(left_tail, right_tail),
            (Self::Function(left), Self::Function(right)) => Rc::ptr_eq(left, right),
            _ => false,
        }
    }

    pub fn equal_value(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::String(left), Self::String(right)) => left == right,
            (Self::List(left), Self::List(right)) | (Self::Vector(left), Self::Vector(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right.iter())
                        .all(|(left, right)| left.equal_value(right))
            }
            (
                Self::Array {
                    dimensions: left_dimensions,
                    elements: left_elements,
                },
                Self::Array {
                    dimensions: right_dimensions,
                    elements: right_elements,
                },
            ) => {
                left_dimensions == right_dimensions
                    && left_elements.len() == right_elements.len()
                    && left_elements
                        .iter()
                        .zip(right_elements.iter())
                        .all(|(left, right)| left.equal_value(right))
            }
            (Self::Values(left), Self::Values(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right.iter())
                        .all(|(left, right)| left.equal_value(right))
            }
            (Self::Condition(left), Self::Condition(right)) => left == right,
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
            (Self::Class(left), Self::Class(right)) => {
                left.name.eq_ignore_ascii_case(&right.name)
            }
            (Self::Instance(left), Self::Instance(right)) => {
                if !left.class.name.eq_ignore_ascii_case(&right.class.name) {
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
            Self::Rational(value) => write!(formatter, "{}/{}", value.numerator, value.denominator),
            Self::Float(value) => {
                if value.fract() == 0.0 {
                    write!(formatter, "{value:.1}")
                } else {
                    value.fmt(formatter)
                }
            }
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
            Self::Vector(values) => {
                formatter.write_str("#(")?;
                write_sequence(formatter, values)?;
                formatter.write_str(")")
            }
            Self::Array { dimensions, .. } => write!(formatter, "#<ARRAY {dimensions:?}>"),
            Self::HashTable { test, .. } => write!(formatter, "#<HASH-TABLE {test}>"),
            Self::Values(values) => {
                formatter.write_str("#<VALUES")?;
                if !values.is_empty() {
                    formatter.write_str(" ")?;
                    write_sequence(formatter, values)?;
                }
                formatter.write_str(">")
            }
            Self::Condition(message) => write!(formatter, "#<CONDITION {message}>"),
            Self::Structure { name, slots, .. } => {
                write!(formatter, "#S({name}")?;
                for (slot_name, value) in slots.borrow().iter() {
                    write!(formatter, " :{slot_name} {value}")?;
                }
                formatter.write_char(')')
            }
            Self::Class(definition) => write!(formatter, "#<CLASS {}>", definition.name),
            Self::Instance(instance) => write!(formatter, "#<{} INSTANCE>", instance.class.name),
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
                Function::Closure { .. } | Function::Compiled { .. } => {
                    formatter.write_str("#<FUNCTION>")
                }
                Function::Macro { .. } => formatter.write_str("#<MACRO>"),
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
