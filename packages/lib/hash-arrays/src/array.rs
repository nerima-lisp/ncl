//! Typed array domain and builtin adapters.
//!
//! The registration module is intentionally kept separate from the domain
//! types so that array arithmetic and bounds checks do not depend on the
//! tagged-word ABI.

use ncl_object::{
    classify_object, ArrayElementType, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier,
    BuiltinImplementation, BuiltinName, BuiltinPackage, Fixnum, LambdaList, LispError,
    MultipleValues, ObjectError, ObjectType, Parameter, ParameterType, Runtime, ThreadContext,
    Word,
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
    use ncl_object::{array_dimensions, array_row_major_ref, Array, Fixnum, ThreadContext, Word};

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

fn array_word(ctx: &ThreadContext, word: Word) -> Result<Word, ObjectError> {
    match classify_object(ctx, word) {
        ncl_object::ObjectRef::Array(_)
        | ncl_object::ObjectRef::SimpleVector(_)
        | ncl_object::ObjectRef::SpecializedArray(_) => Ok(word),
        _ => Err(ObjectError::TypeError),
    }
}

fn rank_dimensions(ctx: &ThreadContext, array: Word) -> Result<Vec<usize>, ObjectError> {
    match classify_object(ctx, array) {
        ncl_object::ObjectRef::Array(_) => ncl_object::array_dimensions(ctx, array),
        ncl_object::ObjectRef::SimpleVector(_) => {
            Ok(vec![ncl_object::simple_vector_length(ctx, array)?])
        }
        ncl_object::ObjectRef::SpecializedArray(_) => {
            let mut length = 0;
            while ncl_object::specialized_array_ref(ctx, array, length).is_ok() {
                length += 1;
            }
            Ok(vec![length])
        }
        _ => Err(ObjectError::TypeError),
    }
}

fn row_ref(ctx: &ThreadContext, array: Word, index: usize) -> Result<Word, ObjectError> {
    match classify_object(ctx, array) {
        ncl_object::ObjectRef::Array(_) => ncl_object::array_row_major_ref(ctx, array, index),
        ncl_object::ObjectRef::SimpleVector(_) => ncl_object::simple_vector_ref(ctx, array, index),
        ncl_object::ObjectRef::SpecializedArray(_) => {
            ncl_object::specialized_array_ref(ctx, array, index)
        }
        _ => Err(ObjectError::TypeError),
    }
}

fn fixnum(word: Word) -> Result<usize, ObjectError> {
    usize::try_from(
        Fixnum::try_from_word(word)
            .map_err(|_| ObjectError::TypeError)?
            .value(),
    )
    .map_err(|_| ObjectError::TypeError)
}

fn total(ctx: &ThreadContext, array: Word) -> Result<usize, ObjectError> {
    rank_dimensions(ctx, array)?
        .into_iter()
        .try_fold(1_usize, |a, b| a.checked_mul(b).ok_or(ObjectError::Layout))
}

fn array_rank_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let rank = rank_dimensions(ctx, array_word(ctx, args.required(0)?)?)?.len();
    i64::try_from(rank)
        .map(Word::fixnum)
        .map_err(|_| ObjectError::Layout)
}

fn array_dimensions_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let dimensions = rank_dimensions(ctx, array_word(ctx, args.required(0)?)?)?;
    let values = dimensions
        .into_iter()
        .map(|v| {
            i64::try_from(v)
                .map(Word::fixnum)
                .map_err(|_| ObjectError::Layout)
        })
        .collect::<Result<Vec<_>, _>>()?;
    ncl_object::make_simple_vector(ctx, runtime, &values)
}

fn array_total_size_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    i64::try_from(total(ctx, array_word(ctx, args.required(0)?)?)?)
        .map(Word::fixnum)
        .map_err(|_| ObjectError::Layout)
}

fn symbol_name_is(ctx: &ThreadContext, word: Word, expected: &str) -> Result<bool, ObjectError> {
    if !matches!(classify_object(ctx, word), ncl_object::ObjectRef::Symbol(_)) {
        return Ok(false);
    }
    let name = ncl_object::symbol_name(ctx, word)?;
    if ncl_object::string_length(ctx, name)? != expected.len() {
        return Ok(false);
    }
    expected
        .chars()
        .enumerate()
        .try_fold(true, |same, (index, character)| {
            Ok(same && ncl_object::string_ref(ctx, name, index)? == character)
        })
}

