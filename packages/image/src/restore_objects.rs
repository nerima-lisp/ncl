//! Ordered passes that reconstruct an image object graph.

use ncl_object::hash_table::HashTable;
use ncl_object::{
    code_offset, function_offset, instance_offset, number_offset, simple_vector_offset,
    specialized_array_offset, string_offset, structure_offset, symbol_offset, widetag, Package,
    Runtime, ThreadContext, Word, allocate, make_cons, make_simple_vector, make_string,
    make_symbol, rplaca, rplacd, simple_vector_set,
};

use crate::error::ImageError;
use crate::format::ImageFile;
use crate::record::Record;
use super::restore_refs::{decode_test, decode_weakness, fix, fix_u64, put, put_ref, resolve};

pub(crate) fn rebuild(runtime: &Runtime, ctx: &mut ThreadContext, file: &ImageFile, slots: &mut [Word]) -> Result<Vec<Word>, ImageError> {
    create_packages(runtime, ctx, &file.objects, slots)?;
    intern_symbols(runtime, ctx, &file.objects, slots)?;
    allocate_objects(runtime, ctx, &file.objects, slots)?;
    fill_objects(runtime, ctx, &file.objects, slots)?;
    file.roots.iter().map(|root| resolve(slots, *root)).collect()
}

fn create_packages(runtime: &Runtime, ctx: &mut ThreadContext, objects: &[Record], slots: &mut [Word]) -> Result<(), ImageError> {
    for (id, record) in objects.iter().enumerate() {
        let Record::Package { name, nicknames } = record else { continue };
        let package = runtime.ensure_package(ctx, name)?;
        slots[id] = package;
        for nickname in nicknames {
            let mut word = make_string(ctx, runtime, &nickname.chars().collect::<Vec<_>>())?;
            let token = ncl_object::push_root(ctx, &mut word);
            let result = Package::from(package).add_nickname(ctx, runtime, word);
            let _ = ncl_object::pop_root(ctx, token);
            result?;
        }
    }
    Ok(())
}

fn intern_symbols(runtime: &Runtime, ctx: &mut ThreadContext, objects: &[Record], slots: &mut [Word]) -> Result<(), ImageError> {
    for (id, record) in objects.iter().enumerate() {
        let Record::Symbol { package, name, .. } = record else { continue };
        slots[id] = if package.is_empty() {
            let name_word = make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
            make_symbol(ctx, runtime, name_word)?
        } else {
            let package_word = runtime.ensure_package(ctx, package)?;
            let (symbol, _) = Package::from(package_word).intern(ctx, runtime, name)?;
            symbol
        };
    }
    Ok(())
}

fn allocate_objects(runtime: &Runtime, ctx: &mut ThreadContext, objects: &[Record], slots: &mut [Word]) -> Result<(), ImageError> {
    for (id, record) in objects.iter().enumerate() {
        let word = match record {
            Record::Symbol { .. } | Record::Package { .. } => continue,
            Record::Cons { .. } => make_cons(ctx, runtime, Word::NIL, Word::NIL)?,
            Record::String(value) => allocate(ctx, runtime, widetag::STRING, value.chars().count() + 1)?,
            Record::Vector(elements) => allocate(ctx, runtime, widetag::SIMPLE_VECTOR, elements.len() + 1)?,
            Record::SpecializedArray { elements, .. } => allocate(ctx, runtime, widetag::SPECIALIZED_ARRAY, elements.len() + 2)?,
            Record::HashTable { test, weakness, .. } => HashTable::new(ctx, runtime, decode_test(*test)?, decode_weakness(*weakness)?)?.as_word(),
            Record::Structure { slots: fields } => allocate(ctx, runtime, widetag::STRUCTURE, fields.len() + 1)?,
            Record::Instance { .. } => allocate(ctx, runtime, widetag::INSTANCE, 3)?,
            Record::Function { closure, captures, .. } => {
                let payload = if *closure { function_offset::CAPTURES + captures.len() } else { function_offset::CAPTURES };
                let tag = if *closure { widetag::CLOSURE } else { widetag::SIMPLE_FUN };
                allocate(ctx, runtime, tag, payload)?
            }
            Record::CodeObject { .. } => allocate(ctx, runtime, widetag::CODE, 5)?,
            Record::Bignum { limbs, .. } => allocate(ctx, runtime, widetag::BIGNUM, 2 + limbs.len().div_ceil(2))?,
            Record::Ratio { .. } => allocate(ctx, runtime, widetag::RATIO, 2)?,
            Record::DoubleFloat { .. } => allocate(ctx, runtime, widetag::DOUBLE_FLOAT, 1)?,
            Record::Complex { .. } => allocate(ctx, runtime, widetag::COMPLEX, 2)?,
        };
        slots[id] = word;
    }
    Ok(())
}

