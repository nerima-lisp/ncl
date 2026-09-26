use crate::{PlaceRegistry, SetfExpansion, expand_get_setf_expansion, list};
use ncl_object::{ObjectError, Runtime, ThreadContext, Word};

pub fn expansion_values(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    expansion: &SetfExpansion,
) -> Result<[Word; 5], ObjectError> {
    with_expansion_roots(
        ctx,
        expansion,
        |ctx, temporary, value_forms, store, store_form, access_form| {
            let mut temporary = list(ctx, runtime, temporary)?;
            ncl_object::with_root(ctx, &mut temporary, |ctx, temporary| {
                let mut value_forms = list(ctx, runtime, value_forms)?;
                ncl_object::with_root(ctx, &mut value_forms, |ctx, value_forms| {
                    let mut store = list(ctx, runtime, store)?;
                    ncl_object::with_root(ctx, &mut store, |_, store| {
                        Ok([*temporary, *value_forms, *store, store_form, access_form])
                    })
                })
            })
        },
    )
}

pub fn get_setf_expansion_values(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    place_word: Word,
) -> Result<[Word; 5], ObjectError> {
    let expansion =
        expand_get_setf_expansion(ctx, runtime, &PlaceRegistry::new(runtime), place_word)?;
    expansion_values(ctx, runtime, &expansion)
}

pub fn with_expansion_roots<T>(
    ctx: &mut ThreadContext,
    expansion: &SetfExpansion,
    f: impl FnOnce(&mut ThreadContext, &[Word], &[Word], &[Word], Word, Word) -> Result<T, ObjectError>,
) -> Result<T, ObjectError> {
    if expansion.temporary_variables.len() != expansion.value_forms.len()
        || expansion.store_variables.len() != 1
    {
        return Err(ObjectError::TypeError);
    }
    let temporary_len = expansion.temporary_variables.len();
    let value_len = expansion.value_forms.len();
    let store_len = expansion.store_variables.len();
    let mut words = Vec::with_capacity(temporary_len + value_len + store_len + 2);
    words.extend_from_slice(&expansion.temporary_variables);
    words.extend_from_slice(&expansion.value_forms);
    words.extend_from_slice(&expansion.store_variables);
    words.push(expansion.store_form);
    words.push(expansion.access_form);
    ncl_object::with_roots(ctx, &words, |ctx, roots| {
        let temporary = roots
            .get(..temporary_len)
            .ok_or(ObjectError::TypeError)?
            .iter()
            .map(|root| **root)
            .collect::<Vec<_>>();
        let values = roots
            .get(temporary_len..temporary_len + value_len)
            .ok_or(ObjectError::TypeError)?
            .iter()
            .map(|root| **root)
            .collect::<Vec<_>>();
        let stores = roots
            .get(temporary_len + value_len..temporary_len + value_len + store_len)
            .ok_or(ObjectError::TypeError)?
            .iter()
            .map(|root| **root)
            .collect::<Vec<_>>();
        let store_form = **roots
            .get(temporary_len + value_len + store_len)
            .ok_or(ObjectError::TypeError)?;
        let access_form = **roots
            .get(temporary_len + value_len + store_len + 1)
            .ok_or(ObjectError::TypeError)?;
        ncl_object::with_roots(ctx, &[store_form, access_form], |ctx, fixed| {
            f(
                ctx,
                &temporary,
                &values,
                &stores,
                **fixed.first().ok_or(ObjectError::TypeError)?,
                **fixed.get(1).ok_or(ObjectError::TypeError)?,
            )
        })
    })
}
