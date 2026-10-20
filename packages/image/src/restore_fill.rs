//! Fill restored object payloads and resolve serialized references.

use ncl_object::hash_table::HashTable;
use ncl_object::{
    FunctionObject, ObjectError, Runtime, ThreadContext, Word, code_offset, function_offset,
    instance_offset, make_simple_vector, number_offset, rplaca, rplacd, simple_vector_offset,
    simple_vector_set, specialized_array_offset, string_offset, structure_offset, symbol_offset,
};
use ncl_sys::StorageCondition;

use crate::error::ImageError;
use crate::record::{Record, Ref};

use super::invalid;

/// Pass 4: write every payload slot and hash-table entry.
#[allow(clippy::too_many_lines, reason = "flat per-kind fill dispatch")]
pub(super) fn fill_objects(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    objects: &[Record],
    slots: &[Word],
) -> Result<(), ImageError> {
    for (id, record) in objects.iter().enumerate() {
        let word = slots
            .get(id)
            .copied()
            .ok_or_else(|| invalid("object reference"))?;
        match record {
            Record::Symbol {
                flags,
                value,
                function,
                plist,
                ..
            } => {
                let value = resolve(slots, *value)?;
                let function = resolve(slots, *function)?;
                let plist = resolve(slots, *plist)?;
                put_ref(ctx, word, symbol_offset::VALUE, value)?;
                put_ref(ctx, word, symbol_offset::FUNCTION, function)?;
                put_ref(ctx, word, symbol_offset::PLIST, plist)?;
                put(
                    ctx,
                    word,
                    symbol_offset::FLAGS,
                    Word::fixnum(i64::from(*flags)),
                )?;
            }
            Record::Cons { car, cdr } => {
                let car = resolve(slots, *car)?;
                let cdr = resolve(slots, *cdr)?;
                rplaca(ctx, word, car)?;
                rplacd(ctx, word, cdr)?;
            }
            Record::String(value) => {
                put(
                    ctx,
                    word,
                    string_offset::LENGTH,
                    fix(value.chars().count())?,
                )?;
                for (offset, character) in value.chars().enumerate() {
                    put(
                        ctx,
                        word,
                        string_offset::DATA + offset,
                        Word::character(u32::from(character)),
                    )?;
                }
            }
            Record::Vector(elements) => {
                put(
                    ctx,
                    word,
                    simple_vector_offset::LENGTH,
                    fix(elements.len())?,
                )?;
                for (offset, element) in elements.iter().enumerate() {
                    let value = resolve(slots, *element)?;
                    put_ref(ctx, word, simple_vector_offset::DATA + offset, value)?;
                }
            }
            Record::SpecializedArray {
                element_type,
                elements,
            } => {
                put(
                    ctx,
                    word,
                    specialized_array_offset::ELEMENT_TYPE,
                    Word::fixnum(i64::from(*element_type)),
                )?;
                put(
                    ctx,
                    word,
                    specialized_array_offset::LENGTH,
                    fix(elements.len())?,
                )?;
                for (offset, element) in elements.iter().enumerate() {
                    put(
                        ctx,
                        word,
                        specialized_array_offset::DATA + offset,
                        Word::from_bits(*element),
                    )?;
                }
            }
            Record::HashTable { entries, .. } => {
                let table = HashTable::from_word(word);
                for (key, value) in entries {
                    let key = resolve(slots, *key)?;
                    let value = resolve(slots, *value)?;
                    table.insert(ctx, runtime, key, value)?;
                }
            }
            Record::Structure { slots: fields } => {
                let layout = runtime.register_structure_layout(fields.len())?;
                put(
                    ctx,
                    word,
                    structure_offset::LAYOUT,
                    Word::fixnum(i64::from(layout.as_u32())),
                )?;
                for (offset, field) in fields.iter().enumerate() {
                    let value = resolve(slots, *field)?;
                    put_ref(ctx, word, structure_offset::SLOTS + offset, value)?;
                }
            }
            Record::Instance {
                class,
                slots: fields,
            } => {
                let class = resolve(slots, *class)?;
                let vector = make_simple_vector(ctx, runtime, &vec![Word::NIL; fields.len()])?;
                put_ref(ctx, word, instance_offset::CLASS, class)?;
                put_ref(ctx, word, instance_offset::SLOT_VECTOR, vector)?;
                put(ctx, word, instance_offset::GENERATION, Word::fixnum(0))?;
                for (offset, field) in fields.iter().enumerate() {
                    let value = resolve(slots, *field)?;
                    simple_vector_set(ctx, vector, offset, value)?;
                }
            }
            Record::Function {
                entry,
                name,
                lambda_list,
                code,
                captures,
                closure,
                builtin,
            } => {
                if !*closure
                    && builtin.is_some()
                    && runtime
                        .builtin_descriptor(ctx, FunctionObject::try_from(word)?)
                        .is_some()
                {
                    continue;
                }
                put(ctx, word, function_offset::ENTRY, fix_u64(*entry)?)?;
                let name = resolve(slots, *name)?;
                let lambda_list = resolve(slots, *lambda_list)?;
                let code = resolve(slots, *code)?;
                put_ref(ctx, word, function_offset::NAME, name)?;
                put_ref(ctx, word, function_offset::LAMBDA_LIST, lambda_list)?;
                put_ref(ctx, word, function_offset::CODE, code)?;
                for (offset, capture) in captures.iter().enumerate() {
                    let value = resolve(slots, *capture)?;
                    put_ref(ctx, word, function_offset::CAPTURES + offset, value)?;
                }
            }
            Record::CodeObject {
                entry,
                size,
                constants,
                stack_map,
                debug,
            } => {
                put(ctx, word, code_offset::ENTRY, fix_u64(*entry)?)?;
                put(ctx, word, code_offset::SIZE, fix_u64(*size)?)?;
                let constants = resolve(slots, *constants)?;
                let stack_map = resolve(slots, *stack_map)?;
                let debug = resolve(slots, *debug)?;
                put_ref(ctx, word, code_offset::CONSTANTS, constants)?;
                put_ref(ctx, word, code_offset::STACK_MAP, stack_map)?;
                put_ref(ctx, word, code_offset::DEBUG, debug)?;
            }
            Record::Bignum { negative, limbs } => {
                put(
                    ctx,
                    word,
                    number_offset::SIGN,
                    Word::from_bits(u64::from(*negative)),
                )?;
                put(ctx, word, number_offset::LIMB_COUNT, fix(limbs.len())?)?;
                for (offset, pair) in limbs.chunks(2).enumerate() {
                    let first = pair
                        .first()
                        .copied()
                        .ok_or_else(|| invalid("bignum limbs"))?;
                    let packed =
                        u64::from(first) | (pair.get(1).copied().map_or(0, u64::from) << 32);
                    put(
                        ctx,
                        word,
                        number_offset::LIMBS + offset,
                        Word::from_bits(packed),
                    )?;
                }
            }
            Record::Ratio {
                numerator,
                denominator,
            } => {
                let numerator = resolve(slots, *numerator)?;
                let denominator = resolve(slots, *denominator)?;
                put_ref(ctx, word, number_offset::RATIO_NUMERATOR, numerator)?;
                put_ref(ctx, word, number_offset::RATIO_DENOMINATOR, denominator)?;
            }
            Record::DoubleFloat { bits } => {
                put(
                    ctx,
                    word,
                    number_offset::DOUBLE_BITS,
                    Word::from_bits(*bits),
                )?;
            }
            Record::Complex { real, imag } => {
                let real = resolve(slots, *real)?;
                let imag = resolve(slots, *imag)?;
                put_ref(ctx, word, number_offset::COMPLEX_REAL, real)?;
                put_ref(ctx, word, number_offset::COMPLEX_IMAG, imag)?;
            }
            Record::Package { .. } => {}
        }
    }
    Ok(())
}

