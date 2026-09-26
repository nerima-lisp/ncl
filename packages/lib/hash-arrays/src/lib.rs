//! Common Lisp array and hash-table builtins.

#![forbid(unsafe_code)]

mod arrays;
mod hash_tables;

use ncl_object::{
    Arity, Builtin, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, LambdaList, ObjectError, Runtime, ThreadContext, Word, string_length,
    string_ref, symbol_name,
};

pub(crate) fn symbol_text(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    let name = symbol_name(ctx, word)?;
    (0..string_length(ctx, name)?)
        .map(|index| string_ref(ctx, name, index))
        .collect()
}

pub(crate) fn register_one(
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

/// Register the implemented hash-table and array builtins owned by this crate.
///
/// # Errors
///
/// Returns the registration error if a builtin cannot be installed.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    hash_tables::register(runtime, &mut ctx)?;
    arrays::register(runtime, &mut ctx)?;
    Ok(())
}
