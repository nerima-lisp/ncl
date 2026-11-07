//! NCL-GC garbage-collection symbol ownership.

use crate::hash_table::{INDEX, KV, MARKER, REHASH_SIZE, REHASH_THRESHOLD};
use crate::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, LambdaList, MultipleValues, ObjectError, Package, Parameter,
    Runtime, ThreadContext, Word,
    layout::{
        code_offset, function_offset, instance_offset, number_offset, readtable_offset,
        reference_words, simple_vector_offset, specialized_array_offset, stream_offset,
        structure_offset, symbol_offset, widetag,
    },
    make_bignum_from_i128, make_string,
};

fn hash_table_layout() -> ncl_sys::ReferenceLayout {
    ncl_sys::ReferenceLayout {
        reference_words: reference_words(&[REHASH_SIZE, REHASH_THRESHOLD, MARKER, KV, INDEX]),
        boxed_from: Some(MARKER + 1),
    }
}

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
        (widetag::SIMPLE_VECTOR, vec![simple_vector_offset::DATA]),
        (
            widetag::HASH_TABLE,
            vec![REHASH_SIZE, REHASH_THRESHOLD, MARKER, KV, INDEX],
        ),
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
        (widetag::SPECIALIZED_ARRAY, vec![]),
        (widetag::NON_SIMPLE_ARRAY, vec![]),
        (
            widetag::VALUE_CELL,
            vec![crate::layout::value_cell_offset::VALUE],
        ),
    ] {
        let layout = match tag {
            widetag::HASH_TABLE => hash_table_layout(),
            widetag::PACKAGE => package_layout(),
            _ => ncl_sys::ReferenceLayout {
                reference_words: reference_words(&slots),
                boxed_from: match tag {
                    widetag::SIMPLE_VECTOR => Some(simple_vector_offset::DATA + 1),
                    widetag::SPECIALIZED_ARRAY => Some(specialized_array_offset::DATA + 1),
                    widetag::NON_SIMPLE_ARRAY => Some(1),
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
mod tests {
    use super::*;

    fn function_word(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
        runtime
            .function(ctx, "NCL-GC", name)
            .filter(|function| crate::FunctionObject::try_from(*function).is_ok())
            .unwrap_or_else(|| panic!("NCL-GC:{name} builtin was not registered"))
    }

    fn call_builtin(ctx: &mut ThreadContext, runtime: &Runtime, function: Word) -> Word {
        runtime
            .call_builtin(
                ctx,
                crate::FunctionObject::try_from(function)
                    .unwrap_or_else(|error| panic!("function word: {error:?}")),
                &[],
            )
            .unwrap_or_else(|error| panic!("NCL-GC counter call failed: {error:?}"))
    }

    fn call_counter(ctx: &mut ThreadContext, runtime: &Runtime, function: Word) -> i64 {
        call_builtin(ctx, runtime, function)
            .as_fixnum()
            .unwrap_or_else(|| panic!("NCL-GC counter did not return a fixnum"))
    }

    #[test]
    fn allocation_debt_and_gc_count_are_read_only_counters() {
        let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
        let mut ctx = crate::ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("register: {error:?}"));
        register(&mut ctx, &runtime).unwrap_or_else(|error| panic!("NCL-GC register: {error:?}"));

        let mut functions = [Word::NIL; 3];
        let roots = ctx.thread_access().register_root_set(&mut functions);
        functions[0] = function_word(&mut ctx, &runtime, "GC");
        functions[1] = function_word(&mut ctx, &runtime, "BYTES-CONSED-SINCE-GC");
        functions[2] = function_word(&mut ctx, &runtime, "GC-COUNT");
        let before_collection = call_counter(&mut ctx, &runtime, functions[2]);
        let _ = call_builtin(&mut ctx, &runtime, functions[0]);
        assert_eq!(
            call_counter(&mut ctx, &runtime, functions[2]),
            before_collection + 1
        );
        assert_eq!(call_counter(&mut ctx, &runtime, functions[1]), 0);

        crate::allocate(&mut ctx, &runtime, widetag::BIGNUM, 3)
            .unwrap_or_else(|error| panic!("heap allocation: {error:?}"));
        assert_eq!(call_counter(&mut ctx, &runtime, functions[1]), 32);
        assert_eq!(
            call_counter(&mut ctx, &runtime, functions[2]),
            before_collection + 1
        );

        let _ = call_builtin(&mut ctx, &runtime, functions[0]);
        assert_eq!(
            call_counter(&mut ctx, &runtime, functions[2]),
            before_collection + 2
        );
        assert_eq!(call_counter(&mut ctx, &runtime, functions[1]), 0);
        assert!(ctx.thread_access().pop_root(roots));
    }

    #[test]
    fn hash_table_and_package_layouts_use_header_inclusive_references() {
        assert_eq!(
            hash_table_layout().reference_words,
            [
                REHASH_SIZE + 1,
                REHASH_THRESHOLD + 1,
                MARKER + 1,
                KV + 1,
                INDEX + 1,
            ]
        );
        assert_eq!(
            package_layout().reference_words,
            reference_words(&crate::package::reference_words())
        );
        assert!(
            package_layout()
                .reference_words
                .contains(&(crate::package::DOCUMENTATION + 1))
        );
        assert_eq!(hash_table_layout().boxed_from, Some(MARKER + 1));
        assert_eq!(package_layout().boxed_from, Some(crate::package::NAME + 1));
    }

    #[test]
    fn gc_builtin_runs_a_full_collection() {
        let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
        let mut ctx = crate::ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("register: {error:?}"));
        register(&mut ctx, &runtime).unwrap_or_else(|error| panic!("NCL-GC register: {error:?}"));
        let gc = runtime
            .function(&mut ctx, "NCL-GC", "GC")
            .and_then(|function| crate::FunctionObject::try_from(function).ok())
            .unwrap_or_else(|| panic!("NCL-GC:GC builtin was not registered"));
        let before = ncl_sys::heap_epoch(&ctx.thread);

        runtime
            .call_builtin(&mut ctx, gc, &[])
            .unwrap_or_else(|error| panic!("NCL-GC:GC failed: {error:?}"));

        let after = ncl_sys::heap_epoch(&ctx.thread);
        assert!(
            after > before,
            "NCL-GC:GC did not advance the collection epoch"
        );
    }
}
