//! `when`/`unless`/`if` conditional-clause holding and expansion, plus
//! destructuring `FOR` variables (both walk held or user-written list
//! structure the same recursive way).

use super::accumulator::expand_accumulator;
use super::clause::HeldLoopClause;
use super::held::{held_form, held_get, held_list, held_symbol};
use super::{
    AccumulatorKind, ConditionalKind, ObjectError, Result, Runtime, ThreadContext, Word,
    string_length, string_ref, symbol_name,
};

/// The upper-cased print name of `word`, or `None` if it is not a symbol.
fn word_upper_name(ctx: &ThreadContext, word: Word) -> Option<String> {
    let name = symbol_name(ctx, word).ok()?;
    let length = string_length(ctx, name).ok()?;
    (0..length)
        .map(|index| string_ref(ctx, name, index))
        .collect::<std::result::Result<String, _>>()
        .ok()
        .map(|text| text.to_ascii_uppercase())
}

/// Find a symbol named `IT` anywhere in `word` (which may be an arbitrary,
/// possibly nested, list form), returning the exact symbol object the user
/// wrote so a generated `it` binding shares its identity: `it` is only
/// visible where the user's own occurrence resolves to the same interned
/// symbol (its home package), not to any package's `IT`.
pub(super) fn find_it_word(ctx: &ThreadContext, word: Word) -> Result<Option<Word>> {
    if word.is_cons() {
        let mut cursor = word;
        loop {
            if !cursor.is_cons() {
                return if cursor == Word::NIL {
                    Ok(None)
                } else {
                    find_it_word(ctx, cursor)
                };
            }
            let head = ncl_object::car(ctx, cursor)?;
            if let Some(found) = find_it_word(ctx, head)? {
                return Ok(Some(found));
            }
            cursor = ncl_object::cdr(ctx, cursor)?;
        }
    }
    Ok(if word_upper_name(ctx, word).as_deref() == Some("IT") {
        Some(word)
    } else {
        None
    })
}

/// Destructure `pattern` (a `d-var-spec`: a symbol, `nil`, or a proper list
/// of `d-var-spec`s) against the already-bound value held at `source`,
/// pushing one `(leaf nil)` binding per leaf symbol into `bindings` and one
/// `(setq leaf accessor)` form per leaf into `body`, in left-to-right order.
#[allow(clippy::too_many_arguments)]
fn destructure(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    pattern: Word,
    source: usize,
    nil_index: usize,
    bindings: &mut Vec<usize>,
    body: &mut Vec<usize>,
) -> Result<()> {
    if pattern == Word::NIL {
        return Ok(());
    }
    if pattern.is_cons() {
        // The first element and the remaining sub-pattern must survive the
        // allocating `held_form` calls below; push them onto `held`
        // immediately so they are refreshed the same way every other
        // GC-managed word this module carries is, then read them back rather
        // than keeping the pre-call `Word`s alive raw.
        let first_pattern_index = held.len();
        held.push(ncl_object::car(ctx, pattern)?);
        let rest_pattern_index = held.len();
        held.push(ncl_object::cdr(ctx, pattern)?);
        let first_source = held_form(ctx, runtime, held, "CAR", &[source])?;
        let first_pattern = held_get(held, first_pattern_index)?;
        destructure(
            ctx,
            runtime,
            held,
            first_pattern,
            first_source,
            nil_index,
            bindings,
            body,
        )?;
        let rest_pattern = held_get(held, rest_pattern_index)?;
        if rest_pattern != Word::NIL {
            let rest_source = held_form(ctx, runtime, held, "CDR", &[source])?;
            let rest_pattern = held_get(held, rest_pattern_index)?;
            destructure(
                ctx,
                runtime,
                held,
                rest_pattern,
                rest_source,
                nil_index,
                bindings,
                body,
            )?;
        }
        return Ok(());
    }
    symbol_name(ctx, pattern)?;
    let pattern_index = held.len();
    held.push(pattern);
    bindings.push(held_list(ctx, runtime, held, &[pattern_index, nil_index])?);
    body.push(held_form(
        ctx,
        runtime,
        held,
        "SETQ",
        &[pattern_index, source],
    )?);
    Ok(())
}

/// Bind a `for`-clause variable, which may be a plain symbol or a
/// destructuring pattern, to `current` for this iteration.
#[allow(clippy::too_many_arguments)]
pub(super) fn bind_for_variable(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    variable: usize,
    current: usize,
    nil_index: usize,
    bindings: &mut Vec<usize>,
    body: &mut Vec<usize>,
) -> Result<()> {
    let variable_word = held_get(held, variable)?;
    if variable_word.is_cons() {
        destructure(
            ctx,
            runtime,
            held,
            variable_word,
            current,
            nil_index,
            bindings,
            body,
        )
    } else {
        symbol_name(ctx, variable_word)?;
        bindings.push(held_list(ctx, runtime, held, &[variable, nil_index])?);
        body.push(held_form(ctx, runtime, held, "SETQ", &[variable, current])?);
        Ok(())
    }
}

