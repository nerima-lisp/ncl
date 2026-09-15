use crate::{ObjectError, Runtime, ThreadContext, allocate};
use crate::{
    function_offset, instance_offset, number_offset, stream_offset, structure_offset, widetag,
};
use ncl_sys::Word;

pub type StructureLayout = u32;
pub type Function = Word;
pub type Instance = Word;
pub type Bignum = Word;
pub type Ratio = Word;
pub type DoubleFloat = Word;
pub type Complex = Word;
pub type Stream = Word;
pub type Readtable = Word;
pub type CodeObject = Word;

fn put(ctx: &mut ThreadContext, object: Word, slot: usize, value: Word) -> Result<(), ObjectError> {
    ctx.write_object_slot(object, slot, value)
}

fn get(ctx: &ThreadContext, object: Word, tag: u8, slot: usize) -> Result<Word, ObjectError> {
    if ncl_sys::object_widetag(&ctx.thread, object) != Some(tag) {
        return Err(ObjectError::TypeError);
    }
    ncl_sys::read_object_word(&ctx.thread, object, slot).ok_or(ObjectError::Storage(
        ncl_sys::StorageCondition::ThreadNotRegistered,
    ))
}

fn fix(value: usize) -> Result<Word, ObjectError> {
    Ok(Word::fixnum(
        i64::try_from(value).map_err(|_| ObjectError::Layout)?,
    ))
}

impl Runtime {
    /// Allocate a monotonically increasing structure layout identifier.
    ///
    /// # Errors
    /// Returns an error when the identifier space is exhausted.
    pub fn register_structure_layout(
        &self,
        slot_count: usize,
    ) -> Result<StructureLayout, ObjectError> {
        let mut next = self
            .next_layout
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let id = *next;
        *next = next.checked_add(1).ok_or(ObjectError::Layout)?;
        drop(next);
        self.layouts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(id, slot_count);
        Ok(id)
    }

    /// Return the slot count for a registered structure layout.
    pub fn structure_layout_size(&self, id: StructureLayout) -> Option<usize> {
        self.layouts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&id)
            .copied()
    }
}

/// Construct a structure instance.
///
/// # Errors
/// Returns an error for an unknown layout or failed allocation.
pub fn make_structure(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    layout: StructureLayout,
    slots: &[Word],
) -> Result<Word, ObjectError> {
    if runtime.structure_layout_size(layout) != Some(slots.len()) {
        return Err(ObjectError::Layout);
    }
    let object = allocate(ctx, runtime, widetag::STRUCTURE, 1 + slots.len())?;
    put(ctx, object, structure_offset::LAYOUT, fix(layout as usize)?)?;
    for (index, value) in slots.iter().copied().enumerate() {
        put(ctx, object, structure_offset::SLOTS + index, value)?;
    }
    Ok(object)
}

/// Read a structure layout identifier.
///
/// # Errors
/// Returns an error for a non-structure or malformed object.
pub fn layout(ctx: &ThreadContext, object: Word) -> Result<StructureLayout, ObjectError> {
    let value = get(ctx, object, widetag::STRUCTURE, structure_offset::LAYOUT)?
        .as_fixnum()
        .ok_or(ObjectError::Layout)?;
    u32::try_from(value).map_err(|_| ObjectError::Layout)
}

/// Read a structure slot.
///
/// # Errors
/// Returns an error for a non-structure or bad index.
pub fn structure_ref(ctx: &ThreadContext, object: Word, index: usize) -> Result<Word, ObjectError> {
    get(
        ctx,
        object,
        widetag::STRUCTURE,
        structure_offset::SLOTS + index,
    )
}

/// Write a structure slot.
///
/// # Errors
/// Returns an error for a non-structure or bad index.
pub fn structure_set(
    ctx: &mut ThreadContext,
    object: Word,
    index: usize,
    value: Word,
) -> Result<(), ObjectError> {
    if ncl_sys::object_widetag(&ctx.thread, object) != Some(widetag::STRUCTURE) {
        return Err(ObjectError::TypeError);
    }
    put(ctx, object, structure_offset::SLOTS + index, value)
}

/// Construct a CLOS instance with an indirect slot vector.
///
/// # Errors
/// Returns an allocation or storage error.
pub fn make_instance(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    class: Word,
    slots: &[Word],
) -> Result<Instance, ObjectError> {
    let vector = crate::make_simple_vector(ctx, runtime, slots)?;
    let object = allocate(ctx, runtime, widetag::INSTANCE, 3)?;
    put(ctx, object, instance_offset::CLASS, class)?;
    put(ctx, object, instance_offset::SLOT_VECTOR, vector)?;
    put(ctx, object, instance_offset::GENERATION, Word::fixnum(0))?;
    Ok(object)
}

