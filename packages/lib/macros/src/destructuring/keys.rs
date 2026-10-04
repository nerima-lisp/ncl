//! `&key` support for the destructuring walk (CLHS 3.4.1.4).

use ncl_object::{ObjectError, Runtime, ThreadContext, Word, car, cdr};

use super::held::{
    Result, held_apply, held_call, held_fresh, held_get, held_push, held_raw_list, held_symbol,
};
use super::{bind_target, is_marker, symbol_text, take_one};

/// Find `(quoted-keyword . value ...)` in the plist at `held[plist]`,
/// returning the matching tail (like `member`) or `nil`.
///
/// This does not call the CL `member`/`getf` builtins: `member` currently
/// signals `TypeError` when the list argument is `nil` (confirmed against
/// the built binary; out of this lane's scope, `ncl-lib-sequences`, see
/// the final report), and `getf` is not registered at all. A self-contained
/// `labels` recursion sidesteps both.
fn plist_lookup(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    quoted_keyword: usize,
    plist: usize,
) -> Result {
    let finder = held_fresh(ctx, runtime, held)?;
    let param = held_fresh(ctx, runtime, held)?;
    let null_test = held_call(ctx, runtime, held, "NULL", &[param])?;
    let nil_value = held_push(held, Word::NIL);
    let stop_clause = held_raw_list(ctx, runtime, held, &[null_test, nil_value])?;
    let param_first = held_call(ctx, runtime, held, "CAR", &[param])?;
    let eq_test = held_call(ctx, runtime, held, "EQ", &[param_first, quoted_keyword])?;
    let match_clause = held_raw_list(ctx, runtime, held, &[eq_test, param])?;
    let true_symbol = held_symbol(ctx, runtime, held, "T")?;
    let param_rest = held_call(ctx, runtime, held, "CDR", &[param])?;
    let param_rest_rest = held_call(ctx, runtime, held, "CDR", &[param_rest])?;
    let recurse = held_apply(ctx, runtime, held, finder, &[param_rest_rest])?;
    let recurse_clause = held_raw_list(ctx, runtime, held, &[true_symbol, recurse])?;
    let cond_form = held_call(
        ctx,
        runtime,
        held,
        "COND",
        &[stop_clause, match_clause, recurse_clause],
    )?;
    let params_list = held_raw_list(ctx, runtime, held, &[param])?;
    let definition = held_raw_list(ctx, runtime, held, &[finder, params_list, cond_form])?;
    let definitions = held_raw_list(ctx, runtime, held, &[definition])?;
    let initial_call = held_apply(ctx, runtime, held, finder, &[plist])?;
    held_call(ctx, runtime, held, "LABELS", &[definitions, initial_call])
}

/// The default keyword for a bare `name`: `:NAME` in the `KEYWORD` package.
fn default_keyword(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    name: usize,
) -> Result {
    let word = held_get(held, name)?;
    let text = symbol_text(ctx, word)?;
    held_symbol(ctx, runtime, held, &format!("KEYWORD::{text}"))
}

