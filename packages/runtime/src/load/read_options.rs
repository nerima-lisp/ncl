use crate::{Runtime, RuntimeError};
use ncl_compiler_front::form::word_string;
use ncl_object::{
    ObjectError, ObjectRef, Package, Readtable as ObjectReadtable, Runtime as ObjectRuntime,
    ThreadContext, Word, classify_object, symbol_name, symbol_value, with_root,
};
use ncl_reader::{FloatFormat, ReadBase, ReadEvaluation, ReadOptions, ReadSuppression, Readtable};

pub fn current_read_options(
    runtime: &mut Runtime,
    previous: Option<ReadOptions>,
) -> Result<ReadOptions, RuntimeError> {
    let mut options = match previous {
        Some(options) => options,
        None => ReadOptions::standard(&mut runtime.context, &runtime.object)?,
    };
    apply_current_read_flags(runtime, &mut options)?;
    let package_word =
        common_lisp_variable_value(&mut runtime.context, &runtime.object, "*PACKAGE*")?;
    let ObjectRef::Package(package) = classify_object(&runtime.context, package_word) else {
        return Err(RuntimeError::Object(ObjectError::TypeError));
    };
    let package_name = word_string(
        &runtime.context,
        Package::from_word(package).name(&runtime.context)?,
    )?;
    let readtable_word =
        common_lisp_variable_value(&mut runtime.context, &runtime.object, "*READTABLE*")?;
    if !matches!(
        classify_object(&runtime.context, readtable_word),
        ObjectRef::Readtable(_)
    ) {
        return Err(RuntimeError::Object(ObjectError::TypeError));
    }
    options.set_current_package(package_name)?;
    options.set_readtable(Readtable::from_object(ObjectReadtable::from_word(
        readtable_word,
    )));
    Ok(options)
}

pub(super) fn apply_current_read_flags(
    runtime: &mut Runtime,
    options: &mut ReadOptions,
) -> Result<(), RuntimeError> {
    options.set_read_evaluation(ReadEvaluation::Dynamic);
    let read_base =
        common_lisp_variable_value(&mut runtime.context, &runtime.object, "*READ-BASE*")?;
    let ObjectRef::Fixnum(read_base) = classify_object(&runtime.context, read_base) else {
        return Err(RuntimeError::Object(ObjectError::TypeError));
    };
    let Ok(read_base) = u32::try_from(read_base) else {
        return Err(RuntimeError::Object(ObjectError::TypeError));
    };
    let Ok(read_base) = ReadBase::new(read_base) else {
        return Err(RuntimeError::Object(ObjectError::TypeError));
    };
    options.set_read_base(read_base);

    let default_float_format = common_lisp_variable_value(
        &mut runtime.context,
        &runtime.object,
        "*READ-DEFAULT-FLOAT-FORMAT*",
    )?;
    if !matches!(
        classify_object(&runtime.context, default_float_format),
        ObjectRef::Symbol(_)
    ) {
        return Err(RuntimeError::Object(ObjectError::TypeError));
    }
    let default_float_format = word_string(
        &runtime.context,
        symbol_name(&runtime.context, default_float_format)?,
    )?;
    let format_name = default_float_format.as_str();
    let float_format = if format_name == "SHORT-FLOAT" || format_name == "SINGLE-FLOAT" {
        FloatFormat::SingleFloat
    } else if format_name == "DOUBLE-FLOAT" || format_name == "LONG-FLOAT" {
        FloatFormat::DoubleFloat
    } else {
        return Err(RuntimeError::Object(ObjectError::TypeError));
    };
    options.set_default_float_format(float_format);

    let read_suppress =
        common_lisp_variable_value(&mut runtime.context, &runtime.object, "*READ-SUPPRESS*")?;
    options.set_read_suppression(if read_suppress == Word::NIL {
        ReadSuppression::Keep
    } else {
        ReadSuppression::Discard
    });
    Ok(())
}

pub(super) fn common_lisp_variable_value(
    ctx: &mut ThreadContext,
    object: &ObjectRuntime,
    name: &str,
) -> Result<Word, ObjectError> {
    let mut common_lisp = object
        .find_package(ctx, "COMMON-LISP")
        .ok_or(ObjectError::Layout)?;
    with_root(ctx, &mut common_lisp, |ctx, common_lisp| {
        let (mut symbol, _) = Package::from_word(*common_lisp).intern(ctx, object, name)?;
        with_root(ctx, &mut symbol, |ctx, symbol| symbol_value(ctx, *symbol))
    })
}
