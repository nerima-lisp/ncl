use ncl_object::{ObjectError, Runtime, SetfExpansion, ThreadContext, Word};

use super::setter_place;

pub(super) fn char_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<SetfExpansion, ObjectError> {
    string_char_place(ctx, runtime, args, "CHAR")
}

pub(super) fn schar_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<SetfExpansion, ObjectError> {
    string_char_place(ctx, runtime, args, "SCHAR")
}

fn string_char_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    accessor: &str,
) -> Result<SetfExpansion, ObjectError> {
    if args.len() != 2 {
        return Err(ObjectError::TypeError);
    }
    setter_place(ctx, runtime, args, accessor, "NCL-EXT::CHAR-SET")
}
