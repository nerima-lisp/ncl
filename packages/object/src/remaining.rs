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
pub fn layout(ctx: &ThreadContext, object: Word) -> Result<StructureLayout, ObjectError> {
    let value = get(ctx, object, widetag::STRUCTURE, structure_offset::LAYOUT)?
        .as_fixnum()
        .ok_or(ObjectError::Layout)?;
    u32::try_from(value).map_err(|_| ObjectError::Layout)
}
pub fn structure_ref(ctx: &ThreadContext, object: Word, index: usize) -> Result<Word, ObjectError> {
    get(
        ctx,
        object,
        widetag::STRUCTURE,
        structure_offset::SLOTS + index,
    )
}
pub fn structure_set(
    ctx: &mut ThreadContext,
    object: Word,
    index: usize,
    value: Word,
) -> Result<(), ObjectError> {
    if ncl_sys::object_widetag(&ctx.thread, object) != Some(widetag::STRUCTURE) {
        return Err(ObjectError::TypeError);
    }
    put(ctx, object, structure_offset::SLOTS + index, value)?;
    ncl_sys::write_barrier(&mut ctx.thread, object, structure_offset::SLOTS + index);
    Ok(())
}
pub fn make_instance(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    class: Word,
    slots: &[Word],
) -> Result<Instance, ObjectError> {
    let mut vector = crate::make_simple_vector(ctx, runtime, slots)?;
    let token = crate::push_root(ctx, &mut vector);
    let object = match allocate(ctx, runtime, widetag::INSTANCE, 3) {
        Ok(object) => object,
        Err(error) => {
            let _ = crate::pop_root(ctx, token);
            return Err(error);
        }
    };
    put(ctx, object, instance_offset::CLASS, class)?;
    put(ctx, object, instance_offset::SLOT_VECTOR, vector)?;
    put(ctx, object, instance_offset::GENERATION, Word::fixnum(0))?;
    let _ = crate::pop_root(ctx, token);
    Ok(object)
}
pub fn instance_class(ctx: &ThreadContext, object: Instance) -> Result<Word, ObjectError> {
    get(ctx, object, widetag::INSTANCE, instance_offset::CLASS)
}
pub fn slot_ref(ctx: &ThreadContext, object: Instance, index: usize) -> Result<Word, ObjectError> {
    let vector = get(ctx, object, widetag::INSTANCE, instance_offset::SLOT_VECTOR)?;
    crate::simple_vector_ref(ctx, vector, index)
}
pub fn slot_set(
    ctx: &mut ThreadContext,
    object: Instance,
    index: usize,
    value: Word,
) -> Result<(), ObjectError> {
    let vector = get(ctx, object, widetag::INSTANCE, instance_offset::SLOT_VECTOR)?;
    crate::simple_vector_set(ctx, vector, index, value)
}
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
pub fn function_entry(ctx: &ThreadContext, object: Function) -> Result<usize, ObjectError> {
    let value = get_any_function(ctx, object, function_offset::ENTRY)?
        .as_fixnum()
        .ok_or(ObjectError::Layout)?;
    usize::try_from(value).map_err(|_| ObjectError::Layout)
}
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
        number_offset::LIMBS + limbs.len().div_ceil(2),
    )?;
    put(
        ctx,
        object,
        number_offset::SIGN,
        Word::from_bits(sign.cast_unsigned()),
    )?;
    put(ctx, object, number_offset::LIMB_COUNT, fix(limbs.len())?)?;
    for (index, pair) in limbs.chunks(2).enumerate() {
        let low = u64::from(pair[0]);
        let high = pair.get(1).map_or(0, |limb| u64::from(*limb));
        put(
            ctx,
            object,
            number_offset::LIMBS + index,
            Word::from_bits(low | (high << 32)),
        )?;
    }
    Ok(object)
}
pub fn bignum_limbs(ctx: &ThreadContext, object: Bignum) -> Result<Vec<u32>, ObjectError> {
    let count = get(ctx, object, widetag::BIGNUM, number_offset::LIMB_COUNT)?
        .as_fixnum()
        .ok_or(ObjectError::Layout)?;
    let count = usize::try_from(count).map_err(|_| ObjectError::Layout)?;
    (0..count)
        .map(|i| {
            let bits = get(ctx, object, widetag::BIGNUM, number_offset::LIMBS + i / 2)?.bits();
            let limb = if i % 2 == 0 {
                u32::try_from(bits & u64::from(u32::MAX)).map_err(|_| ObjectError::Layout)?
            } else {
                u32::try_from(bits >> 32).map_err(|_| ObjectError::Layout)?
            };
            Ok(limb)
        })
        .collect()
}
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
pub fn make_double(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: f64,
) -> Result<DoubleFloat, ObjectError> {
    let object = allocate(ctx, runtime, widetag::DOUBLE_FLOAT, 1)?;
    put(
        ctx,
        object,
        number_offset::DOUBLE_BITS,
        Word::from_bits(value.to_bits()),
    )?;
    Ok(object)
}
pub fn double_value(ctx: &ThreadContext, object: DoubleFloat) -> Result<f64, ObjectError> {
    Ok(f64::from_bits(
        get(
            ctx,
            object,
            widetag::DOUBLE_FLOAT,
            number_offset::DOUBLE_BITS,
        )?
        .bits(),
    ))
}
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
pub fn stream_state(ctx: &ThreadContext, object: Stream) -> Result<Word, ObjectError> {
    get(ctx, object, widetag::STREAM, stream_offset::STATE)
}
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
pub fn ratio_numerator(ctx: &ThreadContext, object: Ratio) -> Result<Word, ObjectError> {
    get(ctx, object, widetag::RATIO, number_offset::RATIO_NUMERATOR)
}
pub fn ratio_denominator(ctx: &ThreadContext, object: Ratio) -> Result<Word, ObjectError> {
    get(
        ctx,
        object,
        widetag::RATIO,
        number_offset::RATIO_DENOMINATOR,
    )
}
pub fn complex_real(ctx: &ThreadContext, object: Complex) -> Result<Word, ObjectError> {
    get(ctx, object, widetag::COMPLEX, number_offset::COMPLEX_REAL)
}
pub fn complex_imag(ctx: &ThreadContext, object: Complex) -> Result<Word, ObjectError> {
    get(ctx, object, widetag::COMPLEX, number_offset::COMPLEX_IMAG)
}
pub fn bignum_sign(ctx: &ThreadContext, object: Bignum) -> Result<bool, ObjectError> {
    Ok(get(ctx, object, widetag::BIGNUM, number_offset::SIGN)?.bits() != 0)
}
pub fn stream_slot(ctx: &ThreadContext, object: Stream, slot: usize) -> Result<Word, ObjectError> {
    get(ctx, object, widetag::STREAM, slot)
}
pub fn readtable_slot(
    ctx: &ThreadContext,
    object: Readtable,
    slot: usize,
) -> Result<Word, ObjectError> {
    get(ctx, object, widetag::READTABLE, slot)
}
pub fn code_slot(
    ctx: &ThreadContext,
    object: CodeObject,
    slot: usize,
) -> Result<Word, ObjectError> {
    get(ctx, object, widetag::CODE, slot)
}
pub fn function_code(ctx: &ThreadContext, object: Function) -> Result<CodeObject, ObjectError> {
    get_any_function(ctx, object, function_offset::CODE)
}
pub fn function_lambda_list(ctx: &ThreadContext, object: Function) -> Result<Word, ObjectError> {
    get_any_function(ctx, object, function_offset::LAMBDA_LIST)
}