#[allow(clippy::too_many_lines, reason = "flat per-kind restoration dispatch")]
fn fill_objects(runtime: &Runtime, ctx: &mut ThreadContext, objects: &[Record], slots: &[Word]) -> Result<(), ImageError> {
    for (id, record) in objects.iter().enumerate() {
        let word = slots[id];
        match record {
            Record::Symbol { flags, value, function, plist, .. } => {
                put_ref(ctx, word, symbol_offset::VALUE, resolve(slots, *value)?)?;
                put_ref(ctx, word, symbol_offset::FUNCTION, resolve(slots, *function)?)?;
                put_ref(ctx, word, symbol_offset::PLIST, resolve(slots, *plist)?)?;
                put(ctx, word, symbol_offset::FLAGS, Word::fixnum(i64::from(*flags)))?;
            }
            Record::Cons { car, cdr } => { rplaca(ctx, word, resolve(slots, *car)?)?; rplacd(ctx, word, resolve(slots, *cdr)?)?; }
            Record::String(value) => {
                put(ctx, word, string_offset::LENGTH, fix(value.chars().count())?)?;
                for (offset, character) in value.chars().enumerate() { put(ctx, word, string_offset::DATA + offset, Word::character(u32::from(character)))?; }
            }
            Record::Vector(elements) => {
                put(ctx, word, simple_vector_offset::LENGTH, fix(elements.len())?)?;
                for (offset, element) in elements.iter().enumerate() { put_ref(ctx, word, simple_vector_offset::DATA + offset, resolve(slots, *element)?)?; }
            }
            Record::SpecializedArray { element_type, elements } => {
                put(ctx, word, specialized_array_offset::ELEMENT_TYPE, Word::fixnum(i64::from(*element_type)))?;
                put(ctx, word, specialized_array_offset::LENGTH, fix(elements.len())?)?;
                for (offset, element) in elements.iter().enumerate() { put(ctx, word, specialized_array_offset::DATA + offset, Word::from_bits(*element))?; }
            }
            Record::HashTable { entries, .. } => {
                let table = HashTable::from(word);
                for (key, value) in entries { table.insert(ctx, runtime, resolve(slots, *key)?, resolve(slots, *value)?)?; }
            }
            Record::Structure { slots: fields } => {
                let layout = runtime.register_structure_layout(fields.len())?;
                put(ctx, word, structure_offset::LAYOUT, Word::fixnum(i64::from(layout.as_u32())))?;
                for (offset, field) in fields.iter().enumerate() { put_ref(ctx, word, structure_offset::SLOTS + offset, resolve(slots, *field)?)?; }
            }
            Record::Instance { class, slots: fields } => {
                let class = resolve(slots, *class)?;
                let vector = make_simple_vector(ctx, runtime, &vec![Word::NIL; fields.len()])?;
                put_ref(ctx, word, instance_offset::CLASS, class)?;
                put_ref(ctx, word, instance_offset::SLOT_VECTOR, vector)?;
                put(ctx, word, instance_offset::GENERATION, Word::fixnum(0))?;
                for (offset, field) in fields.iter().enumerate() { simple_vector_set(ctx, vector, offset, resolve(slots, *field)?)?; }
            }
            Record::Function { entry, name, lambda_list, code, captures, .. } => {
                put(ctx, word, function_offset::ENTRY, fix_u64(*entry)?)?;
                put_ref(ctx, word, function_offset::NAME, resolve(slots, *name)?)?;
                put_ref(ctx, word, function_offset::LAMBDA_LIST, resolve(slots, *lambda_list)?)?;
                put_ref(ctx, word, function_offset::CODE, resolve(slots, *code)?)?;
                for (offset, capture) in captures.iter().enumerate() { put_ref(ctx, word, function_offset::CAPTURES + offset, resolve(slots, *capture)?)?; }
            }
            Record::CodeObject { entry, size, constants, stack_map, debug } => {
                put(ctx, word, code_offset::ENTRY, fix_u64(*entry)?)?;
                put(ctx, word, code_offset::SIZE, fix_u64(*size)?)?;
                put_ref(ctx, word, code_offset::CONSTANTS, resolve(slots, *constants)?)?;
                put_ref(ctx, word, code_offset::STACK_MAP, resolve(slots, *stack_map)?)?;
                put_ref(ctx, word, code_offset::DEBUG, resolve(slots, *debug)?)?;
            }
            Record::Bignum { negative, limbs } => {
                put(ctx, word, number_offset::SIGN, Word::from_bits(u64::from(*negative)))?;
                put(ctx, word, number_offset::LIMB_COUNT, fix(limbs.len())?)?;
                for (offset, pair) in limbs.chunks(2).enumerate() { let packed = u64::from(pair[0]) | (pair.get(1).map_or(0, |limb| u64::from(*limb)) << 32); put(ctx, word, number_offset::LIMBS + offset, Word::from_bits(packed))?; }
            }
            Record::Ratio { numerator, denominator } => {
                put_ref(ctx, word, number_offset::RATIO_NUMERATOR, resolve(slots, *numerator)?)?;
                put_ref(ctx, word, number_offset::RATIO_DENOMINATOR, resolve(slots, *denominator)?)?;
            }
            Record::DoubleFloat { bits } => put(ctx, word, number_offset::DOUBLE_BITS, Word::from_bits(*bits))?,
            Record::Complex { real, imag } => {
                put_ref(ctx, word, number_offset::COMPLEX_REAL, resolve(slots, *real)?)?;
                put_ref(ctx, word, number_offset::COMPLEX_IMAG, resolve(slots, *imag)?)?;
            }
            Record::Package { .. } => {}
        }
    }
    Ok(())
}
