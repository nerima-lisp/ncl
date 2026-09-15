//! SB-EXT garbage-collection symbol ownership.

use crate::{
    ObjectError, Runtime, Word,
    layout::{
        code_offset, function_offset, instance_offset, number_offset, readtable_offset,
        reference_words, simple_vector_offset, stream_offset, structure_offset, symbol_offset,
        widetag,
    },
};

/// Register every header-object reference layout owned by ncl-object.
///
/// # Errors
/// Returns `ObjectError::Layout` when a layout is already registered.
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
        (widetag::HASH_TABLE, vec![]),
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
        (widetag::PACKAGE, vec![]),
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
    ] {
        ncl_sys::register_layout(
            runtime.heap(),
            tag,
            ncl_sys::ReferenceLayout {
                reference_words: reference_words(&slots),
                boxed_from: match tag {
                    widetag::SIMPLE_VECTOR => Some(simple_vector_offset::DATA + 1),
                    widetag::HASH_TABLE | widetag::NON_SIMPLE_ARRAY => Some(1),
                    widetag::STRUCTURE => Some(structure_offset::SLOTS + 1),
                    widetag::CLOSURE => Some(function_offset::CAPTURES + 1),
                    _ => None,
                },
            },
        )
        .map_err(|_| ObjectError::Layout)?;
    }
    Ok(())
}

/// Register the 14 SB-EXT GC, weak-pointer, and finalizer symbols owned here.
pub fn register(runtime: &Runtime) {
    for name in [
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
    ] {
        runtime.define_function("SB-EXT", name, Word::UNBOUND);
    }
}