/// Read an instance class pointer.
///
/// # Errors
/// Returns an error for a non-instance or malformed object.
pub fn instance_class(ctx: &ThreadContext, object: Instance) -> Result<Word, ObjectError> {
    get(ctx, object, widetag::INSTANCE, instance_offset::CLASS)
}
/// Read an indirect instance slot.
///
/// # Errors
/// Returns an error for a non-instance or bad index.
pub fn slot_ref(ctx: &ThreadContext, object: Instance, index: usize) -> Result<Word, ObjectError> {
    let vector = get(ctx, object, widetag::INSTANCE, instance_offset::SLOT_VECTOR)?;
    crate::simple_vector_ref(ctx, vector, index)
}
/// Write an indirect instance slot.
///
/// # Errors
/// Returns an error for a non-instance or bad index.
pub fn slot_set(
    ctx: &mut ThreadContext,
    object: Instance,
    index: usize,
    value: Word,
) -> Result<(), ObjectError> {
    let vector = get(ctx, object, widetag::INSTANCE, instance_offset::SLOT_VECTOR)?;
    crate::simple_vector_set(ctx, vector, index, value)
}

/// Construct a simple function object.
///
/// # Errors
/// Returns an allocation or storage error.
pub fn make_simple_fun(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    entry: usize,
    name: Word,
    lambda_list: Word,
    code: CodeObject,
) -> Result<Function, ObjectError> {
    let object = allocate(ctx, runtime, widetag::SIMPLE_FUN, 4)?;
    put(ctx, object, function_offset::ENTRY, fix(entry)?)?;
    put(ctx, object, function_offset::NAME, name)?;
    put(ctx, object, function_offset::LAMBDA_LIST, lambda_list)?;
    put(ctx, object, function_offset::CODE, code)?;
    Ok(object)
}

/// Construct a closure with inline captured values.
///
/// # Errors
/// Returns an allocation or storage error.
pub fn make_closure(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    entry: usize,
    name: Word,
    lambda_list: Word,
    code: CodeObject,
    values: &[Word],
) -> Result<Function, ObjectError> {
    let object = allocate(
        ctx,
        runtime,
        widetag::CLOSURE,
        function_offset::CAPTURES + values.len(),
    )?;
    for (slot, value) in [
        (function_offset::ENTRY, fix(entry)?),
        (function_offset::NAME, name),
        (function_offset::LAMBDA_LIST, lambda_list),
        (function_offset::CODE, code),
    ] {
        put(ctx, object, slot, value)?;
    }
    for (index, value) in values.iter().copied().enumerate() {
        put(ctx, object, function_offset::CAPTURES + index, value)?;
    }
    Ok(object)
}

/// Read a function entry offset.
///
/// # Errors
/// Returns an error for a non-function or malformed object.
pub fn function_entry(ctx: &ThreadContext, object: Function) -> Result<usize, ObjectError> {
    let value = get_any_function(ctx, object, function_offset::ENTRY)?
        .as_fixnum()
        .ok_or(ObjectError::Layout)?;
    usize::try_from(value).map_err(|_| ObjectError::Layout)
}
/// Read a function name object.
///
/// # Errors
/// Returns an error for a non-function or malformed object.
pub fn function_name(ctx: &ThreadContext, object: Function) -> Result<Word, ObjectError> {
    get_any_function(ctx, object, function_offset::NAME)
}
fn get_any_function(ctx: &ThreadContext, object: Word, slot: usize) -> Result<Word, ObjectError> {
    match ncl_sys::object_widetag(&ctx.thread, object) {
        Some(widetag::SIMPLE_FUN | widetag::CLOSURE) => {
            ncl_sys::read_object_word(&ctx.thread, object, slot).ok_or(ObjectError::Layout)
        }
        _ => Err(ObjectError::TypeError),
    }
}
/// Read a captured closure value.
///
/// # Errors
/// Returns an error for a non-closure or bad index.
pub fn closure_ref(
    ctx: &ThreadContext,
    object: Function,
    index: usize,
) -> Result<Word, ObjectError> {
    get(
        ctx,
        object,
        widetag::CLOSURE,
        function_offset::CAPTURES + index,
    )
}