fn dimensions_argument(ctx: &mut ThreadContext, word: Word) -> Result<Vec<usize>, ObjectError> {
    if let Some(value) = word.as_fixnum() {
        return Ok(vec![usize::try_from(value).map_err(|_| ObjectError::TypeError)?]);
    }
    let mut dimensions = Vec::new();
    let mut cursor = word;
    while cursor != Word::NIL {
        let dimension = ncl_object::car(ctx, cursor)?
            .as_fixnum()
            .ok_or(ObjectError::TypeError)?;
        dimensions.push(usize::try_from(dimension).map_err(|_| ObjectError::TypeError)?);
        cursor = ncl_object::cdr(ctx, cursor)?;
    }
    if dimensions.is_empty() {
        return Err(ObjectError::TypeError);
    }
    Ok(dimensions)
}

fn keyword(ctx: &mut ThreadContext, args: &BuiltinArgs<'_>, name: &str) -> Result<Option<Word>, ObjectError> {
    if !(args.len() - 1).is_multiple_of(2) {
        return Err(ObjectError::TypeError);
    }
    for index in (1..args.len()).step_by(2) {
        if symbol_name_is(ctx, args.required(index)?, name)? {
            return Ok(Some(args.required(index + 1)?));
        }
    }
    Ok(None)
}

fn element_type(ctx: &mut ThreadContext, word: Word) -> Result<ArrayElementType, ObjectError> {
    for (name, kind) in [
        ("T", ArrayElementType::T),
        ("BIT", ArrayElementType::Bit),
        ("CHARACTER", ArrayElementType::Character),
        ("BASE-CHAR", ArrayElementType::BaseChar),
        ("FIXNUM", ArrayElementType::Fixnum),
        ("SIGNED-BYTE", ArrayElementType::Signed),
        ("UNSIGNED-BYTE", ArrayElementType::Unsigned),
        ("SINGLE-FLOAT", ArrayElementType::SingleFloat),
        ("DOUBLE-FLOAT", ArrayElementType::DoubleFloat),
    ] {
        if symbol_name_is(ctx, word, name)? {
            return Ok(kind);
        }
    }
    Err(ObjectError::TypeError)
}

fn make_array_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let dimensions = dimensions_argument(ctx, args.required(0)?)?;
    let fill_pointer = keyword(ctx, args, "FILL-POINTER")?
        .filter(|word| *word != Word::NIL)
        .map(fixnum)
        .transpose()?;
    let displaced_to = keyword(ctx, args, "DISPLACED-TO")?;
    let offset = keyword(ctx, args, "DISPLACED-INDEX-OFFSET")?
        .map(fixnum)
        .transpose()?
        .unwrap_or(0);
    let adjustable = keyword(ctx, args, "ADJUSTABLE")?
        .is_some_and(|word| word != Word::NIL);
    let initial_element = keyword(ctx, args, "INITIAL-ELEMENT")?.unwrap_or(Word::NIL);
    let kind = keyword(ctx, args, "ELEMENT-TYPE")?
        .map(|word| element_type(ctx, word))
        .transpose()?
        .unwrap_or(ArrayElementType::T);
    ncl_object::make_array(
        ctx,
        runtime,
        &dimensions,
        ncl_object::ArrayOptions {
            element_type: kind,
            initial_element,
            adjustable,
            fill_pointer,
            displaced_to,
            displaced_index_offset: offset,
        },
    )
}

fn adjust_array_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let old = array_word(ctx, args.required(0)?)?;
    let dimensions = dimensions_argument(ctx, args.required(1)?)?;
    let kind = keyword(ctx, args, "ELEMENT-TYPE")?
        .map(|word| element_type(ctx, word))
        .transpose()?
        .unwrap_or(ArrayElementType::T);
    let result = ncl_object::make_array(
        ctx,
        runtime,
        &dimensions,
        ncl_object::ArrayOptions {
            element_type: kind,
            initial_element: Word::NIL,
            adjustable: true,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )?;
    let count = total(ctx, old)?.min(total(ctx, result)?);
    for index in 0..count {
        let value = row_ref(ctx, old, index)?;
        ncl_object::array_row_major_set(ctx, result, index, value)?;
    }
    values.clear();
    Ok(result)
}

