use ncl_object::{
    FunctionObject, ObjectError, Package, Runtime as ObjectRuntime, ThreadContext, Word,
    set_symbol_value, symbol_value,
};

pub fn pathname_from_designator(
    ctx: &mut ThreadContext,
    object: &ObjectRuntime,
    value: Word,
) -> Result<Word, ObjectError> {
    let function = object
        .function(ctx, "COMMON-LISP", "PATHNAME")
        .ok_or(ObjectError::UndefinedFunction)?;
    object.call_builtin(ctx, FunctionObject::try_from(function)?, &[value])
}

pub fn bind_pathname_variable(
    ctx: &mut ThreadContext,
    object: &ObjectRuntime,
    name: &str,
    value: Word,
) -> Result<(Word, Word), ObjectError> {
    let package = object
        .find_package(ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    let variable = Package::from_word(package).intern(ctx, object, name)?.0;
    let previous = symbol_value(ctx, variable)?;
    set_symbol_value(ctx, variable, value)?;
    Ok((variable, previous))
}
