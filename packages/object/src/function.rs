use crate::code::CodeObject;
use crate::object_access::{fix, get_function, put};
use crate::{
    ObjectError, Runtime, ThreadContext, allocate, function_offset, widetag, with_root, with_roots,
};
use ncl_sys::Word;

crate::word_newtype!(Function);

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
    let mut name = name;
    let mut lambda_list = lambda_list;
    let mut code = code.into();
    with_root(ctx, &mut name, |ctx, name| {
        with_root(ctx, &mut lambda_list, |ctx, lambda_list| {
            with_root(ctx, &mut code, |ctx, code| {
                let object = allocate(
                    ctx,
                    runtime,
                    widetag::SIMPLE_FUN,
                    function_offset::SIMPLE_FUN_WORDS,
                )?;
                put(ctx, object, function_offset::NAME, *name)?;
                put(ctx, object, function_offset::LAMBDA_LIST, *lambda_list)?;
                put(ctx, object, function_offset::CODE, *code)?;
                put(ctx, object, function_offset::BUILTIN_INDEX, Word::NIL)?;
                set_function_entry(ctx, Function::from_word(object), entry)?;
                Ok(Function::from_word(object))
            })
        })
    })
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
    let mut name = name;
    let mut lambda_list = lambda_list;
    let mut code = code.into();
    with_root(ctx, &mut name, |ctx, name| {
        with_root(ctx, &mut lambda_list, |ctx, lambda_list| {
            with_root(ctx, &mut code, |ctx, code| {
                with_roots(ctx, values, |ctx, values| {
                    let object = allocate(
                        ctx,
                        runtime,
                        widetag::CLOSURE,
                        function_offset::CAPTURES + values.len(),
                    )?;
                    for (slot, value) in [
                        (function_offset::NAME, *name),
                        (function_offset::LAMBDA_LIST, *lambda_list),
                        (function_offset::CODE, *code),
                    ] {
                        put(ctx, object, slot, value)?;
                    }
                    for (index, value) in values.iter().copied().enumerate() {
                        put(ctx, object, function_offset::CAPTURES + index, *value)?;
                    }
                    set_function_entry(ctx, Function::from_word(object), entry)?;
                    Ok(Function::from_word(object))
                })
            })
        })
    })
}

/// Allocate a GC-managed value cell containing `value`.
///
/// # Errors
/// Returns an error when allocation or storing the initial value fails.
pub fn make_value_cell(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Word,
) -> Result<Word, ObjectError> {
    let mut value = value;
    with_root(ctx, &mut value, |ctx, value| {
        let object = allocate(ctx, runtime, widetag::VALUE_CELL, 1)?;
        put(ctx, object, crate::value_cell_offset::VALUE, *value)?;
        Ok(object)
    })
}

/// Read a function entry address.
///
/// # Errors
/// Returns an error when the object or entry is invalid.
pub fn function_entry(ctx: &ThreadContext, object: Function) -> Result<usize, ObjectError> {
    if !matches!(
        ncl_sys::object_widetag(&ctx.thread, object.into()),
        Some(widetag::SIMPLE_FUN | widetag::CLOSURE)
    ) {
        return Err(ObjectError::TypeError);
    }
    usize::try_from(
        ncl_sys::load_object_word_acquire(
            &ctx.thread,
            object.into(),
            ncl_sys::AtomicObjectWordCell::FunctionEntry,
        )
        .ok_or(ObjectError::Layout)?
        .as_fixnum()
        .ok_or(ObjectError::Layout)?,
    )
    .map_err(|_| ObjectError::Layout)
}

/// Release-publish a function object's native entry cell.
///
/// # Errors
/// Returns `Layout` when the address cannot be encoded or the function object is invalid.
pub fn set_function_entry(
    ctx: &ThreadContext,
    object: Function,
    entry: usize,
) -> Result<(), ObjectError> {
    if ncl_sys::store_object_word_release(
        &ctx.thread,
        object.into(),
        ncl_sys::AtomicObjectWordCell::FunctionEntry,
        fix(entry)?,
    ) {
        Ok(())
    } else {
        Err(ObjectError::Layout)
    }
}
/// Read a function name.
///
/// # Errors
/// Returns an error when the object is not a function.
pub fn function_name(ctx: &ThreadContext, object: Function) -> Result<Word, ObjectError> {
    get_function(ctx, object.into(), function_offset::NAME)
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
        object.into(),
        widetag::CLOSURE,
        function_offset::CAPTURES + index,
    )
}
/// Read a function's code object.
///
/// # Errors
/// Returns an error when the object is not a function.
pub fn function_code(ctx: &ThreadContext, object: Function) -> Result<CodeObject, ObjectError> {
    get_function(ctx, object.into(), function_offset::CODE).map(CodeObject::from_word)
}
/// Read a function lambda list.
///
/// # Errors
/// Returns an error when the object is not a function.
pub fn function_lambda_list(ctx: &ThreadContext, object: Function) -> Result<Word, ObjectError> {
    get_function(ctx, object.into(), function_offset::LAMBDA_LIST)
}
