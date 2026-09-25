//! Alien type descriptors and the operations over them.
//!
//! An [`AlienType`] describes a foreign (C) type: its size and alignment, the
//! byte layout of a structure or union, and how a Lisp value marshals into and
//! out of the foreign representation.

mod marshal;
mod parse;
mod size;

pub use marshal::{marshal_argument, unmarshal_result};
pub use parse::{parse_type_name, parse_type_specifier};
pub use size::{align_of, field_offset, offset_of, record_size, size_of, union_size};

/// A foreign (C) type descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AlienType {
    /// `void`.
    Void,
    /// A C boolean, one byte wide.
    Boolean,
    /// A C `char`.
    Char,
    /// A C `unsigned char`.
    UnsignedChar,
    /// A C `short`.
    Short,
    /// A C `unsigned short`.
    UnsignedShort,
    /// A C `int`.
    Int,
    /// A C `unsigned int`.
    UnsignedInt,
    /// A C `long`.
    Long,
    /// A C `unsigned long`.
    UnsignedLong,
    /// A C `long long`.
    LongLong,
    /// A C `unsigned long long`.
    UnsignedLongLong,
    /// C `size_t`.
    SizeT,
    /// C `ssize_t`.
    SSizeT,
    /// C `float`.
    SingleFloat,
    /// C `double`.
    DoubleFloat,
    /// C `long double`.
    LongFloat,
    /// A pointer to another type.
    Pointer(Box<Self>),
    /// A NUL-terminated C string.
    CString,
    /// A NUL-terminated UTF-8 string.
    Utf8String,
    /// A raw system area pointer.
    SystemAreaPointer,
    /// A fixed-length array of an element type.
    Array(Box<Self>, ArrayLength),
    /// A structure with named fields.
    Structure(AlienRecord),
    /// A union with named fields.
    Union(AlienRecord),
    /// An enumeration with explicit variant values.
    Enumeration(AlienEnum),
    /// A function type.
    Function(Box<AlienRoutine>),
}

/// A named record (`struct` or `union`) with ordered fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlienRecord {
    /// Record name.
    name: String,
    /// Field name and type, in declaration order.
    fields: Vec<(String, AlienType)>,
}

impl AlienRecord {
    /// Build a named record from its fields in declaration order.
    #[must_use]
    pub fn new(name: impl Into<String>, fields: Vec<(String, AlienType)>) -> Self {
        Self {
            name: name.into(),
            fields,
        }
    }

    /// Return the record name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the fields in declaration order.
    #[must_use]
    pub fn fields(&self) -> &[(String, AlienType)] {
        &self.fields
    }

    /// Return a field by declaration index.
    #[must_use]
    pub fn field(&self, index: usize) -> Option<(&str, &AlienType)> {
        self.fields.get(index).map(|(name, ty)| (name.as_str(), ty))
    }

    /// Return the typed byte offset of a field by declaration index.
    #[must_use]
    pub fn field_offset(&self, index: usize) -> Option<FieldOffset> {
        crate::alien::field_offset(self, index)
    }
}

/// A named enumeration with explicit variant values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlienEnum {
    /// Enumeration name.
    name: String,
    /// Variant name and value.
    variants: Vec<(String, i64)>,
}

impl AlienEnum {
    /// Build a named enumeration from its explicit variants.
    #[must_use]
    pub fn new(name: impl Into<String>, variants: Vec<(String, i64)>) -> Self {
        Self {
            name: name.into(),
            variants,
        }
    }

    /// Return the enumeration name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the variants and their explicit values.
    #[must_use]
    pub fn variants(&self) -> &[(String, i64)] {
        &self.variants
    }

    /// Return the value assigned to a named variant.
    #[must_use]
    pub fn variant_value(&self, name: &str) -> Option<i64> {
        self.variants
            .iter()
            .find_map(|(variant, value)| (variant == name).then_some(*value))
    }
}

/// A fixed number of elements in an alien array.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct ArrayLength(usize);

impl ArrayLength {
    /// Construct an array length.
    #[must_use]
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    /// Return the length as a primitive value.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}

impl From<usize> for ArrayLength {
    fn from(value: usize) -> Self {
        Self::new(value)
    }
}

impl From<ArrayLength> for usize {
    fn from(value: ArrayLength) -> Self {
        value.get()
    }
}

/// A byte offset of a field within an alien record.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct FieldOffset(usize);

impl FieldOffset {
    /// Construct a field offset.
    #[must_use]
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    /// Return the offset as a primitive value.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}

impl From<FieldOffset> for usize {
    fn from(value: FieldOffset) -> Self {
        value.get()
    }
}

/// Whether a declared foreign routine can be called as a Lisp function.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CallableMode {
    /// The routine is callable from Lisp.
    Callable,
    /// The routine is only a declaration.
    NotCallable,
}

impl From<bool> for CallableMode {
    fn from(value: bool) -> Self {
        if value {
            Self::Callable
        } else {
            Self::NotCallable
        }
    }
}

impl From<CallableMode> for bool {
    fn from(value: CallableMode) -> Self {
        matches!(value, CallableMode::Callable)
    }
}

/// A declared foreign routine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlienRoutine {
    /// Routine name.
    name: String,
    /// Argument types, in order.
    arguments: Vec<AlienType>,
    /// Result type.
    result: AlienType,
    /// Whether the routine is callable as a Lisp function.
    callable: CallableMode,
}

