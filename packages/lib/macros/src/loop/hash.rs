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

#[cfg(any())]
mod tests {
    use super::*;
    use crate::elements;
    use ncl_object::ObjectError;

    #[test]
    fn hash_iteration_maps_explicit_and_generated_key_value_pairs()
    -> std::result::Result<(), ObjectError> {
        let runtime = Runtime::new()?;
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)?;
        let key = Word::fixnum(1);
        let value = Word::fixnum(2);
        let table = Word::fixnum(3);
        let body = Word::fixnum(4);
        for (kind, using) in [
            (HashIterationKind::Key, Some((HashIterationKind::Value, 1))),
            (HashIterationKind::Value, Some((HashIterationKind::Key, 0))),
        ] {
            let mut held = vec![key, value, table, body];
            let result =
                wrap_hash_iteration(&mut ctx, &runtime, &mut held, (0, kind, 2, using), 3)?;
            let form = elements(&mut ctx, held[result])?;
            assert_eq!(
                ncl_object::string_ref(&ctx, ncl_object::symbol_name(&ctx, form[0])?, 0)?,
                'M'
            );
            assert_eq!(elements(&mut ctx, form[2])?[0], table);
        }
        Ok(())
    }
}
