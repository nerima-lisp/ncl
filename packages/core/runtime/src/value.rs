use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use ncl_compiler::{FunctionId, Program};
use ncl_syntax::{
    Form, LambdaListAuxiliaryParameter, LambdaListKeywordParameter, LambdaListOptionalParameter,
    OrdinaryLambdaList,
};

use crate::environment::Environment;
use crate::error::{ReturnValue, RuntimeError};

#[path = "rational.rs"]
mod rational;
pub use rational::Rational;

#[path = "value_display.rs"]
mod display;

pub type Builtin = fn(&[Value]) -> Result<Value, RuntimeError>;
type SlotValues = Rc<RefCell<Vec<(Rc<str>, Value)>>>;
type ValueEntries = Rc<RefCell<Vec<(Value, Value)>>>;

pub(crate) struct ClosureSpec {
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
}

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
    pub(crate) class_value: Option<Rc<RefCell<Value>>>,
}

#[derive(Clone)]
pub struct ClassDefinition {
    pub(crate) name: String,
    pub(crate) precedence: Vec<String>,
    pub(crate) slots: Vec<ClassSlot>,
    pub(crate) default_initargs: Vec<(String, Form)>,
}

#[derive(Clone)]
pub struct MethodDefinition {
    pub(crate) qualifiers: Vec<String>,
    pub(crate) specializers: Vec<String>,
    pub(crate) function: Value,
}

#[derive(Clone)]
pub struct Instance {
    pub(crate) class: Rc<ClassDefinition>,
    pub(crate) slots: SlotValues,
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

pub struct Stream {
    kind: StreamKind,
    closed: bool,
}

enum StreamKind {
    Input {
        characters: Rc<Vec<char>>,
        position: usize,
        pushback: Option<char>,
        file: bool,
    },
    Io {
        characters: Vec<char>,
        position: usize,
        pushback: Option<char>,
        at_line_start: bool,
        file_path: Rc<PathBuf>,
    },
    Output {
        buffer: String,
        at_line_start: bool,
        file_path: Option<Rc<PathBuf>>,
    },
}

impl Stream {
    fn input(source: &str, start: usize, end: usize) -> Self {
        Self {
            kind: StreamKind::Input {
                characters: Rc::new(source.chars().skip(start).take(end - start).collect()),
                position: 0,
                pushback: None,
                file: false,
            },
            closed: false,
        }
    }

    fn file_input(source: String) -> Self {
        Self {
            kind: StreamKind::Input {
                characters: Rc::new(source.chars().collect()),
                position: 0,
                pushback: None,
                file: true,
            },
            closed: false,
        }
    }

    fn file_io(path: PathBuf, source: String, append: bool) -> Self {
        let characters: Vec<char> = source.chars().collect();
        let position = if append { characters.len() } else { 0 };
        let at_line_start = if position == 0 {
            true
        } else {
            characters.get(position - 1) == Some(&'\n')
        };
        Self {
            kind: StreamKind::Io {
                characters,
                position,
                pushback: None,
                at_line_start,
                file_path: Rc::new(path),
            },
            closed: false,
        }
    }

    fn output() -> Self {
        Self {
            kind: StreamKind::Output {
                buffer: String::new(),
                at_line_start: true,
                file_path: None,
            },
            closed: false,
        }
    }

    fn file_output(path: PathBuf, initial: String) -> Self {
        let at_line_start = initial.ends_with('\n');
        Self {
            kind: StreamKind::Output {
                buffer: initial,
                at_line_start,
                file_path: Some(Rc::new(path)),
            },
            closed: false,
        }
    }

    pub(crate) fn kind_name(&self) -> &'static str {
        match &self.kind {
            StreamKind::Input { file, .. } => {
                if *file {
                    "FILE-INPUT-STREAM"
                } else {
                    "STRING-INPUT-STREAM"
                }
            }
            StreamKind::Io { .. } => "FILE-IO-STREAM",
            StreamKind::Output { file_path, .. } => {
                if file_path.is_some() {
                    "FILE-OUTPUT-STREAM"
                } else {
                    "STRING-OUTPUT-STREAM"
                }
            }
        }
    }

    pub(crate) fn is_input(&self) -> bool {
        matches!(&self.kind, StreamKind::Input { .. } | StreamKind::Io { .. })
    }

    pub(crate) fn is_output(&self) -> bool {
        matches!(
            &self.kind,
            StreamKind::Output { .. } | StreamKind::Io { .. }
        )
    }

