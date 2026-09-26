use crate::SetfExpansion;
use ncl_object::{ObjectError, ThreadContext, Word};

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