fn aref_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let array = array_word(ctx, args.required(0)?)?;
    let dimensions = rank_dimensions(ctx, array)?;
    if args.len() != dimensions.len() + 1 {
        return Err(ObjectError::TypeError);
    }
    let mut index = 0usize;
    for (ordinal, dimension) in dimensions.into_iter().enumerate() {
        let word = args.get(ordinal + 1).ok_or(ObjectError::TypeError)?;
        let subscript = fixnum(word)?;
        if subscript >= dimension {
            return Err(ObjectError::TypeError);
        }
        index = index
            .checked_mul(dimension)
            .and_then(|v| v.checked_add(subscript))
            .ok_or(ObjectError::Layout)?;
    }
    row_ref(ctx, array, index)
}

fn row_major_aref_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let array = array_word(ctx, args.required(0)?)?;
    row_ref(ctx, array, fixnum(args.required(1)?)?)
}

fn svref_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let array = args.required(0)?;
    if !matches!(
        classify_object(ctx, array),
        ncl_object::ObjectRef::SimpleVector(_)
    ) {
        return Err(ObjectError::TypeError);
    }
    ncl_object::simple_vector_ref(ctx, array, fixnum(args.required(1)?)?)
}

fn bit_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let array = args.required(0)?;
    if !matches!(
        classify_object(ctx, array),
        ncl_object::ObjectRef::SpecializedArray(_)
    ) || ncl_object::specialized_array_element_type(ctx, array)? != ArrayElementType::Bit
    {
        return Err(ObjectError::TypeError);
    }
    ncl_object::specialized_array_ref(ctx, array, fixnum(args.required(1)?)?)
}

fn sbit_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let array = args.required(0)?;
    if !matches!(
        classify_object(ctx, array),
        ncl_object::ObjectRef::SpecializedArray(_)
    ) || ncl_object::specialized_array_element_type(ctx, array)? != ArrayElementType::Bit
    {
        return Err(ObjectError::TypeError);
    }
    let index = fixnum(args.required(1)?)?;
    let value = args.required(2)?;
    ncl_object::specialized_array_set(ctx, array, index, value)?;
    Ok(value)
}

fn bit_binary(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    op: fn(bool, bool) -> bool,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let left = args.required(0)?;
    let right = args.required(1)?;
    if ncl_object::specialized_array_element_type(ctx, left)? != ArrayElementType::Bit
        || ncl_object::specialized_array_element_type(ctx, right)? != ArrayElementType::Bit
    {
        return Err(ObjectError::TypeError);
    }
    let shape = rank_dimensions(ctx, left)?;
    if shape != rank_dimensions(ctx, right)? {
        return Err(ObjectError::TypeError);
    }
    let mut values = Vec::with_capacity(total(ctx, left)?);
    for index in 0..total(ctx, left)? {
        let a = row_ref(ctx, left, index)?
            .as_fixnum()
            .ok_or(ObjectError::TypeError)?
            == 1;
        let b = row_ref(ctx, right, index)?
            .as_fixnum()
            .ok_or(ObjectError::TypeError)?
            == 1;
        values.push(Word::fixnum(i64::from(op(a, b))));
    }
    ncl_object::make_specialized_array(ctx, runtime, ArrayElementType::Bit, &values)
}

fn bit_and(
    ctx: &mut ThreadContext,
    r: &Runtime,
    a: &BuiltinArgs<'_>,
    v: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    bit_binary(ctx, r, a, |x, y| x & y, v)
}
fn bit_ior(
    ctx: &mut ThreadContext,
    r: &Runtime,
    a: &BuiltinArgs<'_>,
    v: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    bit_binary(ctx, r, a, |x, y| x | y, v)
}
fn bit_xor(
    ctx: &mut ThreadContext,
    r: &Runtime,
    a: &BuiltinArgs<'_>,
    v: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    bit_binary(ctx, r, a, |x, y| x ^ y, v)
}