pub(super) fn walk_key(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    mut remaining: usize,
    cursor: usize,
    bindings: &mut Vec<(usize, usize)>,
) -> std::result::Result<(), ObjectError> {
    loop {
        let word = held_get(held, remaining)?;
        if word == Word::NIL {
            return Ok(());
        }
        if !word.is_cons() {
            return Err(ObjectError::TypeError);
        }
        let element = held_push(held, car(ctx, word)?);
        let tail = held_push(held, cdr(ctx, word)?);
        if is_marker(ctx, held, element, "&ALLOW-OTHER-KEYS")? {
            remaining = tail;
            continue;
        }
        if is_marker(ctx, held, element, "&AUX")? {
            return Err(ObjectError::TypeError);
        }
        // A key spec is `name`, `(name)`, `(name default)`,
        // `(name default supplied-p)`, or with an explicit keyword,
        // `((:keyword name) ...)`. `name` may itself be a pattern.
        let element_word = held_get(held, element)?;
        let (keyword, name, default, supplied) = if element_word.is_cons() {
            let (name_or_pair, rest) = take_one(ctx, held, element)?;
            let name_or_pair_word = held_get(held, name_or_pair)?;
            let (keyword, name) = if name_or_pair_word.is_cons() {
                let (keyword, after) = take_one(ctx, held, name_or_pair)?;
                let (name, _) = take_one(ctx, held, after)?;
                (keyword, name)
            } else {
                (
                    default_keyword(ctx, runtime, held, name_or_pair)?,
                    name_or_pair,
                )
            };
            let rest_word = held_get(held, rest)?;
            if rest_word == Word::NIL {
                (keyword, name, None, None)
            } else {
                let (default, rest) = take_one(ctx, held, rest)?;
                let rest_word = held_get(held, rest)?;
                if rest_word == Word::NIL {
                    (keyword, name, Some(default), None)
                } else {
                    let (supplied, _) = take_one(ctx, held, rest)?;
                    (keyword, name, Some(default), Some(supplied))
                }
            }
        } else {
            let keyword = default_keyword(ctx, runtime, held, element)?;
            (keyword, element, None, None)
        };
        // check-added-lines: allow(panic) Option::unwrap_or_else supplies a fallback; it never panics.
        let default_value = default.unwrap_or_else(|| held_push(held, Word::NIL));
        let quoted_keyword = held_call(ctx, runtime, held, "QUOTE", &[keyword])?;
        let found = plist_lookup(ctx, runtime, held, quoted_keyword, cursor)?;
        let found_symbol = held_fresh(ctx, runtime, held)?;
        bindings.push((found_symbol, found));
        let found_value = held_call(ctx, runtime, held, "CADR", &[found_symbol])?;
        let value = held_call(
            ctx,
            runtime,
            held,
            "IF",
            &[found_symbol, found_value, default_value],
        )?;
        bind_target(ctx, runtime, held, name, value, bindings)?;
        if let Some(supplied) = supplied {
            let true_symbol = held_symbol(ctx, runtime, held, "T")?;
            let nil_value = held_push(held, Word::NIL);
            let supplied_value = held_call(
                ctx,
                runtime,
                held,
                "IF",
                &[found_symbol, true_symbol, nil_value],
            )?;
            bindings.push((supplied, supplied_value));
        }
        remaining = tail;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{elements, list, symbol};

    #[test]
    fn key_walker_builds_default_and_explicit_lookup_bindings()
    -> std::result::Result<(), ObjectError> {
        let runtime = Runtime::new()?;
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)?;
        let name = symbol(&mut ctx, &runtime, "NAME")?;
        let supplied = symbol(&mut ctx, &runtime, "SUPPLIED")?;
        let explicit_name = symbol(&mut ctx, &runtime, "EXPLICIT-NAME")?;
        let explicit_keyword = symbol(&mut ctx, &runtime, "KEYWORD::EXPLICIT")?;
        let default = Word::fixnum(42);
        let explicit_pair = list(&mut ctx, &runtime, &[explicit_keyword, explicit_name])?;
        let first = list(&mut ctx, &runtime, &[name, default, supplied])?;
        let second = list(&mut ctx, &runtime, &[explicit_pair])?;
        let value = symbol(&mut ctx, &runtime, "VALUE")?;
        let specs = list(&mut ctx, &runtime, &[first, second])?;
        let mut held = vec![specs, value];
        let cursor = held_push(&mut held, value);
        let mut bindings = Vec::new();
        walk_key(&mut ctx, &runtime, &mut held, 0, cursor, &mut bindings)?;
        assert!(bindings.len() >= 5);
        let name_binding = bindings
            .iter()
            .map(|(target, form)| (held_get(&held, *target), held_get(&held, *form)))
            .find(|(target, _)| target.as_ref().is_ok_and(|word| *word == name))
            .ok_or(ObjectError::TypeError)?;
        assert_eq!(
            elements(&mut ctx, name_binding.1?)?[0],
            symbol(&mut ctx, &runtime, "IF")?
        );
        Ok(())
    }
}
