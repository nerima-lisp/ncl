use std::cell::RefCell;
use std::fmt;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::rc::Rc;

use ncl_compiler::{FunctionId, Program};
use ncl_syntax::{
    Form, LambdaListAuxiliaryParameter, LambdaListKeywordParameter, LambdaListOptionalParameter,
    OrdinaryLambdaList,
};

use crate::environment::Environment;
use crate::error::{ReturnValue, RuntimeError};

#[path = "value/stream.rs"]
mod stream;

#[path = "value/display.rs"]
mod display;

pub use stream::Stream;

pub type Builtin = fn(&[Value]) -> Result<Value, RuntimeError>;

#[derive(Clone)]
pub(crate) struct MacroBinding {
    pub(crate) name: String,
    pub(crate) escaped: bool,
}

#[derive(Clone)]
pub(crate) enum MacroPattern {
    Name(MacroBinding),
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
    pub(crate) supplied_p: Option<MacroBinding>,
}

#[derive(Clone)]
pub(crate) struct MacroKeywordParameter {
    pub(crate) keyword_name: String,
    pub(crate) keyword_name_escaped: bool,
    pub(crate) pattern: MacroPattern,
    pub(crate) init_form: Form,
    pub(crate) supplied_p: Option<MacroBinding>,
}

#[derive(Clone)]
pub(crate) struct MacroAuxiliaryParameter {
    pub(crate) name: MacroBinding,
    pub(crate) init_form: Form,
}

#[derive(Clone)]
pub(crate) struct MacroLambdaList {
    pub(crate) whole: Option<MacroBinding>,
    pub(crate) environment: Option<MacroBinding>,
    pub(crate) required: Vec<MacroPattern>,
    pub(crate) optional: Vec<MacroOptionalParameter>,
    pub(crate) rest: Option<MacroBinding>,
    pub(crate) keywords: Vec<MacroKeywordParameter>,
    pub(crate) has_keyword_section: bool,
    pub(crate) allow_other_keys: bool,
    pub(crate) auxiliary: Vec<MacroAuxiliaryParameter>,
}

#[derive(Clone)]
pub(crate) struct DefsetfDefinition {
    pub(crate) lambda_list: MacroLambdaList,
    pub(crate) stores: Vec<MacroBinding>,
    pub(crate) body: Vec<Form>,
    pub(crate) environment: Environment,
}

#[derive(Clone)]
pub(crate) struct StructureSlot {
    pub(crate) name: String,
    pub(crate) init_form: Option<Form>,
    pub(crate) read_only: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum StructureRepresentation {
    Structure,
    List,
    Vector,
}

#[derive(Clone)]
pub(crate) struct StructureDefinition {
    pub(crate) documentation: Option<String>,
    pub(crate) slots: Vec<StructureSlot>,
    pub(crate) type_names: Vec<String>,
    pub(crate) representation: StructureRepresentation,
    pub(crate) named: bool,
}

#[derive(Clone)]
pub(crate) struct ConditionSlot {
    pub(crate) name: String,
    pub(crate) initarg: Option<ConditionInitarg>,
    pub(crate) init_form: Option<Form>,
    pub(crate) readers: Vec<String>,
    pub(crate) writers: Vec<String>,
}

#[derive(Clone)]
pub(crate) struct ConditionInitarg {
    pub(crate) name: String,
    pub(crate) escaped: bool,
}

impl ConditionInitarg {
    pub(crate) fn matches(&self, name: &str, escaped: bool) -> bool {
        self.escaped == escaped
            && if escaped {
                self.name == name
            } else {
                self.name.eq_ignore_ascii_case(name)
            }
    }
}

#[derive(Clone)]
pub(crate) struct ConditionDefinition {
    pub(crate) name: String,
    pub(crate) direct_superclasses: Vec<String>,
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
    pub(crate) type_specifier: Option<Value>,
}

#[derive(Clone)]
pub(crate) struct ClassDefinition {
    pub(crate) name: String,
    pub(crate) direct_superclasses: Vec<String>,
    pub(crate) precedence: Vec<String>,
    pub(crate) slots: Vec<ClassSlot>,
    pub(crate) default_initargs: Vec<(String, Form)>,
    pub(crate) documentation: Option<String>,
}

#[derive(Clone)]
pub(crate) enum MethodSpecializer {
    Type(String),
    Eql(Value),
}

#[derive(Clone)]
pub(crate) struct MethodDefinition {
    pub(crate) qualifiers: Vec<String>,
    pub(crate) specializers: Vec<MethodSpecializer>,
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
        representation: StructureRepresentation,
        named: bool,
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
        documentation: Option<String>,
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
        documentation: Option<String>,
    },
}

