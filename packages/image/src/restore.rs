//! Reconstruction of a saved object graph into a destination runtime.

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    Package, Runtime, ThreadContext, Word, allocate, function_offset, make_cons, make_string,
    make_symbol, pop_root, push_root, widetag,
};
use ncl_sys::CodePtr;

use crate::error::ImageError;
use crate::format::ImageFile;
use crate::record::Record;

#[path = "restore_fill.rs"]
mod restore_fill;

/// Objects restored from an image.
#[derive(Debug)]
pub struct LoadedImage {
    /// Restored root values in the same order they were saved.
    pub roots: Vec<Word>,
    /// Republished code blocks in the same order they were saved.
    pub code: Vec<CodePtr>,
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
pub fn load(
    bytes: &[u8],
    runtime: &Runtime,
    ctx: &mut ThreadContext,
) -> Result<LoadedImage, ImageError> {
    let file = ImageFile::from_bytes(bytes)?;
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
    Ok(LoadedImage { roots, code })
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
    restore_fill::fill_objects(runtime, ctx, &file.objects, slots)?;
    file.roots
        .iter()
        .map(|root| restore_fill::resolve(slots, *root))
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
            let result = Package::from_word(slots[index]).add_nickname(ctx, runtime, word);
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
            let (symbol, _status) = Package::from_word(package_word).intern(ctx, runtime, name)?;
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
                closure,
                builtin,
                name,
                captures,
                ..
            } => {
                if *closure {
                    allocate(
                        ctx,
                        runtime,
                        widetag::CLOSURE,
                        function_offset::CAPTURES + captures.len(),
                    )?
                } else if let Some(builtin) = builtin {
                    runtime
                        .registered_builtin_for_image(
                            ctx,
                            restore_fill::resolve(slots, *name)?,
                            &builtin.package,
                            &builtin.name,
                        )
                        .ok_or(ImageError::UnregisteredBuiltin)?
                        .as_word()
                } else {
                    let function = allocate(
                        ctx,
                        runtime,
                        widetag::SIMPLE_FUN,
                        function_offset::SIMPLE_FUN_WORDS,
                    )?;
                    ctx.write_object_slot(function, function_offset::BUILTIN_INDEX, Word::NIL)?;
                    function
                }
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

fn check_architecture(architecture: ncl_objfile::Architecture) -> Result<(), ImageError> {
    let host = if cfg!(target_arch = "x86_64") {
        ncl_objfile::Architecture::X86_64
    } else {
        ncl_objfile::Architecture::Aarch64
    };
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
