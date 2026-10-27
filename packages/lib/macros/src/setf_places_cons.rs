use super::{call_form, mutator_place, nth_place};
use crate::form::symbol;
use ncl_object::{
    HeapRootedWord, ObjectError, ObjectRef, Runtime, SetfExpansion, ThreadContext, Word,
    classify_object, string_length, string_ref, symbol_name,
};

/// Expands standardized cons selectors and ordinal list accessors from their names.
pub(crate) fn standard_cons_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    operator: Word,
    args: &[Word],
) -> Result<Option<SetfExpansion>, ObjectError> {
    let operator = HeapRootedWord::new(ctx, operator)?;
    ncl_object::with_heap_rooted_slice(ctx, args, |ctx, args| {
        if !matches!(classify_object(ctx, operator.get()), ObjectRef::Symbol(_)) {
            return Ok(None);
        }
        let name_word = symbol_name(ctx, operator.get())?;
        let name = (0..string_length(ctx, name_word)?)
            .map(|index| string_ref(ctx, name_word, index))
            .collect::<Result<String, _>>()?;
        let ordinal = [
            ("SECOND", 1),
            ("THIRD", 2),
            ("FOURTH", 3),
            ("FIFTH", 4),
            ("SIXTH", 5),
            ("SEVENTH", 6),
            ("EIGHTH", 7),
            ("NINTH", 8),
            ("TENTH", 9),
        ]
        .into_iter()
        .find_map(|(accessor, index)| (name == accessor).then_some(index));
        if let Some(index) = ordinal {
            let expected = symbol(ctx, runtime, &name)?;
            if expected != operator.get() {
                return Ok(None);
            }
            if args.len() != 1 {
                return Err(ObjectError::TypeError);
            }
            let list = args.first().ok_or(ObjectError::TypeError)?.get();
            return nth_place(ctx, runtime, &[Word::fixnum(index), list]).map(Some);
        }

        let name = name.as_bytes();
        if !(3..=6).contains(&name.len())
            || name.first() != Some(&b'C')
            || name.last() != Some(&b'R')
            || !name
                .get(1..name.len() - 1)
                .ok_or(ObjectError::TypeError)?
                .iter()
                .all(|character| matches!(character, b'A' | b'D'))
        {
            return Ok(None);
        }
        let expected = symbol(
            ctx,
            runtime,
            std::str::from_utf8(name).map_err(|_| ObjectError::TypeError)?,
        )?;
        if expected != operator.get() {
            return Ok(None);
        }

        let path = name
            .get(1..name.len() - 1)
            .ok_or(ObjectError::TypeError)?
            .iter()
            .rev()
            .copied()
            .collect::<Vec<_>>();
        let access = if path.last().copied() == Some(b'A') {
            "CAR"
        } else {
            "CDR"
        };
        let parents = path
            .iter()
            .take(path.len().saturating_sub(1))
            .map(|character| if *character == b'A' { "CAR" } else { "CDR" })
            .collect::<Vec<_>>();
        let args = args.iter().map(HeapRootedWord::get).collect::<Vec<_>>();
        let mutator = if access == "CAR" { "RPLACA" } else { "RPLACD" };
        compound_cons_place(ctx, runtime, &args, &parents, access, mutator).map(Some)
    })
}

fn compound_cons_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    parent_accessors: &[&str],
    access_operator: &str,
    mutator_operator: &str,
) -> Result<SetfExpansion, ObjectError> {
    if args.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        let mut parent_form =
            HeapRootedWord::new(ctx, **roots.first().ok_or(ObjectError::TypeError)?)?;
        for accessor in parent_accessors {
            let next_form = call_form(ctx, runtime, accessor, &[parent_form.get()])?;
            parent_form = HeapRootedWord::new(ctx, next_form)?;
        }
        mutator_place(
            ctx,
            runtime,
            &[parent_form.get()],
            access_operator,
            mutator_operator,
        )
    })
}
