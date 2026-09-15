use crate::code::CodeObject;
use crate::object_access::{fix, get_function, put};
use crate::{ObjectError, Runtime, ThreadContext, allocate, function_offset, widetag};
use ncl_sys::Word;

pub type Function = Word;

/// Allocate a simple function object.
///
/// # Errors
/// Returns an error when allocation or entry encoding fails.
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

/// Allocate a closure with inline captured values.
///
/// # Errors
/// Returns an error when allocation or entry encoding fails.
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

/// Read a function entry address.
///
/// # Errors
/// Returns an error when the object or entry is invalid.
pub fn function_entry(ctx: &ThreadContext, object: Function) -> Result<usize, ObjectError> {
    usize::try_from(
        get_function(ctx, object, function_offset::ENTRY)?
            .as_fixnum()
            .ok_or(ObjectError::Layout)?,
    )
    .map_err(|_| ObjectError::Layout)
}
/// Read a function name.
///
/// # Errors
/// Returns an error when the object is not a function.
pub fn function_name(ctx: &ThreadContext, object: Function) -> Result<Word, ObjectError> {
    get_function(ctx, object, function_offset::NAME)
}
/// Read a closure capture.
///
/// # Errors
/// Returns an error when the object or capture is invalid.
pub fn closure_ref(
    ctx: &ThreadContext,
    object: Function,
    index: usize,
) -> Result<Word, ObjectError> {
    crate::object_access::get(
        ctx,
        object,
        widetag::CLOSURE,
        function_offset::CAPTURES + index,
    )
}
/// Read a function's code object.
///
/// # Errors
/// Returns an error when the object is not a function.
pub fn function_code(ctx: &ThreadContext, object: Function) -> Result<CodeObject, ObjectError> {
    get_function(ctx, object, function_offset::CODE)
}
/// Read a function lambda list.
///
/// # Errors
/// Returns an error when the object is not a function.
pub fn function_lambda_list(ctx: &ThreadContext, object: Function) -> Result<Word, ObjectError> {
    get_function(ctx, object, function_offset::LAMBDA_LIST)
}