/// Construct a bignum from a signed 128-bit integer.
///
/// # Errors
/// Returns an allocation or storage error.
pub fn make_bignum_from_i128(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: i128,
) -> Result<Bignum, ObjectError> {
    let sign = i64::from(value.is_negative());
    let magnitude = value.unsigned_abs();
    let mut limbs = Vec::new();
    let mut rest = magnitude;
    while rest != 0 {
        limbs.push(u32::try_from(rest & u128::from(u32::MAX)).map_err(|_| ObjectError::Layout)?);
        rest >>= 32;
    }
    let object = allocate(
        ctx,
        runtime,
        widetag::BIGNUM,
        number_offset::LIMBS + limbs.len(),
    )?;
    put(ctx, object, number_offset::SIGN, Word::fixnum(sign))?;
    put(ctx, object, number_offset::LIMB_COUNT, fix(limbs.len())?)?;
    for (index, limb) in limbs.into_iter().enumerate() {
        put(
            ctx,
            object,
            number_offset::LIMBS + index,
            Word::fixnum(i64::from(limb)),
        )?;
    }
    Ok(object)
}
/// Read little-endian bignum limbs.
///
/// # Errors
/// Returns an error for a non-bignum or malformed payload.
pub fn bignum_limbs(ctx: &ThreadContext, object: Bignum) -> Result<Vec<u32>, ObjectError> {
    let count = get(ctx, object, widetag::BIGNUM, number_offset::LIMB_COUNT)?
        .as_fixnum()
        .ok_or(ObjectError::Layout)?;
    let count = usize::try_from(count).map_err(|_| ObjectError::Layout)?;
    (0..count)
        .map(|i| {
            get(ctx, object, widetag::BIGNUM, number_offset::LIMBS + i)?
                .as_fixnum()
                .and_then(|x| u32::try_from(x).ok())
                .ok_or(ObjectError::Layout)
        })
        .collect()
}
/// Construct a ratio from numerator and denominator references.
///
/// # Errors
/// Returns an allocation or storage error.
pub fn make_ratio(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    numerator: Word,
    denominator: Word,
) -> Result<Ratio, ObjectError> {
    let object = allocate(ctx, runtime, widetag::RATIO, 2)?;
    put(ctx, object, number_offset::RATIO_NUMERATOR, numerator)?;
    put(ctx, object, number_offset::RATIO_DENOMINATOR, denominator)?;
    Ok(object)
}
const fn split(bits: u64) -> (Word, Word) {
    (
        Word::fixnum((bits & 4_294_967_295_u64).cast_signed()),
        Word::fixnum((bits >> 32).cast_signed()),
    )
}
fn join(low: Word, high: Word) -> Result<u64, ObjectError> {
    let lo = u64::try_from(low.as_fixnum().ok_or(ObjectError::Layout)?)
        .map_err(|_| ObjectError::Layout)?;
    let hi = u64::try_from(high.as_fixnum().ok_or(ObjectError::Layout)?)
        .map_err(|_| ObjectError::Layout)?;
    Ok(lo | (hi << 32))
}
/// Construct a boxed binary64 object.
///
/// # Errors
/// Returns an allocation or storage error.
pub fn make_double(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: f64,
) -> Result<DoubleFloat, ObjectError> {
    let object = allocate(ctx, runtime, widetag::DOUBLE_FLOAT, 2)?;
    let (lo, hi) = split(value.to_bits());
    put(ctx, object, 0, lo)?;
    put(ctx, object, 1, hi)?;
    Ok(object)
}
/// Read a boxed binary64 value.
///
/// # Errors
/// Returns an error for a non-double or malformed payload.
pub fn double_value(ctx: &ThreadContext, object: DoubleFloat) -> Result<f64, ObjectError> {
    Ok(f64::from_bits(join(
        get(ctx, object, widetag::DOUBLE_FLOAT, 0)?,
        get(ctx, object, widetag::DOUBLE_FLOAT, 1)?,
    )?))
}
/// Construct a complex object from real and imaginary references.
///
/// # Errors
/// Returns an allocation or storage error.
pub fn make_complex(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    real: Word,
    imag: Word,
) -> Result<Complex, ObjectError> {
    let object = allocate(ctx, runtime, widetag::COMPLEX, 2)?;
    put(ctx, object, 0, real)?;
    put(ctx, object, 1, imag)?;
    Ok(object)
}

/// Construct a stream object.
///
/// # Errors
/// Returns an allocation or storage error.
pub fn make_stream(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    direction: Word,
    element_type: Word,
    external_format: Word,
    state: Word,
    implementation: Word,
) -> Result<Stream, ObjectError> {
    let object = allocate(ctx, runtime, widetag::STREAM, 5)?;
    for (i, v) in [
        direction,
        element_type,
        external_format,
        state,
        implementation,
    ]
    .into_iter()
    .enumerate()
    {
        put(ctx, object, i, v)?;
    }
    Ok(object)
}
/// Read a stream state reference.
///
/// # Errors
/// Returns an error for a non-stream or malformed object.
pub fn stream_state(ctx: &ThreadContext, object: Stream) -> Result<Word, ObjectError> {
    get(ctx, object, widetag::STREAM, stream_offset::STATE)
}
/// Construct a readtable object.
///
/// # Errors
/// Returns an allocation or storage error.
pub fn make_readtable(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    syntax: Word,
    dispatch: Word,
    case_mode: Word,
) -> Result<Readtable, ObjectError> {
    let object = allocate(ctx, runtime, widetag::READTABLE, 3)?;
    for (i, v) in [syntax, dispatch, case_mode].into_iter().enumerate() {
        put(ctx, object, i, v)?;
    }
    Ok(object)
}
/// Construct a code-object metadata record.
///
/// # Errors
/// Returns an allocation or storage error.
pub fn make_code_object(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    entry: usize,
    size: usize,
    constants: Word,
    stack_map: Word,
    debug: Word,
) -> Result<CodeObject, ObjectError> {
    let object = allocate(ctx, runtime, widetag::CODE, 5)?;
    for (i, v) in [fix(entry)?, fix(size)?, constants, stack_map, debug]
        .into_iter()
        .enumerate()
    {
        put(ctx, object, i, v)?;
    }
    Ok(object)
}