/// Resolve a reference against the restored object table.
pub(super) fn resolve(slots: &[Word], reference: Ref) -> Result<Word, ImageError> {
    match reference {
        Ref::Immediate(bits) => Ok(Word::from_bits(bits)),
        Ref::Object(id) => {
            let index = usize::try_from(id).map_err(|_| invalid("object reference"))?;
            slots
                .get(index)
                .copied()
                .ok_or_else(|| invalid("object reference"))
        }
    }
}

/// Write a reference slot and record the write barrier.
fn put_ref(
    ctx: &mut ThreadContext,
    object: Word,
    slot: usize,
    value: Word,
) -> Result<(), ImageError> {
    put(ctx, object, slot, value)?;
    ctx.thread_access().write_barrier(object, slot);
    Ok(())
}

/// Write a scalar slot.
fn put(ctx: &mut ThreadContext, object: Word, slot: usize, value: Word) -> Result<(), ImageError> {
    if !ctx.thread_access().write_object_word(object, slot, value) {
        return Err(ImageError::Object(ObjectError::Storage(
            StorageCondition::ThreadNotRegistered,
        )));
    }
    Ok(())
}

/// Encode a length as a fixnum.
fn fix(value: usize) -> Result<Word, ImageError> {
    Ok(Word::fixnum(
        i64::try_from(value).map_err(|_| invalid("fixnum"))?,
    ))
}
/// Encode a raw address as a fixnum.
fn fix_u64(value: u64) -> Result<Word, ImageError> {
    Ok(Word::fixnum(
        i64::try_from(value).map_err(|_| invalid("address"))?,
    ))
}
