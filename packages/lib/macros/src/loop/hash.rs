use super::held::{held_form, held_fresh_symbol, held_list};
use super::{HashIterationKind, Result, Runtime, ThreadContext, Word};

pub(super) fn wrap_hash_iteration(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    iteration: (
        usize,
        HashIterationKind,
        usize,
        Option<(HashIterationKind, usize)>,
    ),
    body: usize,
) -> Result<usize> {
    let (variable, kind, table, using) = iteration;
    let (key_variable, value_variable) = if let Some((using_kind, using_variable)) = using {
        if using_kind == HashIterationKind::Key {
            (using_variable, variable)
        } else {
            (variable, using_variable)
        }
    } else {
        let secondary = held_fresh_symbol(ctx, runtime, held)?;
        if kind == HashIterationKind::Key {
            (variable, secondary)
        } else {
            (secondary, variable)
        }
    };
    let parameter_indexes = vec![key_variable, value_variable];
    let parameters = held_list(ctx, runtime, held, &parameter_indexes)?;
    let lambda = held_form(ctx, runtime, held, "LAMBDA", &[parameters, body])?;
    held_form(ctx, runtime, held, "MAPHASH", &[lambda, table])
}
