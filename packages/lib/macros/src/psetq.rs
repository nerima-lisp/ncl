use super::control::{args, bindings, form};
use ncl_object::{
    BuiltinArgs, MultipleValues, ObjectError, Runtime, ThreadContext, Word, symbol_name,
};

type Result<T = Word> = std::result::Result<T, ObjectError>;

pub fn expand(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    input: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result {
    let input_form = input.get(0).ok_or(ObjectError::TypeError)?;
    let values = args(ctx, input_form)?;
    if !values.len().is_multiple_of(2) {
        return Err(ObjectError::TypeError);
    }
    ncl_object::with_roots(ctx, &values, |ctx, roots| {
        let temporaries = vec![Word::NIL; values.len() / 2];
        ncl_object::with_rooted_slice(ctx, &temporaries, |ctx, temporaries| {
            for temporary in temporaries.iter_mut() {
                *temporary = crate::fresh_symbol(ctx, runtime)?;
            }
            let mut binding_pairs = Vec::with_capacity(values.len() / 2);
            for (index, temporary) in temporaries.iter_mut().enumerate() {
                // check-added-lines: allow(index) index is bounded by the rooted value pairs.
                let variable = **roots.get(index * 2).ok_or(ObjectError::TypeError)?;
                symbol_name(ctx, variable)?;
                // check-added-lines: allow(index) index is bounded by the rooted value pairs.
                let value = **roots.get(index * 2 + 1).ok_or(ObjectError::TypeError)?;
                binding_pairs.push((*temporary, value));
            }
            let binding_list = bindings(ctx, runtime, &binding_pairs)?;
            let mut assignments = Vec::with_capacity(values.len());
            for (index, temporary) in temporaries.iter().enumerate() {
                // check-added-lines: allow(index) index is bounded by the rooted value pairs.
                let variable = **roots.get(index * 2).ok_or(ObjectError::TypeError)?;
                assignments.extend([variable, *temporary]);
            }
            ncl_object::with_roots(ctx, &[binding_list], |ctx, roots| {
                let binding_list = **roots.first().ok_or(ObjectError::TypeError)?;
                let assignment = form(ctx, runtime, "SETQ", &assignments)?;
                form(ctx, runtime, "LET", &[binding_list, assignment])
            })
        })
    })
}