impl Function {
    pub(crate) fn documentation(&self) -> Option<&str> {
        match self {
            Self::Closure { documentation, .. } | Self::Compiled { documentation, .. } => {
                documentation.as_deref()
            }
            _ => None,
        }
    }
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
                numerator
                    .checked_neg()
                    .ok_or(RuntimeError::NumericOverflow)?,
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

#[derive(Clone)]
pub(crate) struct ConditionData {
    actual_type: String,
    type_names: Rc<Vec<String>>,
    slots: Rc<RefCell<Vec<(Rc<str>, Value)>>>,
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
pub(crate) struct RestartData {
    name: Rc<str>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArrayElementType {
    T,
    Character,
    Bit,
}

impl ArrayElementType {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::T => "T",
            Self::Character => "CHARACTER",
            Self::Bit => "BIT",
        }
    }
}

pub(crate) struct HashTableIteratorState {
    entries: Vec<(Value, Value)>,
    index: usize,
}

#[derive(Clone)]
pub(crate) struct RandomState {
    state: u64,
}

impl RandomState {
    const NONZERO_SEED: u64 = 0x9E3779B97F4A7C15;

    pub(crate) fn seeded() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};

        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos() as u64);
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        Self::from_seed(time ^ counter.wrapping_mul(Self::NONZERO_SEED))
    }

    fn from_seed(seed: u64) -> Self {
        Self {
            state: if seed == 0 { Self::NONZERO_SEED } else { seed },
        }
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        let mut state = self.state;
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        self.state = state;
        state.wrapping_mul(2_685_829_013_198_858_977)
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
    Complex {
        real: Box<Value>,
        imaginary: Box<Value>,
    },
    String(Rc<str>),
    Character(char),
    Stream(Rc<RefCell<Stream>>),
    RandomState(Rc<RefCell<RandomState>>),
    Package(Rc<str>),
    Environment(Environment),
    Symbol(Rc<str>),
    SymbolExact(Rc<str>),
    QualifiedSymbolExact {
        reference: Rc<str>,
        package_len: usize,
    },
    UninternedSymbol(Rc<str>),
    Keyword(Rc<str>),
    KeywordExact(Rc<str>),
    List(Rc<Vec<Value>>),
    DottedList {
        items: Rc<Vec<Value>>,
        tail: Rc<Value>,
    },
    Vector(Rc<RefCell<Vec<Value>>>),
    Array {
        dimensions: Rc<Vec<usize>>,
        elements: Rc<RefCell<Vec<Value>>>,
        element_type: ArrayElementType,
        fill_pointer: Option<Rc<RefCell<usize>>>,
        adjustable: bool,
    },
    HashTable {
        test: Rc<str>,
        entries: Rc<RefCell<Vec<(Value, Value)>>>,
        size: usize,
        rehash_size: f64,
        rehash_threshold: f64,
        synchronized: bool,
    },
    HashTableIterator(Rc<RefCell<HashTableIteratorState>>),
    Values(Rc<Vec<Value>>),
    Condition(Rc<ConditionData>),
    Restart(Rc<RestartData>),
    Structure {
        name: Rc<str>,
        types: Rc<Vec<Rc<str>>>,
        slots: Rc<RefCell<Vec<(Rc<str>, Value)>>>,
        representation: StructureRepresentation,
        named: bool,
        marker: Option<Rc<RefCell<Value>>>,
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

    pub(crate) fn complex(real: Value, imaginary: Value) -> Result<Self, RuntimeError> {
        if !real.is_real_number() || !imaginary.is_real_number() {
            return Err(RuntimeError::Type {
                expected: "REAL".to_string(),
                actual: format!("{} and {}", real.type_name(), imaginary.type_name()),
                span: None,
            });
        }
        if imaginary.is_numeric_zero() {
            return Ok(real);
        }
        Ok(Self::Complex {
            real: Box::new(real),
            imaginary: Box::new(imaginary),
        })
    }

    pub(crate) fn is_real_number(&self) -> bool {
        matches!(self, Self::Integer(_) | Self::Rational(_) | Self::Float(_))
    }

    pub(crate) fn is_numeric_zero(&self) -> bool {
        match self {
            Self::Integer(value) => *value == 0,
            Self::Rational(value) => value.numerator() == 0,
            Self::Float(value) => *value == 0.0,
            _ => false,
        }
    }

    pub(crate) fn string_input_stream(source: &str, start: usize, end: usize) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::input(source, start, end))))
    }

    pub(crate) fn random_state(state: RandomState) -> Self {
        Self::RandomState(Rc::new(RefCell::new(state)))
    }

    pub(crate) fn random_state_value(state: Rc<RefCell<RandomState>>) -> Self {
        Self::RandomState(state)
    }

    pub(crate) fn random_state_reference(&self) -> Option<Rc<RefCell<RandomState>>> {
        match self {
            Self::RandomState(state) => Some(Rc::clone(state)),
            _ => None,
        }
    }

    pub(crate) fn string_output_stream() -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::output())))
    }

    pub(crate) fn file_input_stream(source: String) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::file_input(source))))
    }

    pub(crate) fn file_probe_stream() -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::file_probe())))
    }

    pub(crate) fn file_output_stream(path: PathBuf, initial: String, append: bool) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::file_output(
            path, initial, append,
        ))))
    }

    pub(crate) fn file_io_stream(path: PathBuf, source: String, append: bool) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::file_io(path, source, append))))
    }

    pub(crate) fn two_way_stream(input: Rc<RefCell<Stream>>, output: Rc<RefCell<Stream>>) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::two_way(input, output))))
    }

    pub(crate) fn broadcast_stream(streams: Vec<Rc<RefCell<Stream>>>) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::broadcast(streams))))
    }

    pub(crate) fn concatenated_stream(streams: Vec<Rc<RefCell<Stream>>>) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::concatenated(streams))))
    }

    pub(crate) fn echo_stream(input: Rc<RefCell<Stream>>, output: Rc<RefCell<Stream>>) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::echo(input, output))))
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

    pub fn qualified_symbol_exact(package: impl AsRef<str>, name: impl AsRef<str>) -> Self {
        let package = package.as_ref();
        let name = name.as_ref();
        Self::QualifiedSymbolExact {
            reference: Rc::from(format!("{package}::{name}")),
            package_len: package.len(),
        }
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
        Self::Vector(Rc::new(RefCell::new(values)))
    }

    pub fn array(dimensions: Vec<usize>, elements: Vec<Self>) -> Self {
        Self::array_with_element_type(dimensions, elements, ArrayElementType::T)
    }

    pub(crate) fn array_with_element_type(
        dimensions: Vec<usize>,
        elements: Vec<Self>,
        element_type: ArrayElementType,
    ) -> Self {
        Self::array_with_options(dimensions, elements, element_type, None, false)
    }

    pub(crate) fn array_with_options(
        dimensions: Vec<usize>,
        elements: Vec<Self>,
        element_type: ArrayElementType,
        fill_pointer: Option<usize>,
        adjustable: bool,
    ) -> Self {
        Self::Array {
            dimensions: Rc::new(dimensions),
            elements: Rc::new(RefCell::new(elements)),
            element_type,
            fill_pointer: fill_pointer.map(|value| Rc::new(RefCell::new(value))),
            adjustable,
        }
    }

    pub(crate) fn hash_table(test: impl AsRef<str>) -> Self {
        Self::hash_table_with_options(test, 16, 1.5, 0.75, false)
    }

    pub(crate) fn hash_table_with_options(
        test: impl AsRef<str>,
        size: usize,
        rehash_size: f64,
        rehash_threshold: f64,
        synchronized: bool,
    ) -> Self {
        Self::HashTable {
            test: Rc::from(test.as_ref()),
            entries: Rc::new(RefCell::new(Vec::new())),
            size,
            rehash_size,
            rehash_threshold,
            synchronized,
        }
    }

    pub(crate) fn hash_table_iterator(table: &Self) -> Option<Self> {
        let entries = table.hash_table_entries()?.borrow().clone();
        Some(Self::HashTableIterator(Rc::new(RefCell::new(
            HashTableIteratorState { entries, index: 0 },
        ))))
    }

    pub(crate) fn hash_table_iterator_next(&self) -> Option<Self> {
        let Self::HashTableIterator(state) = self else {
            return None;
        };
        let mut state = state.borrow_mut();
        let index = state.index;
        let Some((key, value)) = state.entries.get(index).cloned() else {
            return Some(Self::Nil);
        };
        state.index += 1;
        Some(Self::values(vec![Self::boolean(true), key, value]))
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
                    condition_types.clone()
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

    pub(crate) fn condition_from_parts_with_slots(
        actual_type: String,
        slots: Vec<(String, Value)>,
        message: String,
        format_control: Option<String>,
        format_arguments: Vec<Value>,
    ) -> Self {
        Self::condition_from_parts_with_types(
            actual_type.clone(),
            vec![actual_type],
            slots,
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
        Self::closure_with_keywords_and_documentation(
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
            None,
        )
    }

    pub(crate) fn closure_with_keywords_and_documentation(
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
        documentation: Option<String>,
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
            documentation,
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
        let documentation = program
            .functions
            .get(function)
            .and_then(|function| function.documentation.clone());
        Self::Function(Rc::new(Function::Compiled {
            program,
            function,
            environment,
            documentation,
        }))
    }

    pub(crate) fn structure(name: impl AsRef<str>, slots: Vec<(String, Value)>) -> Self {
        let name = name.as_ref().to_string();
        Self::structure_with_types(name.clone(), slots, vec![name])
    }

    pub(crate) fn structure_with_types(
        name: impl AsRef<str>,
        slots: Vec<(String, Value)>,
        type_names: Vec<String>,
    ) -> Self {
        Self::structure_with_representation(
            name,
            slots,
            type_names,
            StructureRepresentation::Structure,
            false,
        )
    }

    pub(crate) fn structure_with_representation(
        name: impl AsRef<str>,
        slots: Vec<(String, Value)>,
        mut type_names: Vec<String>,
        representation: StructureRepresentation,
        named: bool,
    ) -> Self {
        let name = name.as_ref().to_string();
        if !type_names
            .iter()
            .any(|type_name| type_name.eq_ignore_ascii_case(&name))
        {
            type_names.insert(0, name.clone());
        }
        let marker = (named && representation != StructureRepresentation::Structure)
            .then(|| Rc::new(RefCell::new(Self::symbol(&name))));
        Self::Structure {
            name: Rc::from(name),
            types: Rc::new(type_names.into_iter().map(Rc::<str>::from).collect()),
            slots: Rc::new(RefCell::new(
                slots
                    .into_iter()
                    .map(|(slot_name, value)| (Rc::from(slot_name), value))
                    .collect(),
            )),
            representation,
            named,
            marker,
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
        if let Some(slot) = instance
            .class
            .slots
            .iter()
            .find(|slot| slot.name.eq_ignore_ascii_case(slot_name))
        {
            if let Some(class_value) = &slot.class_value {
                return Some(class_value.borrow().clone());
            }
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
        {
            if let Some(class_value) = &slot.class_value {
                *class_value.borrow_mut() = value;
                return true;
            }
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

    pub(crate) fn type_of_name(&self) -> &str {
        match self {
            Self::Structure {
                name,
                representation: StructureRepresentation::Structure,
                ..
            } => name,
            Self::Structure {
                representation: StructureRepresentation::List,
                named: true,
                ..
            } => "CONS",
            Self::Structure {
                representation: StructureRepresentation::List,
                ..
            } => "LIST",
            Self::Structure {
                representation: StructureRepresentation::Vector,
                named: true,
                ..
            } => "SIMPLE-VECTOR",
            Self::Structure {
                representation: StructureRepresentation::Vector,
                ..
            } => "VECTOR",
            _ => self.type_name(),
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

    pub(crate) fn structure_typep_is_type(&self, expected: &str) -> bool {
        self.structure_is_type(expected) && self.structure_discriminator_matches()
    }

    fn structure_discriminator_matches(&self) -> bool {
        let Self::Structure {
            name,
            representation: StructureRepresentation::List | StructureRepresentation::Vector,
            named: true,
            marker: Some(marker),
            ..
        } = self
        else {
            return true;
        };
        let marker = marker.borrow();
        marker
            .symbol_name()
            .is_some_and(|marker_name| marker_name.eq_ignore_ascii_case(name))
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
        let Self::Structure {
            name,
            types,
            slots,
            representation,
            named,
            marker,
        } = self
        else {
            return None;
        };
        Some(Self::Structure {
            name: name.clone(),
            types: types.clone(),
            slots: Rc::new(RefCell::new(slots.borrow().clone())),
            representation: *representation,
            named: *named,
            marker: marker
                .as_ref()
                .map(|marker| Rc::new(RefCell::new(marker.borrow().clone()))),
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
            Self::RandomState(_) => "RANDOM-STATE",
            Self::Package(_) => "PACKAGE",
            Self::Environment(_) => "ENVIRONMENT",
            Self::Symbol(_)
            | Self::SymbolExact(_)
            | Self::QualifiedSymbolExact { .. }
            | Self::UninternedSymbol(_) => "SYMBOL",
            Self::Keyword(_) | Self::KeywordExact(_) => "KEYWORD",
            Self::List(_) | Self::DottedList { .. } => "LIST",
            Self::Vector(_) => "VECTOR",
            Self::Array { .. } => "ARRAY",
            Self::HashTable { .. } => "HASH-TABLE",
            Self::HashTableIterator(_) => "HASH-TABLE-ITERATOR",
            Self::Values(_) => "VALUES",
            Self::Condition(_) => "CONDITION",
            Self::Restart(_) => "RESTART",
            Self::Structure { representation, .. } => match representation {
                StructureRepresentation::Structure => "STRUCTURE",
                StructureRepresentation::List => "LIST",
                StructureRepresentation::Vector => "VECTOR",
            },
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
            "SIMPLE-TYPE-ERROR" => matches!(
                expected.as_str(),
                "CONDITION" | "ERROR" | "SERIOUS-CONDITION" | "SIMPLE-CONDITION" | "TYPE-ERROR"
            ),
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
            "CONTROL-ERROR" => matches!(
                expected.as_str(),
                "CONDITION" | "ERROR" | "SERIOUS-CONDITION"
            ),
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
            Self::Structure {
                name,
                slots,
                representation: StructureRepresentation::List,
                named,
                marker,
                ..
            } => {
                let mut values = Vec::with_capacity(slots.borrow().len() + usize::from(*named));
                if *named {
                    values.push(
                        marker
                            .as_ref()
                            .map(|marker| marker.borrow().clone())
                            .unwrap_or_else(|| Self::symbol(name.as_ref())),
                    );
                }
                values.extend(slots.borrow().iter().map(|(_, value)| value.clone()));
                Some(values)
            }
            _ => None,
        }
    }

    pub fn vector_items(&self) -> Option<Vec<Value>> {
        match self {
            Self::Vector(items) => Some(items.borrow().clone()),
            Self::Structure {
                name,
                slots,
                representation: StructureRepresentation::Vector,
                named,
                marker,
                ..
            } => {
                let mut values = Vec::with_capacity(slots.borrow().len() + usize::from(*named));
                if *named {
                    values.push(
                        marker
                            .as_ref()
                            .map(|marker| marker.borrow().clone())
                            .unwrap_or_else(|| Self::symbol(name.as_ref())),
                    );
                }
                values.extend(slots.borrow().iter().map(|(_, value)| value.clone()));
                Some(values)
            }
            Self::Array {
                dimensions,
                elements,
                fill_pointer,
                ..
            } if dimensions.len() == 1 => {
                let elements = elements.borrow();
                let end = fill_pointer
                    .as_ref()
                    .map(|pointer| *pointer.borrow())
                    .unwrap_or(elements.len())
                    .min(elements.len());
                Some(elements[..end].to_vec())
            }
            _ => None,
        }
    }

    pub(crate) fn is_typed_list(&self) -> bool {
        matches!(
            self,
            Self::Structure {
                representation: StructureRepresentation::List,
                ..
            }
        )
    }

    pub(crate) fn is_typed_vector(&self) -> bool {
        matches!(
            self,
            Self::Structure {
                representation: StructureRepresentation::Vector,
                ..
            }
        )
    }

    pub(crate) fn is_simple_vector(&self) -> bool {
        match self {
            Self::Vector(_) => true,
            Self::Structure {
                representation: StructureRepresentation::Vector,
                ..
            } => true,
            Self::Array {
                dimensions,
                element_type,
                fill_pointer,
                adjustable,
                ..
            } => {
                dimensions.len() == 1
                    && *element_type == ArrayElementType::T
                    && fill_pointer.is_none()
                    && !adjustable
            }
            _ => false,
        }
    }

    pub(crate) fn set_sequence_item(&self, index: usize, value: Value) -> bool {
        match self {
            Self::Vector(items) => {
                let mut items = items.borrow_mut();
                let Some(item) = items.get_mut(index) else {
                    return false;
                };
                *item = value;
                true
            }
            Self::Structure {
                slots,
                representation:
                    StructureRepresentation::List | StructureRepresentation::Vector,
                named,
                marker,
                ..
            } => {
                if *named && index == 0 {
                    let Some(marker) = marker.as_ref() else {
                        return false;
                    };
                    *marker.borrow_mut() = value;
                    return true;
                }
                let Some(slot_index) = index.checked_sub(usize::from(*named)) else {
                    return false;
                };
                let mut slots = slots.borrow_mut();
                let Some((_, slot_value)) = slots.get_mut(slot_index) else {
                    return false;
                };
                *slot_value = value;
                true
            }
            Self::Array {
                dimensions,
                elements,
                fill_pointer,
                element_type: ArrayElementType::T,
                ..
            } if dimensions.len() == 1 => {
                let limit = fill_pointer
                    .as_ref()
                    .map(|pointer| *pointer.borrow())
                    .unwrap_or_else(|| elements.borrow().len());
                if index >= limit {
                    return false;
                }
                let mut elements = elements.borrow_mut();
                let Some(item) = elements.get_mut(index) else {
                    return false;
                };
                *item = value;
                true
            }
            _ => false,
        }
    }

    pub(crate) fn set_typed_list_cdr(&self, replacement: &[Value]) -> bool {
        let Self::Structure {
            slots,
            representation: StructureRepresentation::List,
            named,
            ..
        } = self
        else {
            return false;
        };

        let mut slots = slots.borrow_mut();
        if *named {
            let old_len = slots.len();
            slots.truncate(replacement.len());
            for (index, value) in replacement.iter().enumerate().take(old_len) {
                let Some((_, slot_value)) = slots.get_mut(index) else {
                    break;
                };
                *slot_value = value.clone();
            }
            for value in replacement.iter().skip(old_len) {
                slots.push((Rc::<str>::from(""), value.clone()));
            }
        } else {
            let Some((first_name, first_value)) = slots.first().cloned() else {
                return false;
            };
            let mut updated = Vec::with_capacity(replacement.len() + 1);
            updated.push((first_name, first_value));
            for (index, value) in replacement.iter().enumerate() {
                let slot_name = slots
                    .get(index + 1)
                    .map(|(name, _)| name.clone())
                    .unwrap_or_else(|| Rc::<str>::from(""));
                updated.push((slot_name, value.clone()));
            }
            *slots = updated;
        }
        true
    }

    pub fn array_dimensions(&self) -> Option<Vec<usize>> {
        match self {
            Self::Array {
                dimensions,
                elements,
                fill_pointer,
                ..
            } => {
                if dimensions.len() == 1 && fill_pointer.is_some() {
                    Some(vec![elements.borrow().len()])
                } else {
                    Some(dimensions.as_ref().clone())
                }
            }
            _ => None,
        }
    }

    pub(crate) fn array_fill_pointer(&self) -> Option<usize> {
        match self {
            Self::Array {
                fill_pointer: Some(pointer),
                ..
            } => Some(*pointer.borrow()),
            _ => None,
        }
    }

    pub(crate) fn has_fill_pointer(&self) -> bool {
        self.array_fill_pointer().is_some()
    }

    pub(crate) fn is_adjustable_array(&self) -> bool {
        matches!(self, Self::Array { adjustable: true, .. })
    }

    pub(crate) fn set_array_fill_pointer(&self, value: usize) -> bool {
        let Self::Array {
            dimensions,
            elements,
            fill_pointer: Some(pointer),
            ..
        } = self
        else {
            return false;
        };
        if dimensions.len() != 1 || value > elements.borrow().len() {
            return false;
        }
        *pointer.borrow_mut() = value;
        true
    }

    pub fn array_items(&self) -> Option<Vec<Value>> {
        match self {
            Self::Array { elements, .. } => Some(elements.borrow().clone()),
            _ => None,
        }
    }

    pub(crate) fn array_element_type(&self) -> Option<ArrayElementType> {
        match self {
            Self::String(_) => Some(ArrayElementType::Character),
            Self::Vector(_) => Some(ArrayElementType::T),
            Self::Structure {
                representation: StructureRepresentation::Vector,
                ..
            } => Some(ArrayElementType::T),
            Self::Array { element_type, .. } => Some(*element_type),
            _ => None,
        }
    }

    pub(crate) fn accepts_array_element(&self, value: &Self) -> bool {
        match self.array_element_type() {
            Some(ArrayElementType::T) => true,
            Some(ArrayElementType::Character) => matches!(value, Self::Character(_)),
            Some(ArrayElementType::Bit) => {
                matches!(value, Self::Integer(bit) if *bit == 0 || *bit == 1)
            }
            None => false,
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

    pub(crate) fn hash_table_size(&self) -> Option<usize> {
        match self {
            Self::HashTable { size, .. } => Some(*size),
            _ => None,
        }
    }

    pub(crate) fn hash_table_rehash_size(&self) -> Option<f64> {
        match self {
            Self::HashTable { rehash_size, .. } => Some(*rehash_size),
            _ => None,
        }
    }

    pub(crate) fn hash_table_rehash_threshold(&self) -> Option<f64> {
        match self {
            Self::HashTable {
                rehash_threshold,
                ..
            } => Some(*rehash_threshold),
            _ => None,
        }
    }

    pub(crate) fn hash_table_synchronized(&self) -> Option<bool> {
        match self {
            Self::HashTable { synchronized, .. } => Some(*synchronized),
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
            Self::QualifiedSymbolExact {
                reference,
                package_len,
            } => Some(&reference[*package_len + 2..]),
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
            Self::QualifiedSymbolExact { reference, .. } => Some((reference, true)),
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
                    imaginary: left_imaginary,
                },
                Self::Complex {
                    real: right_real,
                    imaginary: right_imaginary,
                },
            ) => left_real.eq_value(right_real) && left_imaginary.eq_value(right_imaginary),
            (Self::Character(left), Self::Character(right)) => left == right,
            (Self::Stream(left), Self::Stream(right)) => Rc::ptr_eq(left, right),
            (Self::RandomState(left), Self::RandomState(right)) => Rc::ptr_eq(left, right),
            (Self::Package(left), Self::Package(right)) => left == right,
            (Self::String(left), Self::String(right)) => Rc::ptr_eq(left, right),
            (Self::Symbol(left), Self::Symbol(right))
            | (Self::Keyword(left), Self::Keyword(right)) => left == right,
            (Self::SymbolExact(left), Self::SymbolExact(right))
            | (Self::KeywordExact(left), Self::KeywordExact(right)) => left == right,
            (
                Self::Symbol(left),
                Self::QualifiedSymbolExact {
                    reference: right, ..
                },
            )
            | (
                Self::QualifiedSymbolExact {
                    reference: right, ..
                },
                Self::Symbol(left),
            ) => left == right,
            (
                Self::QualifiedSymbolExact {
                    reference: left_reference,
                    package_len: left_package_len,
                },
                Self::QualifiedSymbolExact {
                    reference: right_reference,
                    package_len: right_package_len,
                },
            ) => left_package_len == right_package_len && left_reference == right_reference,
            (Self::UninternedSymbol(left), Self::UninternedSymbol(right)) => {
                Rc::ptr_eq(left, right)
            }
            (Self::List(left), Self::List(right)) => Rc::ptr_eq(left, right),
            (Self::Vector(left), Self::Vector(right)) => Rc::ptr_eq(left, right),
            (
                Self::Array {
                    dimensions: left_dimensions,
                    elements: left_elements,
                    ..
                },
                Self::Array {
                    dimensions: right_dimensions,
                    elements: right_elements,
                    ..
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
            (Self::HashTableIterator(left), Self::HashTableIterator(right)) => Rc::ptr_eq(left, right),
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
            (Self::List(left), Self::List(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right.iter())
                        .all(|(left, right)| left.equal_value(right))
            }
            (Self::Vector(left), Self::Vector(right)) => {
                let left = left.borrow();
                let right = right.borrow();
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right.iter())
                        .all(|(left, right)| left.equal_value(right))
            }
            (
                Self::Complex {
                    real: left_real,
                    imaginary: left_imaginary,
                },
                Self::Complex {
                    real: right_real,
                    imaginary: right_imaginary,
                },
            ) => left_real.equal_value(right_real) && left_imaginary.equal_value(right_imaginary),
            (
                Self::Array {
                    dimensions: left_dimensions,
                    elements: left_elements,
                    ..
                },
                Self::Array {
                    dimensions: right_dimensions,
                    elements: right_elements,
                    ..
                },
            ) => {
                left_dimensions == right_dimensions
                    && left_elements.borrow().len() == right_elements.borrow().len()
                    && left_elements
                        .borrow()
                        .iter()
                        .zip(right_elements.borrow().iter())
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
