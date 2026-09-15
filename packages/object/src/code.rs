use crate::object_access::{fix, get, put};
use crate::{ObjectError, Runtime, ThreadContext, allocate, widetag};
use ncl_sys::Word;

crate::word_newtype!(CodeObject);

/// Allocate a code object descriptor.
///
/// # Errors
/// Returns an error when an integer field cannot be encoded or allocation fails.
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
    Ok(object.into())
}
/// Read a raw code object slot.
///
/// # Errors
/// Returns an error when the object or slot is invalid.
pub fn code_slot(
    ctx: &ThreadContext,
    object: CodeObject,
    slot: usize,
) -> Result<Word, ObjectError> {
    get(ctx, object.into(), widetag::CODE, slot)
}

macro_rules! code_accessor {
    ($name:ident, $slot:expr, $doc:literal) => {
        #[doc = $doc]
        ///
        /// # Errors
        /// Returns an error when the object is not a code object.
        pub fn $name(ctx: &ThreadContext, object: CodeObject) -> Result<Word, ObjectError> {
            code_slot(ctx, object, $slot)
        }
    };
}
code_accessor!(
    code_entry,
    crate::code_offset::ENTRY,
    "Read a code entry address."
);
code_accessor!(
    code_size,
    crate::code_offset::SIZE,
    "Read a code object size."
);
code_accessor!(
    code_constants,
    crate::code_offset::CONSTANTS,
    "Read a code constant table."
);
code_accessor!(
    code_stack_map,
    crate::code_offset::STACK_MAP,
    "Read a code stack-map index."
);
code_accessor!(
    code_debug,
    crate::code_offset::DEBUG,
    "Read a code debug table."
);
