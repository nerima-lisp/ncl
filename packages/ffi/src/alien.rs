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
pub use size::{align_of, offset_of, record_size, size_of, union_size};

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
    Array(Box<Self>, usize),
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
    pub name: String,
    /// Field name and type, in declaration order.
    pub fields: Vec<(String, AlienType)>,
}

/// A named enumeration with explicit variant values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlienEnum {
    /// Enumeration name.
    pub name: String,
    /// Variant name and value.
    pub variants: Vec<(String, i64)>,
}

/// A declared foreign routine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlienRoutine {
    /// Routine name.
    pub name: String,
    /// Argument types, in order.
    pub arguments: Vec<AlienType>,
    /// Result type.
    pub result: AlienType,
    /// Whether the routine is callable as a Lisp function.
    pub callable: bool,
}

impl AlienType {
    /// Build a pointer to `element`.
    #[must_use]
    pub fn pointer(element: Self) -> Self {
        Self::Pointer(Box::new(element))
    }

    /// Build an array of `length` `element`s.
    #[must_use]
    pub fn array(element: Self, length: usize) -> Self {
        Self::Array(Box::new(element), length)
    }

    /// Build a structure type.
    #[must_use]
    pub fn structure(name: impl Into<String>, fields: Vec<(String, Self)>) -> Self {
        Self::Structure(AlienRecord {
            name: name.into(),
            fields,
        })
    }

    /// Build a union type.
    #[must_use]
    pub fn union(name: impl Into<String>, fields: Vec<(String, Self)>) -> Self {
        Self::Union(AlienRecord {
            name: name.into(),
            fields,
        })
    }

    /// Build an enumeration type.
    #[must_use]
    pub fn enumeration(name: impl Into<String>, variants: Vec<(String, i64)>) -> Self {
        Self::Enumeration(AlienEnum {
            name: name.into(),
            variants,
        })
    }

    /// Build a function type.
    #[must_use]
    pub fn function(
        name: impl Into<String>,
        arguments: Vec<Self>,
        result: Self,
        callable: bool,
    ) -> Self {
        Self::Function(Box::new(AlienRoutine {
            name: name.into(),
            arguments,
            result,
            callable,
        }))
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
