//! Condition class representation and the standard hierarchy.
//!
//! A condition class is a small descriptor object: a simple vector of
//! `[name, superclass, slots]`, where `superclass` is the descriptor of the
//! direct superclass (NIL at the root) and `slots` is a deferred slot
//! specification reserved for `define-condition`. A condition instance is a
//! CLOS-style `make_instance` whose class word is the descriptor. This is the
//! minimal shape that L18 (`ncl-clos`) can later formalize into a real class.
use crate::error::ConditionError;
use ncl_object::{
    Instance, ObjectError, Runtime, ThreadContext, Word, car, cdr, instance_class, make_cons,
    make_instance, make_simple_vector, make_string, simple_vector_ref, simple_vector_set,
    string_length, string_ref,
};
/// Index of the class name string in a class descriptor.
pub const NAME_SLOT: usize = 0;
/// Index of the direct superclass descriptor in a class descriptor.
///
/// Index 2 is the deferred slot specification, reserved for `define-condition`
/// and not yet read.
pub const SUPERCLASS_SLOT: usize = 1;
/// An opaque handle to a registered condition class.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConditionClass(Word);
impl ConditionClass {
    /// Return the underlying class descriptor word.
    #[must_use]
    pub const fn as_word(self) -> Word {
        self.0
    }
    /// Wrap a class descriptor word.
    #[must_use]
    pub const fn from_word(word: Word) -> Self {
        Self(word)
    }
}
/// Identifier for a standard condition class.
///
/// The identifier is independent of a runtime class descriptor. Use
/// [`ConditionIdentifier::name`] when resolving it through [`condition_class`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
#[allow(
    missing_docs,
    reason = "variants mirror the registered CL condition names"
)]
pub enum ConditionIdentifier {
    Condition,
    Warning,
    SeriousCondition,
    Error,
    StorageCondition,
    TypeError,
    SimpleTypeError,
    ArithmeticError,
    DivisionByZero,
    FloatingPointOverflow,
    FloatingPointUnderflow,
    FloatingPointInvalidOperation,
    FloatingPointInexact,
    CellError,
    UnboundVariable,
    UndefinedFunction,
    UnboundSlot,
    FileError,
    PackageError,
    ControlError,
    ProgramError,
    ParseError,
    ReaderError,
    PrintNotReadable,
    StreamError,
    EndOfFile,
    SimpleCondition,
    SimpleError,
    SimpleWarning,
    StyleWarning,
    UndefinedAlienError,
    CodeDeletionNote,
    CompilerNote,
    DefconstantUneql,
    DeleteFileError,
    DeprecationCondition,
    DeprecationError,
    EarlyDeprecationWarning,
    FileDoesNotExist,
    FileExists,
    FinalDeprecationWarning,
    ImplicitGenericFunctionWarning,
    InvalidFasl,
    LateDeprecationWarning,
    NameConflict,
    PackageDoesNotExist,
    PackageLockViolation,
    PackageLockedError,
    ReaderPackageDoesNotExist,
    StepCondition,
    StepFinishedCondition,
    StepFormCondition,
    StepValuesCondition,
    SymbolPackageLockedError,
    Timeout,
    UnknownKeywordArgument,
    SystemCondition,
    BreakpointError,
    DeadlineTimeout,
    InteractiveInterrupt,
    IoTimeout,
    MemoryFaultError,
    ThreadError,
    InterruptThreadError,
    JoinThreadError,
    SymbolValueInThreadError,
    ThreadDeadlock,
}
impl ConditionIdentifier {
    /// Return the registered Common Lisp class name for this identifier.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Condition => "CONDITION",
            Self::Warning => "WARNING",
            Self::SeriousCondition => "SERIOUS-CONDITION",
            Self::Error => "ERROR",
            Self::StorageCondition => "STORAGE-CONDITION",
            Self::TypeError => "TYPE-ERROR",
            Self::SimpleTypeError => "SIMPLE-TYPE-ERROR",
            Self::ArithmeticError => "ARITHMETIC-ERROR",
            Self::DivisionByZero => "DIVISION-BY-ZERO",
            Self::FloatingPointOverflow => "FLOATING-POINT-OVERFLOW",
            Self::FloatingPointUnderflow => "FLOATING-POINT-UNDERFLOW",
            Self::FloatingPointInvalidOperation => "FLOATING-POINT-INVALID-OPERATION",
            Self::FloatingPointInexact => "FLOATING-POINT-INEXACT",
            Self::CellError => "CELL-ERROR",
            Self::UnboundVariable => "UNBOUND-VARIABLE",
            Self::UndefinedFunction => "UNDEFINED-FUNCTION",
            Self::UnboundSlot => "UNBOUND-SLOT",
            Self::FileError => "FILE-ERROR",
            Self::PackageError => "PACKAGE-ERROR",
            Self::ControlError => "CONTROL-ERROR",
            Self::ProgramError => "PROGRAM-ERROR",
            Self::ParseError => "PARSE-ERROR",
            Self::ReaderError => "READER-ERROR",
            Self::PrintNotReadable => "PRINT-NOT-READABLE",
            Self::StreamError => "STREAM-ERROR",
            Self::EndOfFile => "END-OF-FILE",
            Self::SimpleCondition => "SIMPLE-CONDITION",
            Self::SimpleError => "SIMPLE-ERROR",
            Self::SimpleWarning => "SIMPLE-WARNING",
            Self::StyleWarning => "STYLE-WARNING",
            Self::UndefinedAlienError => "UNDEFINED-ALIEN-ERROR",
            Self::CodeDeletionNote => "CODE-DELETION-NOTE",
            Self::CompilerNote => "COMPILER-NOTE",
            Self::DefconstantUneql => "DEFCONSTANT-UNEQL",
            Self::DeleteFileError => "DELETE-FILE-ERROR",
            Self::DeprecationCondition => "DEPRECATION-CONDITION",
            Self::DeprecationError => "DEPRECATION-ERROR",
            Self::EarlyDeprecationWarning => "EARLY-DEPRECATION-WARNING",
            Self::FileDoesNotExist => "FILE-DOES-NOT-EXIST",
            Self::FileExists => "FILE-EXISTS",
            Self::FinalDeprecationWarning => "FINAL-DEPRECATION-WARNING",
            Self::ImplicitGenericFunctionWarning => "IMPLICIT-GENERIC-FUNCTION-WARNING",
            Self::InvalidFasl => "INVALID-FASL",
            Self::LateDeprecationWarning => "LATE-DEPRECATION-WARNING",
            Self::NameConflict => "NAME-CONFLICT",
            Self::PackageDoesNotExist => "PACKAGE-DOES-NOT-EXIST",
            Self::PackageLockViolation => "PACKAGE-LOCK-VIOLATION",
            Self::PackageLockedError => "PACKAGE-LOCKED-ERROR",
            Self::ReaderPackageDoesNotExist => "READER-PACKAGE-DOES-NOT-EXIST",
            Self::StepCondition => "STEP-CONDITION",
            Self::StepFinishedCondition => "STEP-FINISHED-CONDITION",
            Self::StepFormCondition => "STEP-FORM-CONDITION",
            Self::StepValuesCondition => "STEP-VALUES-CONDITION",
            Self::SymbolPackageLockedError => "SYMBOL-PACKAGE-LOCKED-ERROR",
            Self::Timeout => "TIMEOUT",
            Self::UnknownKeywordArgument => "UNKNOWN-KEYWORD-ARGUMENT",
            Self::SystemCondition => "SYSTEM-CONDITION",
            Self::BreakpointError => "BREAKPOINT-ERROR",
            Self::DeadlineTimeout => "DEADLINE-TIMEOUT",
            Self::InteractiveInterrupt => "INTERACTIVE-INTERRUPT",
            Self::IoTimeout => "IO-TIMEOUT",
            Self::MemoryFaultError => "MEMORY-FAULT-ERROR",
            Self::ThreadError => "THREAD-ERROR",
            Self::InterruptThreadError => "INTERRUPT-THREAD-ERROR",
            Self::JoinThreadError => "JOIN-THREAD-ERROR",
            Self::SymbolValueInThreadError => "SYMBOL-VALUE-IN-THREAD-ERROR",
            Self::ThreadDeadlock => "THREAD-DEADLOCK",
        }
    }
    /// Resolve this identifier to a registered runtime class.
    #[must_use]
    pub fn class(self, ctx: &mut ThreadContext, runtime: &Runtime) -> Option<ConditionClass> {
        condition_class(ctx, runtime, self.name())
    }
}
/// A typed condition slot value at the object boundary.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConditionSlotValue(Word);
impl ConditionSlotValue {
    /// Wrap an object value as a condition slot value.
    #[must_use]
    pub const fn from_word(value: Word) -> Self {
        Self(value)
    }
    /// Return the object value stored in this slot.
    #[must_use]
    pub const fn as_word(self) -> Word {
        self.0
    }
}
/// A typed handle to an allocated condition record.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConditionRecord(Word);
impl ConditionRecord {
    /// Wrap an allocated condition record.
    #[must_use]
    pub const fn from_word(record: Word) -> Self {
        Self(record)
    }
    /// Return the underlying condition instance word.
    #[must_use]
    pub const fn as_word(self) -> Word {
        self.0
    }
}
/// Look up a registered condition class by name.
#[must_use]
pub fn condition_class(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Option<ConditionClass> {
    runtime.class(ctx, name).map(ConditionClass::from_word)
}
/// Return the name string of a condition class.
///
/// # Errors
/// Returns an object-layer error when the descriptor is not a vector.
pub fn condition_class_name(
    ctx: &ThreadContext,
    class: ConditionClass,
) -> Result<Word, ConditionError> {
    simple_vector_ref(ctx, class.as_word(), NAME_SLOT).map_err(ConditionError::from)
}
/// Construct a condition instance of `class` with the given slot values.
///
/// # Errors
/// Returns an object-layer error when allocation fails.
pub fn make_condition(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    class: ConditionClass,
    slots: &[Word],
) -> Result<Word, ConditionError> {
    let typed_slots: Vec<ConditionSlotValue> = slots
        .iter()
        .copied()
        .map(ConditionSlotValue::from_word)
        .collect();
    make_condition_record(ctx, runtime, class, &typed_slots).map(ConditionRecord::as_word)
}
/// Construct a typed condition record from a runtime class and typed slots.
///
/// This is the allocation boundary for condition instances. The legacy
/// [`make_condition`] entry point delegates here so callers can migrate
/// without changing the heap representation.
///
/// # Errors
/// Returns an object-layer error when allocation or instance construction fails.
pub fn make_condition_record(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    class: ConditionClass,
    slots: &[ConditionSlotValue],
) -> Result<ConditionRecord, ConditionError> {
    let values: Vec<Word> = slots
        .iter()
        .copied()
        .map(ConditionSlotValue::as_word)
        .collect();
    make_instance(ctx, runtime, class.as_word(), &values)
        .map(Instance::as_word)
        .map(ConditionRecord::from_word)
        .map_err(ConditionError::from)
}
/// Construct a typed condition record from a standard condition identifier.
///
/// An unregistered identifier is reported as an object layout error. This
/// keeps registration failures in the existing condition error boundary and
/// does not introduce a second error conversion path.
///
/// # Errors
/// Returns an object-layer error when the identifier is not registered or
/// allocation fails.
pub fn make_typed_condition(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    identifier: ConditionIdentifier,
    slots: &[ConditionSlotValue],
) -> Result<ConditionRecord, ConditionError> {
    let class = identifier
        .class(ctx, runtime)
        .ok_or(ConditionError::Object(ObjectError::Layout))?;
    make_condition_record(ctx, runtime, class, slots)
}
/// Return the class of a condition instance.
///
/// # Errors
/// Returns [`ConditionError::NotACondition`] when the word is not an
/// instance, and [`ConditionError::Object`] for other object-layer failures.
pub fn condition_class_of(
    ctx: &ThreadContext,
    condition: Word,
) -> Result<ConditionClass, ConditionError> {
    match instance_class(ctx, Instance::from_word(condition)) {
        Ok(class) => Ok(ConditionClass::from_word(class)),
        Err(ObjectError::TypeError) => Err(ConditionError::NotACondition),
        Err(error) => Err(ConditionError::Object(error)),
    }
}
pub use crate::hierarchy::HIERARCHY;
/// Register a class descriptor under `name`.
///
/// # Errors
/// Returns an object-layer error when allocation or registration fails.
pub fn install_class(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<(), ObjectError> {
    let descriptor = make_class_descriptor(ctx, runtime, name)?;
    runtime.define_class(ctx, name, descriptor)
}
/// Link `name` to its direct `superclasses`, most-specific-first.
///
/// A single superclass is stored as the bare class descriptor word (the
/// original Phase 1 representation, still relied on outside this crate, for
/// example by `ncl-clos`'s `TYPEP`). Two or more superclasses are stored as a
/// proper list of descriptors, modelling genuine multiple inheritance.
///
/// # Errors
/// Returns an object-layer error when any named class is not registered or
/// the list cannot be allocated.
pub fn wire_superclasses(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    superclasses: &[&str],
) -> Result<(), ObjectError> {
    let child = runtime.class(ctx, name).ok_or(ObjectError::Layout)?;
    let parents = superclasses
        .iter()
        .map(|superclass| runtime.class(ctx, superclass).ok_or(ObjectError::Layout))
        .collect::<Result<Vec<_>, _>>()?;
    let value = match parents.as_slice() {
        [] => Word::NIL,
        [single] => *single,
        // check-added-lines: allow(wildcard) slice-length match is exhaustive over 0/1/many
        _ => {
            let mut list = Word::NIL;
            for parent in parents.iter().rev() {
                list = make_cons(ctx, runtime, *parent, list)?;
            }
            list
        }
    };
    simple_vector_set(ctx, child, SUPERCLASS_SLOT, value)
}
/// Read the raw direct-superclass slot of a class: `NIL` at the root, a
/// single class descriptor word, or a list of descriptors for a class with
/// more than one direct superclass.
///
/// # Errors
/// Returns an object-layer error when `class` is not a descriptor vector.
pub fn superclass_of(ctx: &ThreadContext, class: Word) -> Result<Word, ObjectError> {
    simple_vector_ref(ctx, class, SUPERCLASS_SLOT)
}
/// Decode a raw direct-superclass slot value into its direct parent
/// descriptors (zero, one, or many).
///
/// # Errors
/// Returns an object-layer error when `value` is a malformed list.
pub fn direct_parents(ctx: &ThreadContext, value: Word) -> Result<Vec<Word>, ObjectError> {
    if value == Word::NIL {
        return Ok(Vec::new());
    }
    if !value.is_cons() {
        return Ok(vec![value]);
    }
    let mut parents = Vec::new();
    let mut current = value;
    while current != Word::NIL {
        parents.push(car(ctx, current)?);
        current = cdr(ctx, current)?;
    }
    Ok(parents)
}
/// Whether `class` is `name` or has an ancestor named `name`, walking every
/// direct superclass when a class has more than one (the full precedence
/// list, not just the first parent).
///
/// # Errors
/// Returns an object-layer error when a descriptor is not a vector.
pub fn class_named(ctx: &ThreadContext, class: Word, name: &str) -> Result<bool, ObjectError> {
    if string_equals(ctx, simple_vector_ref(ctx, class, NAME_SLOT)?, name)? {
        return Ok(true);
    }
    for parent in direct_parents(ctx, superclass_of(ctx, class)?)? {
        if class_named(ctx, parent, name)? {
            return Ok(true);
        }
    }
    Ok(false)
}
/// Whether a heap string equals a Rust string literal.
pub fn string_equals(ctx: &ThreadContext, word: Word, name: &str) -> Result<bool, ObjectError> {
    let chars: Vec<char> = name.chars().collect();
    if string_length(ctx, word)? != chars.len() {
        return Ok(false);
    }
    for (index, expected) in chars.iter().enumerate() {
        if string_ref(ctx, word, index)? != *expected {
            return Ok(false);
        }
    }
    Ok(true)
}
/// Whether two heap strings have equal contents.
pub fn string_words_equal(ctx: &ThreadContext, a: Word, b: Word) -> Result<bool, ObjectError> {
    let length_a = string_length(ctx, a)?;
    if length_a != string_length(ctx, b)? {
        return Ok(false);
    }
    for index in 0..length_a {
        if string_ref(ctx, a, index)? != string_ref(ctx, b, index)? {
            return Ok(false);
        }
    }
    Ok(true)
}
fn make_class_descriptor(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<Word, ObjectError> {
    let name_word = make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
    make_simple_vector(ctx, runtime, &[name_word, Word::NIL, Word::NIL])
}
