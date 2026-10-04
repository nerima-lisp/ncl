use super::super::{
    BuiltinArgs, MultipleValues, ObjectError, Runtime, SLOTS, ThreadContext, Word, make_string,
};
use super::{
    namestring_value, parse_namestring_value, pathname_component_match, pathname_designator,
    structure_ref, translate_wildcards, wildcard_match,
};
use ncl_conditions::{ConditionIdentifier, condition_class_of};
use ncl_object::{Instance, slot_ref};

pub fn file_error_pathname_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let condition = args.required(0)?;
    let Ok(class) = condition_class_of(ctx, condition) else {
        return Ok(Word::NIL);
    };
    let file_error = ConditionIdentifier::FileError
        .class(ctx, runtime)
        .ok_or(ObjectError::Layout)?;
    if class != file_error {
        return Err(ObjectError::TypeError);
    }
    slot_ref(ctx, Instance::from_word(condition), 0)
}

pub fn translate_pathname_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let source = pathname_designator(ctx, runtime, args.required(0)?)?;
    let from = pathname_designator(ctx, runtime, args.required(1)?)?;
    let to = pathname_designator(ctx, runtime, args.required(2)?)?;
    let directory_matches = super::super::wildcard::directory_match(
        ctx,
        structure_ref(ctx, from, 2)?,
        structure_ref(ctx, source, 2)?,
    )?;
    let components_match = directory_matches
        && (0..SLOTS)
            .filter(|index| *index != 2)
            .try_fold(true, |matched, index| {
                Ok::<_, ObjectError>(
                    matched
                        && pathname_component_match(
                            ctx,
                            structure_ref(ctx, from, index)?,
                            structure_ref(ctx, source, index)?,
                        )?,
                )
            })?;
    if !components_match {
        return Err(ObjectError::TypeError);
    }
    let source_name = namestring_value(ctx, source)?;
    let from_name = namestring_value(ctx, from)?;
    let to_name = namestring_value(ctx, to)?;
    if !wildcard_match(&from_name, &source_name) {
        return Err(ObjectError::TypeError);
    }
    let translated =
        translate_wildcards(&from_name, &to_name, &source_name).ok_or(ObjectError::TypeError)?;
    let string = make_string(ctx, runtime, &translated.chars().collect::<Vec<_>>())?;
    parse_namestring_value(ctx, runtime, string)
}
