//! NCL-GC garbage-collection symbol ownership.

use crate::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, LambdaList, MultipleValues, ObjectError, Package, Parameter,
    Runtime, ThreadContext, Word,
    layout::{
        code_offset, function_offset, instance_offset, number_offset, readtable_offset,
        reference_words, stream_offset, structure_offset, symbol_offset, widetag,
    },
    make_bignum_from_i128, make_string,
};

fn package_layout() -> ncl_sys::ReferenceLayout {
    ncl_sys::ReferenceLayout {
        reference_words: reference_words(&crate::package::reference_words()),
        boxed_from: Some(crate::package::NAME + 1),
    }
}

const NO_PARAMETERS: &[Parameter] = &[];
const SB_EXT_SYMBOLS: &[&str] = &[
    "*AFTER-GC-HOOKS*",
    "*GC-REAL-TIME*",
    "*GC-RUN-TIME*",
    "CANCEL-FINALIZATION",
    "FINALIZE",
    "GC",
    "GENERATION-BYTES-ALLOCATED",
    "HASH-TABLE-WEAKNESS",
    "MAKE-WEAK-POINTER",
    "MAKE-WEAK-VECTOR",
    "WEAK-POINTER",
    "WEAK-POINTER-P",
    "WEAK-POINTER-VALUE",
    "WEAK-VECTOR-P",
];

fn gc_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    _args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    ctx.collect(true)?;
    Ok(Word::NIL)
}

fn bytes_consed_since_gc_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    _args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let bytes = ctx.thread_access().bytes_cons_since_gc();
    counter_word(
        ctx,
        runtime,
        u128::try_from(bytes).map_err(|_| ObjectError::Layout)?,
    )
}

fn gc_count_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    _args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let count = ctx.thread_access().heap_epoch();
    counter_word(ctx, runtime, u128::from(count))
}

fn counter_word(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: u128,
) -> Result<Word, ObjectError> {
    const MAX_FIXNUM: u128 = (1_u128 << 62) - 1;
    if value <= MAX_FIXNUM {
        return Ok(Word::fixnum(
            i64::try_from(value).map_err(|_| ObjectError::Layout)?,
        ));
    }
    let integer = i128::try_from(value).map_err(|_| ObjectError::Layout)?;
    Ok(make_bignum_from_i128(ctx, runtime, integer)?.into())
}

/// Register every header-object reference layout owned by ncl-object.
///
/// # Errors
/// Returns `ObjectError::Layout` when layout registration fails.
pub fn register_layouts(runtime: &Runtime) -> Result<(), ObjectError> {
    for (tag, slots) in [
        (
            widetag::SYMBOL,
            vec![
                symbol_offset::VALUE,
                symbol_offset::FUNCTION,
                symbol_offset::PLIST,
                symbol_offset::PACKAGE,
                symbol_offset::NAME,
            ],
        ),
        (widetag::STRING, vec![]),
        (widetag::STRUCTURE, vec![structure_offset::SLOTS]),
        (
            widetag::INSTANCE,
            vec![instance_offset::CLASS, instance_offset::SLOT_VECTOR],
        ),
        (
            widetag::SIMPLE_FUN,
            vec![
                function_offset::NAME,
                function_offset::LAMBDA_LIST,
                function_offset::CODE,
            ],
        ),
        (
            widetag::CLOSURE,
            vec![
                function_offset::NAME,
                function_offset::LAMBDA_LIST,
                function_offset::CODE,
            ],
        ),
        (widetag::BIGNUM, vec![]),
        (
            widetag::RATIO,
            vec![
                number_offset::RATIO_NUMERATOR,
                number_offset::RATIO_DENOMINATOR,
            ],
        ),
        (widetag::DOUBLE_FLOAT, vec![]),
        (
            widetag::COMPLEX,
            vec![number_offset::COMPLEX_REAL, number_offset::COMPLEX_IMAG],
        ),
        (widetag::PACKAGE, crate::package::reference_words()),
        (
            widetag::READTABLE,
            vec![readtable_offset::SYNTAX, readtable_offset::DISPATCH],
        ),
        (
            widetag::STREAM,
            vec![
                stream_offset::DIRECTION,
                stream_offset::ELEMENT_TYPE,
                stream_offset::EXTERNAL_FORMAT,
                stream_offset::STATE,
                stream_offset::IMPLEMENTATION,
            ],
        ),
        (
            widetag::CODE,
            vec![
                code_offset::CONSTANTS,
                code_offset::STACK_MAP,
                code_offset::DEBUG,
            ],
        ),
        (
            widetag::VALUE_CELL,
            vec![crate::layout::value_cell_offset::VALUE],
        ),
    ] {
        let layout = match tag {
            widetag::PACKAGE => package_layout(),
            _ => ncl_sys::ReferenceLayout {
                reference_words: reference_words(&slots),
                boxed_from: match tag {
                    widetag::STRUCTURE => Some(structure_offset::SLOTS + 1),
                    widetag::CLOSURE => Some(function_offset::CAPTURES + 1),
                    _ => None,
                },
            },
        };
        runtime.register_layout(tag, layout)?;
    }
    Ok(())
}

