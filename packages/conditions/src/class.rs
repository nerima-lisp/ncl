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
    Instance, ObjectError, Runtime, ThreadContext, Word, instance_class, make_instance,
    make_simple_vector, make_string, simple_vector_ref, simple_vector_set, string_length,
    string_ref,
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
    match instance_class(ctx, Instance::from(condition)) {
        Ok(class) => Ok(ConditionClass::from_word(class)),
        Err(ObjectError::TypeError) => Err(ConditionError::NotACondition),
        Err(error) => Err(ConditionError::Object(error)),
    }
}
/// One standard condition class and its direct superclass.
pub struct HierarchyRow {
    /// Class name.
    pub name: &'static str,
    /// Direct superclass name, `None` for the root `condition`.
    pub superclass: Option<&'static str>,
}
/// The standard condition hierarchy, using single-superclass links.
/// Multiple inheritance is a Wave-2 (`ncl-clos`) concern.
#[rustfmt::skip]
pub const HIERARCHY: &[HierarchyRow] = &[
    HierarchyRow { name: "CONDITION", superclass: None },
    HierarchyRow { name: "WARNING", superclass: Some("CONDITION") },
    HierarchyRow { name: "SERIOUS-CONDITION", superclass: Some("CONDITION") },
    HierarchyRow { name: "ERROR", superclass: Some("SERIOUS-CONDITION") },
    HierarchyRow { name: "STORAGE-CONDITION", superclass: Some("SERIOUS-CONDITION") },
    HierarchyRow { name: "TYPE-ERROR", superclass: Some("ERROR") },
    HierarchyRow { name: "SIMPLE-TYPE-ERROR", superclass: Some("TYPE-ERROR") },
    HierarchyRow { name: "ARITHMETIC-ERROR", superclass: Some("ERROR") },
    HierarchyRow { name: "DIVISION-BY-ZERO", superclass: Some("ARITHMETIC-ERROR") },
    HierarchyRow { name: "FLOATING-POINT-OVERFLOW", superclass: Some("ARITHMETIC-ERROR") },
    HierarchyRow { name: "FLOATING-POINT-UNDERFLOW", superclass: Some("ARITHMETIC-ERROR") },
    HierarchyRow { name: "FLOATING-POINT-INVALID-OPERATION", superclass: Some("ARITHMETIC-ERROR") },
    HierarchyRow { name: "FLOATING-POINT-INEXACT", superclass: Some("ARITHMETIC-ERROR") },
    HierarchyRow { name: "CELL-ERROR", superclass: Some("ERROR") },
    HierarchyRow { name: "UNBOUND-VARIABLE", superclass: Some("CELL-ERROR") },
    HierarchyRow { name: "UNDEFINED-FUNCTION", superclass: Some("CELL-ERROR") },
    HierarchyRow { name: "UNBOUND-SLOT", superclass: Some("CELL-ERROR") },
    HierarchyRow { name: "FILE-ERROR", superclass: Some("ERROR") },
    HierarchyRow { name: "PACKAGE-ERROR", superclass: Some("ERROR") },
    HierarchyRow { name: "CONTROL-ERROR", superclass: Some("ERROR") },
    HierarchyRow { name: "PROGRAM-ERROR", superclass: Some("ERROR") },
    HierarchyRow { name: "PARSE-ERROR", superclass: Some("ERROR") },
    HierarchyRow { name: "READER-ERROR", superclass: Some("PARSE-ERROR") },
    HierarchyRow { name: "PRINT-NOT-READABLE", superclass: Some("ERROR") },
    HierarchyRow { name: "STREAM-ERROR", superclass: Some("ERROR") },
    HierarchyRow { name: "END-OF-FILE", superclass: Some("STREAM-ERROR") },
    HierarchyRow { name: "SIMPLE-CONDITION", superclass: Some("CONDITION") },
    HierarchyRow { name: "SIMPLE-ERROR", superclass: Some("SIMPLE-CONDITION") },
    HierarchyRow { name: "SIMPLE-WARNING", superclass: Some("SIMPLE-CONDITION") },
    HierarchyRow { name: "STYLE-WARNING", superclass: Some("WARNING") },
    HierarchyRow { name: "UNDEFINED-ALIEN-ERROR", superclass: Some("ERROR") },
    HierarchyRow { name: "CODE-DELETION-NOTE", superclass: Some("CONDITION") },
    HierarchyRow { name: "COMPILER-NOTE", superclass: Some("CONDITION") },
    HierarchyRow { name: "DEFCONSTANT-UNEQL", superclass: Some("ERROR") },
    HierarchyRow { name: "DELETE-FILE-ERROR", superclass: Some("FILE-ERROR") },
    HierarchyRow { name: "DEPRECATION-CONDITION", superclass: Some("CONDITION") },
    HierarchyRow { name: "DEPRECATION-ERROR", superclass: Some("DEPRECATION-CONDITION") },
    HierarchyRow { name: "EARLY-DEPRECATION-WARNING", superclass: Some("STYLE-WARNING") },
    HierarchyRow { name: "FILE-DOES-NOT-EXIST", superclass: Some("FILE-ERROR") },
    HierarchyRow { name: "FILE-EXISTS", superclass: Some("FILE-ERROR") },
    HierarchyRow { name: "FINAL-DEPRECATION-WARNING", superclass: Some("STYLE-WARNING") },
    HierarchyRow { name: "IMPLICIT-GENERIC-FUNCTION-WARNING", superclass: Some("STYLE-WARNING") },
    HierarchyRow { name: "INVALID-FASL", superclass: Some("ERROR") },
    HierarchyRow { name: "LATE-DEPRECATION-WARNING", superclass: Some("STYLE-WARNING") },
    HierarchyRow { name: "NAME-CONFLICT", superclass: Some("ERROR") },
    HierarchyRow { name: "PACKAGE-DOES-NOT-EXIST", superclass: Some("ERROR") },
    HierarchyRow { name: "PACKAGE-LOCK-VIOLATION", superclass: Some("ERROR") },
    HierarchyRow { name: "PACKAGE-LOCKED-ERROR", superclass: Some("PACKAGE-ERROR") },
    HierarchyRow { name: "READER-PACKAGE-DOES-NOT-EXIST", superclass: Some("ERROR") },
    HierarchyRow { name: "STEP-CONDITION", superclass: Some("CONDITION") },
    HierarchyRow { name: "STEP-FINISHED-CONDITION", superclass: Some("STEP-CONDITION") },
    HierarchyRow { name: "STEP-FORM-CONDITION", superclass: Some("STEP-CONDITION") },
    HierarchyRow { name: "STEP-VALUES-CONDITION", superclass: Some("STEP-CONDITION") },
    HierarchyRow { name: "SYMBOL-PACKAGE-LOCKED-ERROR", superclass: Some("PACKAGE-LOCKED-ERROR") },
    HierarchyRow { name: "TIMEOUT", superclass: Some("ERROR") },
    HierarchyRow { name: "UNKNOWN-KEYWORD-ARGUMENT", superclass: Some("ERROR") },
    HierarchyRow { name: "SYSTEM-CONDITION", superclass: Some("CONDITION") },
    HierarchyRow { name: "BREAKPOINT-ERROR", superclass: Some("ERROR") },
    HierarchyRow { name: "DEADLINE-TIMEOUT", superclass: Some("TIMEOUT") },
    HierarchyRow { name: "INTERACTIVE-INTERRUPT", superclass: Some("SYSTEM-CONDITION") },
    HierarchyRow { name: "IO-TIMEOUT", superclass: Some("TIMEOUT") },
    HierarchyRow { name: "MEMORY-FAULT-ERROR", superclass: Some("STORAGE-CONDITION") },
    HierarchyRow { name: "THREAD-ERROR", superclass: Some("ERROR") },
    HierarchyRow { name: "INTERRUPT-THREAD-ERROR", superclass: Some("THREAD-ERROR") },
    HierarchyRow { name: "JOIN-THREAD-ERROR", superclass: Some("THREAD-ERROR") },
    HierarchyRow { name: "SYMBOL-VALUE-IN-THREAD-ERROR", superclass: Some("THREAD-ERROR") },
    HierarchyRow { name: "THREAD-DEADLOCK", superclass: Some("THREAD-ERROR") },
];
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
/// Link `name` to its `superclass` descriptor.
///
/// # Errors
/// Returns an object-layer error when either class is not registered.
pub fn wire_superclass(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    superclass: &str,
) -> Result<(), ObjectError> {
    let child = runtime.class(ctx, name).ok_or(ObjectError::Layout)?;
    let parent = runtime.class(ctx, superclass).ok_or(ObjectError::Layout)?;
    simple_vector_set(ctx, child, SUPERCLASS_SLOT, parent)
}
/// Read the direct superclass descriptor of a class, or NIL at the root.
///
/// # Errors
/// Returns an object-layer error when `class` is not a descriptor vector.
pub fn superclass_of(ctx: &ThreadContext, class: Word) -> Result<Word, ObjectError> {
    simple_vector_ref(ctx, class, SUPERCLASS_SLOT)
}
/// Whether `class` is `name` or has an ancestor named `name`.
///
/// # Errors
/// Returns an object-layer error when a descriptor is not a vector.
pub fn class_named(ctx: &ThreadContext, class: Word, name: &str) -> Result<bool, ObjectError> {
    let mut current = class;
    loop {
        if string_equals(ctx, simple_vector_ref(ctx, current, NAME_SLOT)?, name)? {
            return Ok(true);
        }
        let superclass = superclass_of(ctx, current)?;
        if superclass == Word::NIL {
            return Ok(false);
        }
        current = superclass;
    }
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
