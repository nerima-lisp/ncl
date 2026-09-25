//! Reconstruction of a saved object graph into a destination runtime.

#![allow(clippy::too_many_lines, reason = "flat per-kind restoration dispatch")]

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ObjectError, Package, Runtime, ThreadContext, Word, allocate, code_offset, function_offset,
    instance_offset, make_cons, make_simple_vector, make_string, make_symbol, number_offset,
    pop_root, push_root, rplaca, rplacd, simple_vector_offset, simple_vector_set,
    specialized_array_offset, string_offset, structure_offset, symbol_offset, widetag,
};
use ncl_sys::{CodePtr, RootToken, StorageCondition};

use crate::adapter::parse;
use crate::domain::Architecture;
use crate::error::ImageError;
use crate::format::ImageFile;
use crate::record::{Record, Ref};

/// Objects restored from an image.
#[derive(Debug)]
pub struct LoadedImage {
    /// Restored root values in the same order they were saved.
    roots: Box<[Word]>,
    /// Republished code blocks in the same order they were saved.
    pub code: Vec<CodePtr>,
    root_token: RootToken,
}

impl LoadedImage {
    /// Borrow roots while this image owns their precise root registration.
    #[must_use]
    pub fn roots(&self) -> &[Word] {
        &self.roots
    }

    /// Release the precise roots owned by this image.
    ///
    /// The image must be released in reverse order of other root registrations
    /// made on the same thread. Keeping the image alive keeps its roots valid.
    ///
    /// # Errors
    /// Returns an error when another root was registered after this image.
    pub fn release(self, ctx: &mut ThreadContext) -> Result<(), ImageError> {
        if ncl_sys::pop_root(ctx.thread_mut(), self.root_token) {
            Ok(())
        } else {
            Err(ImageError::InvalidField {
                field: "root ownership",
            })
        }
    }
}

/// Restore an image into `runtime`, returning its roots and code blocks.
///
/// Symbols are re-interned into their saved packages, so loading twice yields
/// the same symbols. Code blocks are republished into fresh, non-moving
/// allocations; the returned pointers own those allocations.
///
/// # Errors
///
/// Returns [`ImageError`] when the byte stream is malformed, targets another
/// architecture, references an unknown record, or when heap or code-space
/// allocation fails.
#[must_use]
pub fn load(
    bytes: &[u8],
    runtime: &Runtime,
    ctx: &mut ThreadContext,
) -> Result<LoadedImage, ImageError> {
    let file = parse(bytes)?;
    check_architecture(file.architecture)?;
    let count = file.objects.len();
    let mut slots: Vec<Word> = vec![Word::NIL; count];
    let mut tokens = Vec::with_capacity(count);
    for slot in &mut slots {
        tokens.push(push_root(ctx, slot));
    }
    let outcome = rebuild(runtime, ctx, &file, &mut slots);
    for token in tokens.into_iter().rev() {
        let _ = pop_root(ctx, token);
    }
    let roots = outcome?;
    let mut code = Vec::with_capacity(file.code.len());
    for image in &file.code {
        code.push(image.publish()?);
    }
    let mut roots = roots.into_boxed_slice();
    let root_token = ncl_sys::register_root_set(ctx.thread_mut(), &mut roots);
    Ok(LoadedImage {
        roots,
        code,
        root_token,
    })
}

/// Run the four reconstruction passes and return the resolved roots.
fn rebuild(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    file: &ImageFile,
    slots: &mut [Word],
) -> Result<Vec<Word>, ImageError> {
    create_packages(runtime, ctx, &file.objects, slots)?;
    intern_symbols(runtime, ctx, &file.objects, slots)?;
    allocate_objects(runtime, ctx, &file.objects, slots)?;
    fill_objects(runtime, ctx, &file.objects, slots)?;
    file.roots
        .iter()
        .map(|root| resolve(slots, *root))
        .collect()
}

/// Pass 1: create packages so that symbol interning can find them.
fn create_packages(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    objects: &[Record],
    slots: &mut [Word],
) -> Result<(), ImageError> {
    for (id, record) in objects.iter().enumerate() {
        let Record::Package { name, nicknames } = record else {
            continue;
        };
        let package = runtime.ensure_package(ctx, name)?;
        let index = id;
        slots[index] = package;
        for nickname in nicknames {
            let mut word = make_string(ctx, runtime, &nickname.chars().collect::<Vec<_>>())?;
            let token = push_root(ctx, &mut word);
            let result = Package::from(slots[index]).add_nickname(ctx, runtime, word);
            let _ = pop_root(ctx, token);
            result?;
        }
    }
    Ok(())
}

