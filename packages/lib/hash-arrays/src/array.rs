//! Typed array domain and builtin adapters.
//!
//! The registration module is intentionally kept separate from the domain
//! types so that array arithmetic and bounds checks do not depend on the
//! tagged-word ABI.

use ncl_object::{
    ArrayElementType, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier,
    BuiltinImplementation, BuiltinName, BuiltinPackage, Fixnum, LambdaList, LispError,
    MultipleValues, ObjectError, ObjectType, Parameter, ParameterType, Runtime, ThreadContext,
    Word, classify_object,
};

/// A non-negative array dimension.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Dimension(usize);

impl Dimension {
    /// Construct a dimension from its mathematical value.
    #[must_use]
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    /// Return the dimension's value.
    #[must_use]
    pub const fn value(self) -> usize {
        self.0
    }
}

/// A checked sequence of dimensions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dimensions(Vec<Dimension>);

impl Dimensions {
    /// Create dimensions and reject an empty rank description.
    pub fn new(values: impl IntoIterator<Item = Dimension>) -> Result<Self, LispError> {
        let values = values.into_iter().collect::<Vec<_>>();
        if values.is_empty() {
            return Err(LispError::ProgramError(
                ncl_object::ProgramError::WrongNumberOfArguments {
                    minimum: 1,
                    maximum: None,
                },
            ));
        }
        Ok(Self(values))
    }

    /// Return the array rank.
    #[must_use]
    pub fn rank(&self) -> usize {
        self.0.len()
    }

    /// Return one dimension after checking its ordinal.
    pub fn dimension(&self, ordinal: usize) -> Result<Dimension, LispError> {
        self.0.get(ordinal).copied().ok_or(LispError::TypeError {
            datum: Word::fixnum(i64::try_from(ordinal).unwrap_or(i64::MAX)),
            expected: ObjectType::Fixnum,
        })
    }

    /// Calculate the total number of elements with overflow checking.
    pub fn total_size(&self) -> Result<usize, LispError> {
        self.0.iter().try_fold(1_usize, |total, dimension| {
            total
                .checked_mul(dimension.value())
                .ok_or(LispError::Object(ncl_object::ObjectError::Layout))
        })
    }

    /// Calculate a row-major offset from checked subscripts.
    pub fn row_major_index(&self, indices: &[Dimension]) -> Result<RowMajorIndex, LispError> {
        if indices.len() != self.rank() {
            return Err(LispError::ProgramError(
                ncl_object::ProgramError::WrongNumberOfArguments {
                    minimum: self.rank(),
                    maximum: Some(self.rank()),
                },
            ));
        }
        let mut result = 0_usize;
        for (index, dimension) in indices.iter().zip(&self.0) {
            if index.value() >= dimension.value() {
                return Err(LispError::Object(ncl_object::ObjectError::TypeError));
            }
            result = result
                .checked_mul(dimension.value())
                .and_then(|value| value.checked_add(index.value()))
                .ok_or(LispError::Object(ncl_object::ObjectError::Layout))?;
        }
        Ok(RowMajorIndex(result))
    }
}

/// A row-major offset known to be within an array.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct RowMajorIndex(usize);

impl RowMajorIndex {
    /// Validate an offset against a total size.
    pub fn new(value: usize, total_size: usize) -> Result<Self, LispError> {
        if value < total_size {
            Ok(Self(value))
        } else {
            Err(LispError::Object(ncl_object::ObjectError::TypeError))
        }
    }

    /// Return the checked offset.
    #[must_use]
    pub const fn value(self) -> usize {
        self.0
    }
}

/// The element types represented by the object-layer array ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ElementType {
    /// General Lisp object elements.
    T,
    /// One-bit elements.
    Bit,
    /// Character elements.
    Character,
    /// Base-character elements.
    BaseChar,
    /// Fixnum elements.
    Fixnum,
    /// Signed integer elements.
    Signed,
    /// Unsigned integer elements.
    Unsigned,
    /// Single-float elements.
    SingleFloat,
    /// Double-float elements.
    DoubleFloat,
}

impl From<ArrayElementType> for ElementType {
    fn from(value: ArrayElementType) -> Self {
        match value {
            ArrayElementType::T => Self::T,
            ArrayElementType::Bit => Self::Bit,
            ArrayElementType::Character => Self::Character,
            ArrayElementType::BaseChar => Self::BaseChar,
            ArrayElementType::Fixnum => Self::Fixnum,
            ArrayElementType::Signed => Self::Signed,
            ArrayElementType::Unsigned => Self::Unsigned,
            ArrayElementType::SingleFloat => Self::SingleFloat,
            ArrayElementType::DoubleFloat => Self::DoubleFloat,
        }
    }
}

