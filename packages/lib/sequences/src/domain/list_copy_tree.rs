//! `COPY-TREE`, split out of `list.rs` to keep that file under the project's
//! line-count limit.

use ncl_object::{LispError, ObjectError, Runtime, ThreadContext, Word, car as object_car, cdr as object_cdr};

use super::make_cons_rooted;

fn rooted_slot(rooted: &[Word], index: usize) -> Result<Word, ObjectError> {
    rooted.get(index).copied().ok_or(ObjectError::Layout)
}

/// Recursively copy every cons cell of `value`, including cons cells found
/// while descending into CARs. Non-cons atoms (including a dotted tail) are
/// shared, matching CLHS `copy-tree`.
///
/// The top-level cdr spine is walked iteratively so arbitrarily long lists do
/// not recurse on the native stack; only CAR subtrees recurse. All heap
/// values are kept rooted for the duration of the (possibly allocating)
/// recursive walk.
fn copy_tree_inner(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Word,
) -> Result<Word, ObjectError> {
    if !value.is_cons() {
        return Ok(value);
    }
    let mut elements = Vec::new();
    let mut cursor = value;
    while cursor.is_cons() {
        elements.push(object_car(ctx, cursor)?);
        cursor = object_cdr(ctx, cursor)?;
    }
    let tail_index = elements.len();
    elements.push(cursor);
    let accumulator_index = elements.len();
    elements.push(Word::NIL);
    ncl_object::with_rooted_slice(ctx, &elements, |ctx, rooted| {
        let tail = rooted_slot(rooted, tail_index)?;
        let copied_tail = copy_tree_inner(ctx, runtime, tail)?;
        *rooted.get_mut(accumulator_index).ok_or(ObjectError::Layout)? = copied_tail;
        for index in (0..tail_index).rev() {
            let car_value = rooted_slot(rooted, index)?;
            let new_car = copy_tree_inner(ctx, runtime, car_value)?;
            let accumulator = rooted_slot(rooted, accumulator_index)?;
            let new_pair = make_cons_rooted(ctx, runtime, new_car, accumulator)?;
            *rooted.get_mut(accumulator_index).ok_or(ObjectError::Layout)? = new_pair;
        }
        rooted_slot(rooted, accumulator_index)
    })
}

pub fn copy_tree(ctx: &mut ThreadContext, runtime: &Runtime, value: Word) -> Result<Word, LispError> {
    copy_tree_inner(ctx, runtime, value).map_err(LispError::from)
}