/// Pass 2: intern symbols into their home packages.
fn intern_symbols(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    objects: &[Record],
    slots: &mut [Word],
) -> Result<(), ImageError> {
    for (id, record) in objects.iter().enumerate() {
        let Record::Symbol { package, name, .. } = record else {
            continue;
        };
        let symbol = if package.is_empty() {
            let name_word = make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
            make_symbol(ctx, runtime, name_word)?
        } else {
            let package_word = runtime.ensure_package(ctx, package)?;
            let (symbol, _status) = Package::from(package_word).intern(ctx, runtime, name)?;
            symbol
        };
        let index = id;
        slots[index] = symbol;
    }
    Ok(())
}

/// Pass 3: allocate a placeholder object for every non-symbol record.
fn allocate_objects(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    objects: &[Record],
    slots: &mut [Word],
) -> Result<(), ImageError> {
    for (id, record) in objects.iter().enumerate() {
        let word = match record {
            Record::Symbol { .. } | Record::Package { .. } => continue,
            Record::Cons { .. } => make_cons(ctx, runtime, Word::NIL, Word::NIL)?,
            Record::String(value) => {
                allocate(ctx, runtime, widetag::STRING, value.chars().count() + 1)?
            }
            Record::Vector(elements) => {
                allocate(ctx, runtime, widetag::SIMPLE_VECTOR, elements.len() + 1)?
            }
            Record::SpecializedArray { elements, .. } => {
                allocate(ctx, runtime, widetag::SPECIALIZED_ARRAY, elements.len() + 2)?
            }
            Record::HashTable { test, weakness, .. } => HashTable::new(
                ctx,
                runtime,
                decode_test(*test)?,
                decode_weakness(*weakness)?,
            )?
            .as_word(),
            Record::Structure { slots: fields } => {
                allocate(ctx, runtime, widetag::STRUCTURE, fields.len() + 1)?
            }
            Record::Instance { .. } => allocate(ctx, runtime, widetag::INSTANCE, 3)?,
            Record::Function {
                closure, captures, ..
            } => {
                let payload = if *closure {
                    function_offset::CAPTURES + captures.len()
                } else {
                    function_offset::CAPTURES
                };
                let tag = if *closure {
                    widetag::CLOSURE
                } else {
                    widetag::SIMPLE_FUN
                };
                allocate(ctx, runtime, tag, payload)?
            }
            Record::CodeObject { .. } => allocate(ctx, runtime, widetag::CODE, 5)?,
            Record::Bignum { limbs, .. } => {
                allocate(ctx, runtime, widetag::BIGNUM, 2 + limbs.len().div_ceil(2))?
            }
            Record::Ratio { .. } => allocate(ctx, runtime, widetag::RATIO, 2)?,
            Record::DoubleFloat { .. } => allocate(ctx, runtime, widetag::DOUBLE_FLOAT, 1)?,
            Record::Complex { .. } => allocate(ctx, runtime, widetag::COMPLEX, 2)?,
        };
        let index = id;
        slots[index] = word;
    }
    Ok(())
}

/// Pass 4: write every payload slot and hash-table entry.
fn fill_objects(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    objects: &[Record],
    slots: &[Word],
) -> Result<(), ImageError> {
    for (id, record) in objects.iter().enumerate() {
        let index = id;
        let word = slots[index];
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
                let table = HashTable::from(word);
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
                ..
            } => {
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
                    let packed =
                        u64::from(pair[0]) | (pair.get(1).map_or(0, |limb| u64::from(*limb)) << 32);
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
fn resolve(slots: &[Word], reference: Ref) -> Result<Word, ImageError> {
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
    ncl_sys::write_barrier(ctx.thread_mut(), object, slot);
    Ok(())
}

/// Write a scalar slot.
fn put(ctx: &mut ThreadContext, object: Word, slot: usize, value: Word) -> Result<(), ImageError> {
    if !ncl_sys::write_object_word(ctx.thread_mut(), object, slot, value) {
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

const fn decode_test(value: u8) -> Result<HashTest, ImageError> {
    match value {
        0 => Ok(HashTest::Eq),
        1 => Ok(HashTest::Eql),
        2 => Ok(HashTest::Equal),
        3 => Ok(HashTest::Equalp),
        _ => Err(invalid("hash test")),
    }
}

const fn decode_weakness(value: u8) -> Result<Weakness, ImageError> {
    match value {
        0 => Ok(Weakness::None),
        1 => Ok(Weakness::Key),
        2 => Ok(Weakness::Value),
        3 => Ok(Weakness::KeyAndValue),
        4 => Ok(Weakness::KeyOrValue),
        _ => Err(invalid("hash weakness")),
    }
}

fn check_architecture(architecture: Architecture) -> Result<(), ImageError> {
    let host = Architecture::host();
    if architecture == host {
        Ok(())
    } else {
        Err(ImageError::InvalidField {
            field: "architecture",
        })
    }
}

const fn invalid(field: &'static str) -> ImageError {
    ImageError::InvalidLayout { field }
}
