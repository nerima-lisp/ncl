use super::super::{BuiltinArgs, MultipleValues, ObjectError, Runtime, ThreadContext, Word};
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