    pub(crate) fn read_char(&mut self) -> Option<char> {
        if self.closed {
            return None;
        }
        match &mut self.kind {
            StreamKind::Input {
                characters,
                position,
                pushback,
                ..
            } => {
                if let Some(character) = pushback.take() {
                    return Some(character);
                }
                let character = characters.get(*position).copied()?;
                *position += 1;
                Some(character)
            }
            StreamKind::Io {
                characters,
                position,
                pushback,
                ..
            } => {
                if let Some(character) = pushback.take() {
                    return Some(character);
                }
                let character = characters.get(*position).copied()?;
                *position += 1;
                Some(character)
            }
            StreamKind::Output { .. } => None,
        }
    }

    pub(crate) fn peek_char(&self) -> Option<char> {
        if self.closed {
            return None;
        }
        match &self.kind {
            StreamKind::Input {
                characters,
                position,
                pushback,
                ..
            } => {
                if let Some(character) = pushback {
                    return Some(*character);
                }
                characters.get(*position).copied()
            }
            StreamKind::Io {
                characters,
                position,
                pushback,
                ..
            } => {
                if let Some(character) = pushback {
                    return Some(*character);
                }
                characters.get(*position).copied()
            }
            StreamKind::Output { .. } => None,
        }
    }

    pub(crate) fn unread_char(&mut self, character: char) -> bool {
        if self.closed {
            return false;
        }
        match &mut self.kind {
            StreamKind::Input {
                characters,
                position,
                pushback,
                ..
            } => {
                if pushback.is_some() || *position == 0 {
                    return false;
                }
                if characters.get(*position - 1).copied() != Some(character) {
                    return false;
                }
                *pushback = Some(character);
                true
            }
            StreamKind::Io {
                characters,
                position,
                pushback,
                ..
            } => {
                if pushback.is_some() || *position == 0 {
                    return false;
                }
                if characters.get(*position - 1).copied() != Some(character) {
                    return false;
                }
                *pushback = Some(character);
                true
            }
            StreamKind::Output { .. } => false,
        }
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

    pub(crate) fn remaining_input(&self) -> Option<String> {
        if self.closed {
            return None;
        }
        match &self.kind {
            StreamKind::Input {
                characters,
                position,
                pushback,
                ..
            } => {
                let mut source = String::new();
                if let Some(character) = pushback {
                    source.push(*character);
                }
                source.extend(characters.iter().skip(*position).copied());
                Some(source)
            }
            StreamKind::Io {
                characters,
                position,
                pushback,
                ..
            } => {
                let mut source = String::new();
                if let Some(character) = pushback {
                    source.push(*character);
                }
                source.extend(characters.iter().skip(*position).copied());
                Some(source)
            }
            StreamKind::Output { .. } => None,
        }
    }

    pub(crate) fn consume_input(&mut self, count: usize) -> bool {
        if self.closed {
            return false;
        }
        match &mut self.kind {
            StreamKind::Input {
                characters,
                position,
                pushback,
                ..
            } => {
                let available =
                    usize::from(pushback.is_some()) + characters.len().saturating_sub(*position);
                if count > available {
                    return false;
                }
                if count == 0 {
                    return true;
                }
                let mut remaining = count;
                if pushback.take().is_some() {
                    remaining -= 1;
                }
                *position += remaining;
                true
            }
            StreamKind::Io {
                characters,
                position,
                pushback,
                ..
            } => {
                let available =
                    usize::from(pushback.is_some()) + characters.len().saturating_sub(*position);
                if count > available {
                    return false;
                }
                if count == 0 {
                    return true;
                }
                let mut remaining = count;
                if pushback.take().is_some() {
                    remaining -= 1;
                }
                *position += remaining;
                true
            }
            StreamKind::Output { .. } => false,
        }
    }

    pub(crate) fn write(&mut self, text: &str) -> bool {
        if self.closed {
            return false;
        }
        match &mut self.kind {
            StreamKind::Output {
                buffer,
                at_line_start,
                ..
            } => {
                buffer.push_str(text);
                if let Some(character) = text.chars().last() {
                    *at_line_start = character == '\n';
                }
                true
            }
            StreamKind::Io {
                characters,
                position,
                pushback,
                at_line_start,
                ..
            } => {
                pushback.take();
                for character in text.chars() {
                    if *position < characters.len() {
                        characters[*position] = character;
                    } else {
                        characters.push(character);
                    }
                    *position += 1;
                }
                if let Some(character) = text.chars().last() {
                    *at_line_start = character == '\n';
                }
                true
            }
            StreamKind::Input { .. } => false,
        }
    }

    pub(crate) fn fresh_line(&mut self) -> Option<bool> {
        if self.closed {
            return None;
        }
        let at_line_start = match &self.kind {
            StreamKind::Output { at_line_start, .. } | StreamKind::Io { at_line_start, .. } => {
                *at_line_start
            }
            StreamKind::Input { .. } => return None,
        };
        if at_line_start {
            return Some(false);
        }
        if self.write("\n") { Some(true) } else { None }
    }

    pub(crate) fn take_output(&mut self) -> Option<String> {
        let StreamKind::Output {
            buffer,
            file_path: None,
            ..
        } = &mut self.kind
        else {
            return None;
        };
        Some(std::mem::take(buffer))
    }

    pub(crate) fn close(&mut self, abort: bool) -> Result<(), std::io::Error> {
        if self.closed {
            return Ok(());
        }
        if !abort {
            if let StreamKind::Output {
                buffer,
                file_path: Some(path),
                ..
            } = &self.kind
            {
                std::fs::write(path.as_ref(), buffer.as_bytes())?;
            }
            if let StreamKind::Io {
                characters,
                file_path,
                ..
            } = &self.kind
            {
                let source: String = characters.iter().collect();
                std::fs::write(file_path.as_ref(), source.as_bytes())?;
            }
        }
        self.closed = true;
        Ok(())
    }
}

#[derive(Clone)]
pub struct ConditionData {
    actual_type: String,
    type_names: Rc<Vec<String>>,
    slots: SlotValues,
    message: Rc<str>,
    format_control: Option<Rc<str>>,
    format_arguments: Vec<Value>,
}

impl ConditionData {
    fn equal_value(&self, other: &Self) -> bool {
        self.actual_type == other.actual_type
            && self.type_names == other.type_names
            && self.message == other.message
            && self.format_control == other.format_control
            && self.format_arguments.len() == other.format_arguments.len()
            && self
                .format_arguments
                .iter()
                .zip(other.format_arguments.iter())
                .all(|(left, right)| left.equal_value(right))
            && {
                let left_slots = self.slots.borrow();
                let right_slots = other.slots.borrow();
                left_slots.len() == right_slots.len()
                    && left_slots.iter().zip(right_slots.iter()).all(
                        |((left_name, left_value), (right_name, right_value))| {
                            left_name == right_name && left_value.equal_value(right_value)
                        },
                    )
            }
    }
}

#[derive(Clone)]
pub struct RestartData {
    name: Rc<str>,
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
    Vector(Rc<Vec<Value>>),
    Array {
        dimensions: Rc<Vec<usize>>,
        elements: Rc<Vec<Value>>,
    },
    HashTable {
        test: Rc<str>,
        entries: ValueEntries,
    },
    Values(Rc<Vec<Value>>),
    Condition(Rc<ConditionData>),
    Restart(Rc<RestartData>),
    Structure {
        name: Rc<str>,
        types: Rc<Vec<Rc<str>>>,
        slots: SlotValues,
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
        let (actual_type, type_names, message, format_control, format_arguments) = match error {
            RuntimeError::Signaled(signaled) => (
                error.condition_type_name(),
                if signaled.condition_types.is_empty() {
                    vec![signaled.condition.clone()]
                } else {
                    signaled.condition_types.clone()
                },
                signaled.message.clone(),
                signaled.format_control.clone(),
                signaled
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
            ClosureSpec {
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
            },
            environment,
        )
    }

    pub(crate) fn closure_with_keywords(spec: ClosureSpec, environment: Environment) -> Self {
        Self::Function(Rc::new(Function::Closure {
            parameters: spec.parameters,
            required_escaped: spec.required_escaped,
            optional: spec.optional,
            rest: spec.rest,
            rest_escaped: spec.rest_escaped,
            keywords: spec.keywords,
            has_keyword_section: spec.has_keyword_section,
            allow_other_keys: spec.allow_other_keys,
            auxiliary: spec.auxiliary,
            body: spec.body,
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

    pub(crate) fn instance_slot(&self, slot_name: &str) -> Option<Value> {
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
        match condition.actual_type.to_ascii_uppercase().as_str() {
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
            Self::Restart(restart) => Some(restart.name.as_ref()),
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

#[cfg(test)]
mod tests {
    use super::{Rational, Stream, Value};
    use crate::error::RuntimeError;
    use std::rc::Rc;

    #[test]
    fn boolean_and_multiple_values_follow_truthiness_rules() {
        assert!(Value::boolean(true).is_truthy());
        assert!(!Value::boolean(false).is_truthy());
        assert!(!Value::Nil.is_truthy());
        assert!(Value::Values(Rc::new(vec![Value::Integer(1), Value::Nil])).is_truthy());
        assert!(!Value::Values(Rc::new(Vec::new())).is_truthy());
    }

    #[test]
    fn type_names_cover_scalar_and_collection_values() {
        let cases = [
            (Value::Nil, "NIL"),
            (Value::Unbound, "UNBOUND"),
            (Value::Boolean(true), "BOOLEAN"),
            (Value::Integer(1), "INTEGER"),
            (Value::Float(1.0), "FLOAT"),
            (Value::string("text"), "STRING"),
            (Value::Character('x'), "CHARACTER"),
            (Value::List(Rc::new(Vec::new())), "LIST"),
            (
                Value::DottedList {
                    items: Rc::new(Vec::new()),
                    tail: Rc::new(Value::Nil),
                },
                "LIST",
            ),
            (Value::Vector(Rc::new(Vec::new())), "VECTOR"),
            (
                Value::Array {
                    dimensions: Rc::new(vec![0]),
                    elements: Rc::new(Vec::new()),
                },
                "ARRAY",
            ),
            (Value::Values(Rc::new(Vec::new())), "VALUES"),
        ];
        for (value, expected) in cases {
            assert_eq!(value.type_name(), expected);
        }
    }

    #[test]
    fn collection_accessors_preserve_shape_and_reject_other_values() {
        let items = vec![Value::Integer(1), Value::Integer(2)];
        let list = Value::List(Rc::new(items.clone()));
        assert_eq!(list.list_items().as_ref().map(Vec::len), Some(2));
        assert_eq!(Value::Nil.list_items().as_ref().map(Vec::len), Some(0));
        assert!(Value::Integer(1).list_items().is_none());

        let vector = Value::Vector(Rc::new(items));
        assert_eq!(vector.vector_items().as_ref().map(Vec::len), Some(2));
        assert!(list.vector_items().is_none());

        let array = Value::Array {
            dimensions: Rc::new(vec![2, 1]),
            elements: Rc::new(vec![Value::Nil, Value::Nil]),
        };
        assert_eq!(array.array_dimensions(), Some(vec![2, 1]));
        assert_eq!(array.array_items().as_ref().map(Vec::len), Some(2));
        assert!(vector.array_items().is_none());
    }

    #[test]
    fn symbol_names_and_references_preserve_exactness() {
        let cases = [
            (Value::Nil, Some(("NIL", false))),
            (Value::Boolean(true), Some(("T", false))),
            (Value::Symbol(Rc::from("name")), Some(("name", false))),
            (Value::SymbolExact(Rc::from("name")), Some(("name", true))),
            (Value::Integer(1), None),
        ];
        for (value, expected) in cases {
            assert_eq!(value.symbol_reference(), expected);
            assert_eq!(value.symbol_name(), expected.map(|(name, _)| name));
        }
    }

    #[test]
    fn eq_is_identity_based_while_equal_is_structural() {
        let left = Value::List(Rc::new(vec![Value::string("same")]));
        let right = Value::List(Rc::new(vec![Value::string("same")]));
        assert!(!left.eq_value(&right));
        assert!(left.equal_value(&right));
        assert!(Value::Nil.eq_value(&Value::Boolean(false)));
        assert!(!Value::Integer(1).equal_value(&Value::Integer(2)));
    }

    #[test]
    fn input_stream_preserves_peek_pushback_and_consumption_semantics() {
        let mut stream = Stream::input("ab\ncd", 0, 5);
        assert_eq!(stream.kind_name(), "STRING-INPUT-STREAM");
        assert!(stream.is_input());
        assert!(!stream.is_output());
        assert_eq!(stream.peek_char(), Some('a'));
        assert_eq!(stream.read_char(), Some('a'));
        assert!(stream.unread_char('a'));
        assert!(!stream.unread_char('a'));
        assert_eq!(stream.remaining_input(), Some("ab\ncd".to_owned()));
        assert!(stream.consume_input(2));
        assert_eq!(stream.peek_char(), Some('\n'));
        assert_eq!(stream.read_line(), Some((String::new(), false)));
        assert_eq!(stream.read_line(), Some(("cd".to_owned(), true)));
        assert_eq!(stream.read_char(), None);
        assert!(!stream.consume_input(1));
    }

    #[test]
    fn output_stream_tracks_line_state_and_can_be_drained() {
        let mut stream = Stream::output();
        assert_eq!(stream.kind_name(), "STRING-OUTPUT-STREAM");
        assert!(!stream.is_input());
        assert!(stream.is_output());
        assert_eq!(stream.fresh_line(), Some(false));
        assert!(stream.write("hello"));
        assert_eq!(stream.fresh_line(), Some(true));
        assert_eq!(stream.fresh_line(), Some(false));
        assert_eq!(stream.take_output(), Some("hello\n".to_owned()));
        assert_eq!(stream.take_output(), Some(String::new()));
        stream.close(false).expect("string output closes");
        assert!(!stream.write("closed later"));
        assert_eq!(stream.fresh_line(), None);
    }

    #[test]
    fn rational_constructor_reduces_integers_and_rejects_zero_denominator() {
        let cases = [
            ((6, 3), Value::Integer(2)),
            ((-6, 3), Value::Integer(-2)),
            ((6, -3), Value::Integer(-2)),
            (
                (2, 4),
                Value::Rational(Rational::new(1, 2).expect("reduced ratio")),
            ),
        ];

        for ((numerator, denominator), expected) in cases {
            assert!(
                Value::rational(numerator, denominator)
                    .expect("valid ratio")
                    .equal_value(&expected)
            );
        }

        for (numerator, denominator) in [(1, 0), (i128::MIN, 1), (1, i128::MIN)] {
            assert!(
                Value::rational(numerator, denominator).is_err(),
                "ratio {numerator}/{denominator} must be rejected"
            );
        }
    }

    #[test]
    fn constructors_normalize_symbols_keywords_and_collections() {
        assert!(matches!(Value::boolean(true), Value::Boolean(true)));
        assert!(matches!(Value::boolean(false), Value::Nil));
        assert!(matches!(Value::symbol("name"), Value::Symbol(name) if name.as_ref() == "NAME"));
        assert!(
            matches!(Value::symbol_exact("name"), Value::SymbolExact(name) if name.as_ref() == "name")
        );
        assert!(matches!(Value::keyword(":name"), Value::Keyword(name) if name.as_ref() == "NAME"));
        assert!(
            matches!(Value::keyword_exact(":name"), Value::KeywordExact(name) if name.as_ref() == "name")
        );
        assert!(matches!(Value::list(Vec::new()), Value::Nil));
        assert!(
            Value::list(vec![Value::Integer(1)])
                .list_items()
                .expect("list items")[0]
                .equal_value(&Value::Integer(1))
        );
        assert!(
            Value::vector(vec![Value::Integer(1)])
                .vector_items()
                .expect("vector items")[0]
                .equal_value(&Value::Integer(1))
        );
        assert_eq!(
            Value::array(vec![1, 2], vec![Value::Integer(1), Value::Integer(2)]).array_dimensions(),
            Some(vec![1, 2])
        );
    }

    #[test]
    fn conditions_preserve_hierarchy_slots_and_format_metadata() {
        let condition = Value::condition_from_parts_with_types(
            "SIMPLE-ERROR".to_owned(),
            vec!["SIMPLE-ERROR".to_owned()],
            vec![("datum".to_owned(), Value::Integer(1))],
            "bad value".to_owned(),
            Some("~A".to_owned()),
            vec![Value::Integer(7)],
        );

        assert!(condition.condition_is_type(":error"));
        assert!(condition.condition_is_type("SERIOUS-CONDITION"));
        assert!(condition.condition_is_type("condition"));
        assert!(!condition.condition_is_type("warning"));
        assert!(!Value::Nil.condition_is_type("condition"));

        let lowercase = Value::condition_from_parts(
            "simple-error".to_owned(),
            "lowercase".to_owned(),
            None,
            Vec::new(),
        );
        assert!(lowercase.condition_is_type("serious-condition"));
        assert_eq!(condition.condition_type_name(), Some("SIMPLE-ERROR"));
        assert_eq!(condition.condition_message(), Some("bad value"));
        assert_eq!(condition.simple_condition_format_control(), Some("~A"));
        assert!(matches!(
            condition.simple_condition_format_arguments(),
            Some(arguments) if arguments.len() == 1 && arguments[0].equal_value(&Value::Integer(7))
        ));

        assert!(!condition.set_condition_slot("warning", "datum", Value::Nil));
        assert!(condition.set_condition_slot("error", "datum", Value::Integer(2)));
        assert!(matches!(
            condition.condition_slot("ERROR", "DATUM"),
            Some(value) if value.equal_value(&Value::Integer(2))
        ));
        assert!(!condition.set_condition_slot("error", "missing", Value::Nil));

        let division = Value::condition(&RuntimeError::DivisionByZero);
        assert!(division.condition_is_type("arithmetic-error"));
        assert!(division.condition_is_type("error"));
        assert_eq!(division.condition_message(), Some("division by zero"));
        assert_eq!(Value::Nil.condition_message(), None);
    }
}
