use ncl_object::{
    BuiltinArgs, MultipleValues, ObjectError, ObjectRef, Runtime, ThreadContext, Word,
};

pub(super) const fn bool_word(value: bool) -> Word {
    if value { Word::TRUE } else { Word::NIL }
}

pub(super) fn symbolp(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(bool_word(matches!(
        ncl_object::classify_object(ctx, args.required(0)?),
        ObjectRef::Symbol(_)
    )))
}