impl AlienRoutine {
    /// Build a declared foreign routine.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        arguments: Vec<AlienType>,
        result: AlienType,
        callable: impl Into<CallableMode>,
    ) -> Self {
        Self {
            name: name.into(),
            arguments,
            result,
            callable: callable.into(),
        }
    }

    /// Return the routine name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the argument types in declaration order.
    #[must_use]
    pub fn arguments(&self) -> &[AlienType] {
        &self.arguments
    }

    /// Return the result type.
    #[must_use]
    pub const fn result(&self) -> &AlienType {
        &self.result
    }

    /// Return the routine's callable mode.
    #[must_use]
    pub const fn callable_mode(&self) -> CallableMode {
        self.callable
    }

    /// Return whether the routine is callable from Lisp.
    #[must_use]
    pub const fn is_callable(&self) -> bool {
        matches!(self.callable, CallableMode::Callable)
    }
}

impl AlienType {
    /// Build a pointer to `element`.
    #[must_use]
    pub fn pointer(element: Self) -> Self {
        Self::Pointer(Box::new(element))
    }

    /// Build an array of `length` `element`s.
    #[must_use]
    pub fn array(element: Self, length: impl Into<ArrayLength>) -> Self {
        Self::Array(Box::new(element), length.into())
    }

    /// Return the array length when this is an array type.
    #[must_use]
    pub const fn array_length(&self) -> Option<ArrayLength> {
        if let Self::Array(_, length) = self {
            Some(*length)
        } else {
            None
        }
    }

    /// Build a structure type.
    #[must_use]
    pub fn structure(name: impl Into<String>, fields: Vec<(String, Self)>) -> Self {
        Self::Structure(AlienRecord::new(name, fields))
    }

    /// Build a union type.
    #[must_use]
    pub fn union(name: impl Into<String>, fields: Vec<(String, Self)>) -> Self {
        Self::Union(AlienRecord::new(name, fields))
    }

    /// Build an enumeration type.
    #[must_use]
    pub fn enumeration(name: impl Into<String>, variants: Vec<(String, i64)>) -> Self {
        Self::Enumeration(AlienEnum::new(name, variants))
    }

    /// Build a function type.
    #[must_use]
    pub fn function(
        name: impl Into<String>,
        arguments: Vec<Self>,
        result: Self,
        callable: impl Into<CallableMode>,
    ) -> Self {
        Self::Function(Box::new(AlienRoutine::new(
            name, arguments, result, callable,
        )))
    }

    /// Build a function type with an explicit callable mode.
    #[must_use]
    pub fn function_with_mode(
        name: impl Into<String>,
        arguments: Vec<Self>,
        result: Self,
        callable: CallableMode,
    ) -> Self {
        Self::Function(Box::new(AlienRoutine::new(
            name, arguments, result, callable,
        )))
    }

    /// A short static label for error messages.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Void => "void",
            Self::Boolean => "boolean",
            Self::Char => "char",
            Self::UnsignedChar => "unsigned-char",
            Self::Short => "short",
            Self::UnsignedShort => "unsigned-short",
            Self::Int => "int",
            Self::UnsignedInt => "unsigned-int",
            Self::Long => "long",
            Self::UnsignedLong => "unsigned-long",
            Self::LongLong => "long-long",
            Self::UnsignedLongLong => "unsigned-long-long",
            Self::SizeT => "size-t",
            Self::SSizeT => "ssize-t",
            Self::SingleFloat => "single-float",
            Self::DoubleFloat => "double-float",
            Self::LongFloat => "long-float",
            Self::Pointer(_) => "pointer",
            Self::CString => "c-string",
            Self::Utf8String => "utf8-string",
            Self::SystemAreaPointer => "system-area-pointer",
            Self::Array(_, _) => "array",
            Self::Structure(_) => "struct",
            Self::Union(_) => "union",
            Self::Enumeration(_) => "enum",
            Self::Function(_) => "function",
        }
    }

    /// Whether the type is passed by value in a single register or stack slot.
    #[must_use]
    pub const fn is_scalar(&self) -> bool {
        matches!(
            self,
            Self::Boolean
                | Self::Char
                | Self::UnsignedChar
                | Self::Short
                | Self::UnsignedShort
                | Self::Int
                | Self::UnsignedInt
                | Self::Long
                | Self::UnsignedLong
                | Self::LongLong
                | Self::UnsignedLongLong
                | Self::SizeT
                | Self::SSizeT
                | Self::SingleFloat
                | Self::DoubleFloat
                | Self::Pointer(_)
                | Self::CString
                | Self::Utf8String
                | Self::SystemAreaPointer
                | Self::Enumeration(_)
        )
    }
}