fn register_one(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &'static str,
    arity: u8,
    function: ncl_object::RustBuiltin,
) -> Result<(), ObjectError> {
    let parameters: &'static [Parameter] = Box::leak(
        (0..usize::from(arity))
            .map(|_| Parameter {
                name: BuiltinName::new("ARG"),
                ty: ParameterType::Any,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    let descriptor = Builtin {
        lambda_list: LambdaList::fixed(parameters),
        convention: BuiltinConvention::Direct(ncl_object::Arity::exact(arity)),
    };
    runtime
        .register_builtin(
            ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            BuiltinImplementation::direct(descriptor, function),
        )
        .map(|_| ())
}

fn register_aref(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    const REQUIRED: &[Parameter] = &[Parameter {
        name: BuiltinName::new("ARRAY"),
        ty: ParameterType::Any,
    }];
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("AREF")),
        BuiltinImplementation::direct(
            Builtin {
                lambda_list: LambdaList::with_rest(
                    REQUIRED,
                    Parameter { name: BuiltinName::new("INDEX"), ty: ParameterType::Any },
                ),
                convention: BuiltinConvention::Direct(ncl_object::Arity::exact(2)),
            },
            aref_builtin,
        ),
    )?;
    Ok(())
}

/// Register the array/vector/bit builtins implemented in this module.
pub fn register(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    const MAKE_ARRAY_KEYS: &[Parameter] = &[
        Parameter { name: BuiltinName::new("ELEMENT-TYPE"), ty: ParameterType::Any },
        Parameter { name: BuiltinName::new("INITIAL-ELEMENT"), ty: ParameterType::Any },
        Parameter { name: BuiltinName::new("ADJUSTABLE"), ty: ParameterType::Any },
        Parameter { name: BuiltinName::new("FILL-POINTER"), ty: ParameterType::Any },
        Parameter { name: BuiltinName::new("DISPLACED-TO"), ty: ParameterType::Any },
        Parameter { name: BuiltinName::new("DISPLACED-INDEX-OFFSET"), ty: ParameterType::Any },
    ];
    const MAKE_ARRAY_REQUIRED: &[Parameter] = &[Parameter {
        name: BuiltinName::new("DIMENSIONS"),
        ty: ParameterType::Any,
    }];
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("MAKE-ARRAY")),
        BuiltinImplementation::direct(
            Builtin {
                lambda_list: LambdaList::with_keys(MAKE_ARRAY_REQUIRED, MAKE_ARRAY_KEYS, false),
                convention: BuiltinConvention::Adapted,
            },
            make_array_builtin,
        ),
    )?;
    const ADJUST_ARRAY_KEYS: &[Parameter] = &[Parameter {
        name: BuiltinName::new("ELEMENT-TYPE"),
        ty: ParameterType::Any,
    }];
    const ADJUST_ARRAY_REQUIRED: &[Parameter] = &[
        Parameter { name: BuiltinName::new("ARRAY"), ty: ParameterType::Any },
        Parameter { name: BuiltinName::new("DIMENSIONS"), ty: ParameterType::Any },
    ];
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("ADJUST-ARRAY")),
        BuiltinImplementation::direct(
            Builtin {
                lambda_list: LambdaList::with_keys(ADJUST_ARRAY_REQUIRED, ADJUST_ARRAY_KEYS, false),
                convention: BuiltinConvention::Adapted,
            },
            adjust_array_builtin,
        ),
    )?;
    register_aref(ctx, runtime)?;
    for (name, arity, function) in [
        (
            "ARRAY-RANK",
            1,
            array_rank_builtin as ncl_object::RustBuiltin,
        ),
        ("ARRAY-DIMENSIONS", 1, array_dimensions_builtin),
        ("ARRAY-TOTAL-SIZE", 1, array_total_size_builtin),
        ("ROW-MAJOR-AREF", 2, row_major_aref_builtin),
        ("SVREF", 2, svref_builtin),
        ("BIT", 2, bit_builtin),
        ("SBIT", 3, sbit_builtin),
        ("BIT-AND", 2, bit_and),
        ("BIT-IOR", 2, bit_ior),
        ("BIT-XOR", 2, bit_xor),
    ] {
        register_one(ctx, runtime, name, arity, function)?;
    }
    Ok(())
}

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