/// Typed array metadata and row-major operations.
pub mod builtins {
    use super::{Dimension, Dimensions, LispError, RowMajorIndex};
    use ncl_object::{Array, Fixnum, ThreadContext, Word, array_dimensions, array_row_major_ref};

    /// Convert a builtin argument into an array view at the ABI boundary.
    pub fn array_argument(ctx: &ThreadContext, word: Word) -> Result<Array, LispError> {
        match ncl_object::classify_object(ctx, word) {
            ncl_object::ObjectRef::Array(value) => Ok(Array::from_word(value)),
            _ => Err(LispError::TypeError {
                datum: word,
                expected: ncl_object::ObjectType::Array,
            }),
        }
    }

    /// Return an array's rank.
    pub fn array_rank(ctx: &ThreadContext, array: Array) -> Result<Fixnum, LispError> {
        let dimensions = Dimensions::new(
            array_dimensions(ctx, array.as_word())?
                .into_iter()
                .map(Dimension::new),
        )?;
        Fixnum::try_from_word(Word::fixnum(
            i64::try_from(dimensions.rank()).map_err(|_| ncl_object::ObjectError::Layout)?,
        ))
        .map_err(LispError::from)
    }

    /// Return an array's checked dimensions.
    pub fn dimensions(ctx: &ThreadContext, array: Array) -> Result<Dimensions, LispError> {
        Dimensions::new(
            array_dimensions(ctx, array.as_word())?
                .into_iter()
                .map(Dimension::new),
        )
    }

    /// Read an element through a checked row-major index.
    pub fn row_major_aref(
        ctx: &ThreadContext,
        array: Array,
        index: RowMajorIndex,
    ) -> Result<Word, LispError> {
        array_row_major_ref(ctx, array.as_word(), index.value()).map_err(LispError::from)
    }

    /// Convert a fixnum subscript into a checked dimension value.
    pub fn subscript(value: Fixnum) -> Result<Dimension, LispError> {
        usize::try_from(value.value())
            .map(Dimension::new)
            .map_err(|_| LispError::Object(ncl_object::ObjectError::TypeError))
    }

    /// Convert an array and row-major fixnum into a checked index.
    pub fn row_major_index(
        ctx: &ThreadContext,
        array: Array,
        value: Fixnum,
    ) -> Result<RowMajorIndex, LispError> {
        let dimensions = dimensions(ctx, array)?;
        let total = dimensions.total_size()?;
        let index = usize::try_from(value.value())
            .map_err(|_| LispError::Object(ncl_object::ObjectError::TypeError))?;
        RowMajorIndex::new(index, total)
    }
}

/// Descriptors and registration hooks for array builtins.
pub mod register {
    use ncl_object::{Builtin, BuiltinConvention, LambdaList, Parameter, ParameterType};

    /// Descriptor for fixed one-array metadata builtins.
    #[must_use]
    pub const fn one_array_descriptor() -> Builtin {
        const REQUIRED: &[Parameter] = &[Parameter {
            name: ncl_object::BuiltinName::new("array"),
            ty: ParameterType::Any,
        }];
        Builtin {
            lambda_list: LambdaList::fixed(REQUIRED),
            convention: BuiltinConvention::Direct(ncl_object::Arity::exact(1)),
        }
    }
}

#[path = "array_helpers.rs"]
mod array_helpers;
#[path = "array_builtins.rs"]
mod builtins_impl;
#[path = "array_register.rs"]
mod register_impl;
pub use register_impl::register;

#[cfg(test)]
mod tests {
    use super::{Dimension, Dimensions, RowMajorIndex};

    #[test]
    fn dimensions_calculate_checked_row_major_index() {
        let dimensions = Dimensions::new([Dimension::new(2), Dimension::new(3)]).unwrap();
        assert_eq!(dimensions.total_size().unwrap(), 6);
        assert_eq!(
            dimensions
                .row_major_index(&[Dimension::new(1), Dimension::new(2)])
                .unwrap()
                .value(),
            5
        );
    }

    #[test]
    fn row_major_index_rejects_out_of_bounds_values() {
        assert!(RowMajorIndex::new(3, 3).is_err());
    }

    #[test]
    fn empty_dimensions_are_rejected() {
        assert!(Dimensions::new([]).is_err());
    }
}
