//! Common Lisp array and vector builtins.

#![forbid(unsafe_code)]

mod arrays;

use ncl_object::{
    Arity, Builtin, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, LambdaList, ObjectError, Runtime, ThreadContext, Word, string_length,
    string_ref, symbol_name,
};

fn symbol_text(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    if word == Word::NIL {
        return Ok("NIL".to_owned());
    }
    let name = symbol_name(ctx, word)?;
    (0..string_length(ctx, name)?)
        .map(|index| string_ref(ctx, name, index))
        .collect()
}

fn register_one(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &'static str,
    list: LambdaList,
    function: ncl_object::RustBuiltin,
) -> Result<(), ObjectError> {
    let descriptor = Builtin {
        lambda_list: list,
        convention: if list.is_direct() {
            BuiltinConvention::Direct(Arity::exact(
                u8::try_from(list.required.len()).map_err(|_| ObjectError::Layout)?,
            ))
        } else {
            BuiltinConvention::Adapted
        },
    };
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
        BuiltinImplementation::direct(descriptor, function),
    )?;
    Ok(())
}

fn register_one_ncl(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &'static str,
    list: LambdaList,
    function: ncl_object::RustBuiltin,
) -> Result<(), ObjectError> {
    let descriptor = Builtin {
        lambda_list: list,
        convention: if list.is_direct() {
            BuiltinConvention::Direct(Arity::exact(
                u8::try_from(list.required.len()).map_err(|_| ObjectError::Layout)?,
            ))
        } else {
            BuiltinConvention::Adapted
        },
    };
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::NclExt, BuiltinName::new(name)),
        BuiltinImplementation::direct(descriptor, function),
    )?;
    Ok(())
}

/// Register the array layout with the runtime heap.
///
/// # Errors
///
/// Returns [`ObjectError::Layout`] if an array layout widetag is already registered.
pub fn register_layouts(runtime: &Runtime) -> Result<(), ObjectError> {
    ncl_arrays::register_layouts(runtime)
}

/// Register the Common Lisp array and vector builtins.
///
/// # Errors
///
/// Returns the registration error if a builtin cannot be installed.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    arrays::register(runtime, &mut ctx)
}