/// Expand one of the clause kinds allowed inside a `when`/`unless`/`if`
/// selectable-clause list (`do`, `return`, an accumulation clause, or a
/// nested conditional), appending the forms it produces to `body`. Also used
/// to expand the top-level loop body's `do`/`return`/accumulation/conditional
/// clauses, so the two paths stay in sync.
#[allow(clippy::too_many_arguments)]
pub(super) fn expand_selectable_clause(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    clause: &HeldLoopClause,
    name: Option<usize>,
    nil: usize,
    it: Option<usize>,
    bindings: &mut Vec<usize>,
    body: &mut Vec<usize>,
    initialized_accumulators: &mut Vec<usize>,
    result: &mut usize,
    result_kind: &mut Option<AccumulatorKind>,
) -> Result<()> {
    match *clause {
        HeldLoopClause::Do(ref forms) => body.extend(forms.iter().copied()),
        HeldLoopClause::Return(value) => {
            let target = name.unwrap_or(nil);
            body.push(held_form(
                ctx,
                runtime,
                held,
                "RETURN-FROM",
                &[target, value],
            )?);
        }
        HeldLoopClause::Accumulate {
            kind,
            form: value,
            variable,
        } => {
            let (accumulator, kind) = expand_accumulator(
                ctx,
                runtime,
                held,
                kind,
                value,
                variable,
                bindings,
                body,
                initialized_accumulators,
            )?;
            *result = accumulator;
            *result_kind = Some(kind);
        }
        HeldLoopClause::Conditional {
            kind,
            test,
            ref then,
            ref otherwise,
        } => {
            expand_conditional(
                ctx,
                runtime,
                held,
                kind,
                test,
                then,
                otherwise,
                name,
                nil,
                it,
                bindings,
                body,
                initialized_accumulators,
                result,
                result_kind,
            )?;
        }
        // check-added-lines: allow(wildcard) only selectable clauses reach here.
        _ => return Err(ObjectError::TypeError),
    }
    Ok(())
}

/// Expand a `when`/`unless`/`if` clause into a single `(let ((it test)) (if
/// <it-or-not-it> then-progn else-progn))` form, pushed onto `body`. `it` is
/// bound to whichever symbol the user's own loop form used to spell `IT`
/// (see [`find_it_word`]): a real lexical binding, not a gensym, so
/// `then`/`otherwise` forms written with that symbol observe the test's
/// value, per ANSI CL 6.1.3. Falls back to a `COMMON-LISP::IT` binding when
/// the loop form never mentions `it` (the binding is then simply unused).
#[allow(clippy::too_many_arguments)]
fn expand_conditional(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    kind: ConditionalKind,
    test: usize,
    then: &[HeldLoopClause],
    otherwise: &[HeldLoopClause],
    name: Option<usize>,
    nil: usize,
    it: Option<usize>,
    bindings: &mut Vec<usize>,
    body: &mut Vec<usize>,
    initialized_accumulators: &mut Vec<usize>,
    result: &mut usize,
    result_kind: &mut Option<AccumulatorKind>,
) -> Result<()> {
    let it = match it {
        Some(it) => it,
        None => held_symbol(ctx, runtime, held, "IT")?,
    };
    let it_binding = held_list(ctx, runtime, held, &[it, test])?;
    let it_bindings = held_list(ctx, runtime, held, &[it_binding])?;

    let mut then_body = Vec::new();
    for clause in then {
        expand_selectable_clause(
            ctx,
            runtime,
            held,
            clause,
            name,
            nil,
            Some(it),
            bindings,
            &mut then_body,
            initialized_accumulators,
            result,
            result_kind,
        )?;
    }
    let then_progn = held_form(ctx, runtime, held, "PROGN", &then_body)?;

    let else_progn = if otherwise.is_empty() {
        nil
    } else {
        let mut else_body = Vec::new();
        for clause in otherwise {
            expand_selectable_clause(
                ctx,
                runtime,
                held,
                clause,
                name,
                nil,
                Some(it),
                bindings,
                &mut else_body,
                initialized_accumulators,
                result,
                result_kind,
            )?;
        }
        held_form(ctx, runtime, held, "PROGN", &else_body)?
    };

    let condition = match kind {
        ConditionalKind::When | ConditionalKind::If => it,
        ConditionalKind::Unless => held_form(ctx, runtime, held, "NOT", &[it])?,
    };
    let if_form = held_form(
        ctx,
        runtime,
        held,
        "IF",
        &[condition, then_progn, else_progn],
    )?;
    let let_form = held_form(ctx, runtime, held, "LET", &[it_bindings, if_form])?;
    body.push(let_form);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{elements, list, symbol};

    #[test]
    fn conditional_helpers_find_nested_it_and_emit_when_unless_forms()
    -> std::result::Result<(), ObjectError> {
        let runtime = Runtime::new()?;
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)?;
        let it = symbol(&mut ctx, &runtime, "IT")?;
        let nested = list(&mut ctx, &runtime, &[Word::fixnum(1), it])?;
        assert_eq!(find_it_word(&ctx, nested)?, Some(it));
        assert_eq!(find_it_word(&ctx, Word::NIL)?, None);
        assert_eq!(find_it_word(&ctx, Word::fixnum(1))?, None);

        for kind in [
            ConditionalKind::When,
            ConditionalKind::Unless,
            ConditionalKind::If,
        ] {
            let test = symbol(&mut ctx, &runtime, "TEST")?;
            let body_form = symbol(&mut ctx, &runtime, "BODY")?;
            let mut held = vec![test, body_form];
            let mut bindings = Vec::new();
            let mut body = Vec::new();
            let mut initialized = Vec::new();
            let mut result = 0;
            let mut result_kind = None;
            let clause = HeldLoopClause::Do(vec![1]);
            expand_conditional(
                &mut ctx,
                &runtime,
                &mut held,
                kind,
                0,
                &[clause],
                &[],
                None,
                1,
                None,
                &mut bindings,
                &mut body,
                &mut initialized,
                &mut result,
                &mut result_kind,
            )?;
            let form = elements(&mut ctx, held[*body.first().expect("conditional body")])?;
            assert_eq!(form[0], symbol(&mut ctx, &runtime, "LET")?);
        }
        Ok(())
    }
}
