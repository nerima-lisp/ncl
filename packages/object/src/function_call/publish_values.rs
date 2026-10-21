use crate::{MultipleValues, ObjectError, ThreadContext, Word};

pub(super) fn publish_builtin_values(
    ctx: &mut ThreadContext,
    result: &Result<Word, ObjectError>,
    values: &mut MultipleValues,
) {
    match result {
        Ok(_) if values.is_explicit() => ctx.set_values(values.as_slice()),
        Ok(primary) => ctx.set_values(std::slice::from_ref(primary)),
        Err(_) => {
            values.clear();
            ctx.set_values(&[]);
        }
    }
}