/// Register the NCL-GC GC, weak-pointer, and finalizer symbols owned here.
///
/// # Errors
/// Returns an allocation, layout, or storage error from function registration.
pub fn register(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    let package = runtime.ensure_package(ctx, "NCL-GC")?;
    let package = Package::from_word(package);
    for name in ["*AFTER-GC-HOOKS*", "*GC-REAL-TIME*", "*GC-RUN-TIME*"] {
        let (symbol, _) = package.intern(ctx, runtime, name)?;
        crate::set_symbol_special(ctx, symbol, true)?;
    }
    for name in [
        "CANCEL-FINALIZATION",
        "FINALIZE",
        "GC",
        "GENERATION-BYTES-ALLOCATED",
        "BYTES-CONSED-SINCE-GC",
        "GC-COUNT",
        "HASH-TABLE-WEAKNESS",
        "MAKE-WEAK-POINTER",
        "MAKE-WEAK-VECTOR",
        "WEAK-POINTER-P",
        "WEAK-POINTER-VALUE",
        "WEAK-VECTOR-P",
    ] {
        package.intern(ctx, runtime, name)?;
        runtime.define_function(ctx, "NCL-GC", name, Word::UNBOUND)?;
    }
    for name in std::iter::once("BYTES-CONSED-SINCE-GC").chain(std::iter::once("GC-COUNT")) {
        let name_word = make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
        if !package.export(ctx, runtime, name_word)? {
            return Err(ObjectError::Layout);
        }
    }
    let (weak_pointer, _) = package.intern(ctx, runtime, "WEAK-POINTER")?;
    runtime.define_class(ctx, "WEAK-POINTER", Word::UNBOUND)?;
    let _ = weak_pointer;
    let descriptor = Builtin {
        lambda_list: LambdaList::fixed(NO_PARAMETERS),
        convention: BuiltinConvention::Direct(Arity::exact(0)),
    };
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::NclGc, BuiltinName::new("GC")),
        BuiltinImplementation::direct(descriptor, gc_builtin),
    )?;
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(
            BuiltinPackage::NclGc,
            BuiltinName::new("BYTES-CONSED-SINCE-GC"),
        ),
        BuiltinImplementation::direct(descriptor, bytes_consed_since_gc_builtin),
    )?;
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::NclGc, BuiltinName::new("GC-COUNT")),
        BuiltinImplementation::direct(descriptor, gc_count_builtin),
    )?;
    for name in SB_EXT_SYMBOLS {
        runtime.register_sb_symbol(
            ctx,
            BuiltinPackage::NclGc,
            BuiltinPackage::SbExt,
            BuiltinName::new(name),
            true,
        )?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/support/gc.rs"]
mod tests;
