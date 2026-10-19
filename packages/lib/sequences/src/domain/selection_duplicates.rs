use super::{SelectionOptions, bounds, call_one, matches, scope_roots};
use ncl_object::{FunctionCaller, ObjectError, Runtime, ThreadContext, Word};

fn rooted_options(
    roots: &[Word],
    options: SelectionOptions,
) -> Result<SelectionOptions, ObjectError> {
    let key = *roots.first().ok_or(ObjectError::Layout)?;
    let test = *roots.get(1).ok_or(ObjectError::Layout)?;
    let test_not = *roots.get(2).ok_or(ObjectError::Layout)?;
    Ok(SelectionOptions {
        key: options.key.map(|_| key),
        test: options.test.map(|_| test),
        test_not: options.test_not.map(|_| test_not),
        ..options
    })
}

pub fn remove_duplicates<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    values: &[Word], // check-added-lines: allow(index) slice type
    options: SelectionOptions,
) -> Result<Vec<Word>, ObjectError> {
    scope_roots(ctx, values, |ctx, rooted_values| {
        let option_words = [
            options.key.unwrap_or(Word::NIL),
            options.test.unwrap_or(Word::NIL),
            options.test_not.unwrap_or(Word::NIL),
        ];
        scope_roots(ctx, &option_words, |ctx, option_roots| {
            let options = rooted_options(option_roots, options)?;
            if options.count.is_some() {
                return Err(ObjectError::TypeError);
            }
            let (start, end) = bounds(options, rooted_values.len())?;
            let keyed_values = vec![Word::NIL; rooted_values.len()];
            scope_roots(ctx, &keyed_values, |ctx, rooted_keys| {
                for index in start..end {
                    let item = *rooted_values.get(index).ok_or(ObjectError::Layout)?;
                    let keyed = if let Some(key) = rooted_options(option_roots, options)?.key {
                        call_one(caller, ctx, runtime, key, item)?
                    } else {
                        item
                    };
                    *rooted_keys.get_mut(index).ok_or(ObjectError::Layout)? = keyed;
                }

                let candidates = if options.from_end {
                    (start..end).collect::<Vec<_>>()
                } else {
                    (start..end).rev().collect::<Vec<_>>()
                };
                let mut removed = vec![false; rooted_values.len()];
                let mut retained = Vec::new();
                for index in candidates {
                    let mut duplicate = false;
                    for &previous in &retained {
                        let candidate = *rooted_keys.get(index).ok_or(ObjectError::Layout)?;
                        let prior = *rooted_keys.get(previous).ok_or(ObjectError::Layout)?;
                        let options = SelectionOptions {
                            key: None,
                            count: None,
                            ..rooted_options(option_roots, options)?
                        };
                        if matches(caller, ctx, runtime, options, candidate, prior)? {
                            duplicate = true;
                            break;
                        }
                    }
                    if duplicate {
                        *removed.get_mut(index).ok_or(ObjectError::Layout)? = true;
                    } else {
                        retained.push(index);
                    }
                }

                let mut result = Vec::with_capacity(
                    rooted_values.len() - removed.iter().filter(|&&v| v).count(),
                );
                for (index, value) in rooted_values.iter().enumerate() {
                    if !*removed.get(index).ok_or(ObjectError::Layout)? {
                        result.push(*value);
                    }
                }
                Ok(result)
            })
        })
    })
}
