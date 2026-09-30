use super::accumulator::expand_accumulator;
use super::clause::HeldLoopClause;
use super::hash::wrap_hash_iteration;
use super::held::{expand_body, held_form, held_fresh_symbol, held_get, held_list, held_symbol};
use super::{
    AccumulatorKind, ConditionalKind, LimitDirection, LoopAst, LoopClause, ObjectError, Result,
    Runtime, StepDirection, ThreadContext, Word, string_length, string_ref, symbol_name,
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
/// wrote so a generated `it` binding shares its identity — `it` is only
/// visible where the user's own occurrence resolves to the same interned
/// symbol (its home package), not to any package's `IT`.
fn find_it_word(ctx: &ThreadContext, word: Word) -> Result<Option<Word>> {
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

/// Convert one parsed [`LoopClause`] into a [`HeldLoopClause`], pushing any
/// `Word` payloads it carries onto `held` so the rest of expansion only deals
/// in GC-safe indices. Recurses for the selectable clauses nested inside a
/// `when`/`unless`/`if` conditional clause.
#[allow(clippy::too_many_lines)]
fn hold_clause(
    ctx: &mut ThreadContext,
    held: &mut Vec<Word>,
    clause: &LoopClause,
) -> Result<HeldLoopClause> {
    Ok(match *clause {
        LoopClause::With { variable, init } => {
            let variable_index = held.len();
            held.push(variable);
            let init_index = held.len();
            held.push(init);
            HeldLoopClause::With {
                variable: variable_index,
                init: init_index,
            }
        }
        LoopClause::For(spec) => {
            let variable = held.len();
            held.push(spec.variable);
            let init = held.len();
            held.push(spec.init);
            let step = spec.step.map(|step| {
                let index = held.len();
                held.push(step);
                index
            });
            let limit = spec.limit.map(|(direction, limit)| {
                let index = held.len();
                held.push(limit);
                (direction, index)
            });
            HeldLoopClause::For {
                variable,
                init,
                step,
                direction: spec.direction,
                limit,
            }
        }
        LoopClause::Hash(spec) => {
            symbol_name(ctx, spec.variable)?;
            let variable = held.len();
            held.push(spec.variable);
            let table = held.len();
            held.push(spec.table);
            let using = spec.using.map(|(kind, variable)| {
                let index = held.len();
                held.push(variable);
                (kind, index)
            });
            HeldLoopClause::Hash {
                variable,
                kind: spec.kind,
                table,
                using,
            }
        }
        LoopClause::EqualsThen {
            variable,
            init,
            then,
        } => {
            symbol_name(ctx, variable)?;
            let variable_index = held.len();
            held.push(variable);
            let init_index = held.len();
            held.push(init);
            let then_index = held.len();
            held.push(then);
            HeldLoopClause::EqualsThen {
                variable: variable_index,
                init: init_index,
                then: then_index,
            }
        }
        LoopClause::In {
            variable,
            sequence,
            on,
            by,
        } => {
            // `variable` may be a destructuring pattern rather than a plain
            // symbol; leaf symbols are validated once the pattern is walked
            // during body expansion.
            let variable_index = held.len();
            held.push(variable);
            let sequence_index = held.len();
            held.push(sequence);
            let by = by.map(|by| {
                let index = held.len();
                held.push(by);
                index
            });
            HeldLoopClause::In {
                variable: variable_index,
                sequence: sequence_index,
                on,
                by,
            }
        }
        LoopClause::Across { variable, vector } => {
            symbol_name(ctx, variable)?;
            let variable_index = held.len();
            held.push(variable);
            let vector_index = held.len();
            held.push(vector);
            HeldLoopClause::Across {
                variable: variable_index,
                vector: vector_index,
            }
        }
        LoopClause::Repeat(count) => {
            let index = held.len();
            held.push(count);
            HeldLoopClause::Repeat(index)
        }
        LoopClause::While(test) => {
            let index = held.len();
            held.push(test);
            HeldLoopClause::While(index)
        }
        LoopClause::Until(test) => {
            let index = held.len();
            held.push(test);
            HeldLoopClause::Until(index)
        }
        LoopClause::Initially(ref forms) => HeldLoopClause::Initially(
            forms
                .iter()
                .map(|word| {
                    let index = held.len();
                    held.push(*word);
                    index
                })
                .collect(),
        ),
        LoopClause::Finally(ref forms) => HeldLoopClause::Finally(
            forms
                .iter()
                .map(|word| {
                    let index = held.len();
                    held.push(*word);
                    index
                })
                .collect(),
        ),
        LoopClause::Do(ref forms) => HeldLoopClause::Do(
            forms
                .iter()
                .map(|word| {
                    let index = held.len();
                    held.push(*word);
                    index
                })
                .collect(),
        ),
        LoopClause::Accumulate {
            kind,
            form,
            variable,
        } => {
            let form_index = held.len();
            held.push(form);
            let variable = variable.map(|variable| {
                let index = held.len();
                held.push(variable);
                index
            });
            HeldLoopClause::Accumulate {
                kind,
                form: form_index,
                variable,
            }
        }
        LoopClause::Return(value) => {
            let index = held.len();
            held.push(value);
            HeldLoopClause::Return(index)
        }
        LoopClause::Conditional {
            kind,
            test,
            ref then,
            ref otherwise,
        } => {
            let test_index = held.len();
            held.push(test);
            let then = hold_clauses(ctx, held, then)?;
            let otherwise = hold_clauses(ctx, held, otherwise)?;
            HeldLoopClause::Conditional {
                kind,
                test: test_index,
                then,
                otherwise,
            }
        }
    })
}

fn hold_clauses(
    ctx: &mut ThreadContext,
    held: &mut Vec<Word>,
    clauses: &[LoopClause],
) -> Result<Vec<HeldLoopClause>> {
    let mut result = Vec::with_capacity(clauses.len());
    for clause in clauses {
        result.push(hold_clause(ctx, held, clause)?);
    }
    Ok(result)
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
fn bind_for_variable(
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
fn expand_selectable_clause(
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
/// bound to whichever symbol the user's own loop form used to spell `IT` (see
/// [`find_it_word`]) — a real lexical binding, not a gensym — so
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

/// Expand a parsed LOOP AST into portable CL primitive forms.
#[allow(clippy::too_many_lines)]
pub fn expand_loop_ast(ctx: &mut ThreadContext, runtime: &Runtime, ast: &LoopAst) -> Result {
    let mut held = Vec::new();
    let name = ast.name.map(|name| {
        held.push(name);
        held.len() - 1
    });
    let clauses = hold_clauses(ctx, &mut held, &ast.clauses)?;
    // Every occurrence of `it` the user wrote anywhere in the loop form is
    // the same interned symbol object (interning is idempotent), so a single
    // pass over everything already held finds it once for the whole
    // expansion; conditional clauses bind exactly this symbol (see
    // `expand_conditional`) so lexical lookup resolves the way the user
    // wrote it, regardless of the current package.
    let it_word = held
        .iter()
        .find_map(|&word| find_it_word(ctx, word).ok().flatten());
    let it = it_word.map(|word| {
        let index = held.len();
        held.push(word);
        index
    });
    let end = held_fresh_symbol(ctx, runtime, &mut held)?;
    let start = held_fresh_symbol(ctx, runtime, &mut held)?;
    let mut bindings = Vec::new();
    let mut updates = Vec::new();
    let mut tests = Vec::new();
    let mut body = Vec::new();
    let mut initially = Vec::new();
    let mut finally: Vec<usize> = Vec::new();
    let nil = held.len();
    held.push(Word::NIL);
    let mut result = nil;
    let mut result_kind = None;
    let mut initialized_accumulators = Vec::new();
    let mut hash_iteration = None;
    let mut has_iteration_driver = false;

    for clause in &clauses {
        match *clause {
            HeldLoopClause::With { variable, init } => {
                bindings.push(held_list(ctx, runtime, &mut held, &[variable, init])?);
            }
            HeldLoopClause::For {
                variable,
                init,
                step: spec_step,
                direction,
                limit,
            } => {
                has_iteration_driver = true;
                bindings.push(held_list(ctx, runtime, &mut held, &[variable, init])?);
                let step = if spec_step.is_some() {
                    spec_step.ok_or(ObjectError::TypeError)?
                } else {
                    let step = held.len();
                    held.push(Word::fixnum(
                        if matches!(direction, Some(StepDirection::DownFrom)) {
                            -1
                        } else {
                            1
                        },
                    ));
                    step
                };
                let down = matches!(direction, Some(StepDirection::DownFrom))
                    || matches!(
                        limit.map(|(kind, _)| kind),
                        Some(LimitDirection::DownTo | LimitDirection::Above)
                    );
                let operator = if down { "-" } else { "+" };
                let update = held_form(ctx, runtime, &mut held, operator, &[variable, step])?;
                updates.extend([variable, update]);
                if let Some((limit_direction, limit)) = limit {
                    let operator = if down {
                        if matches!(limit_direction, LimitDirection::Above) {
                            "<="
                        } else {
                            "<"
                        }
                    } else if matches!(limit_direction, LimitDirection::Below) {
                        ">="
                    } else {
                        ">"
                    };
                    tests.push(held_form(
                        ctx,
                        runtime,
                        &mut held,
                        operator,
                        &[variable, limit],
                    )?);
                }
            }
            HeldLoopClause::Hash {
                variable,
                kind,
                table,
                using,
            } => {
                if hash_iteration.is_some() {
                    return Err(ObjectError::TypeError);
                }
                if using.is_some_and(|(_, using_variable)| {
                    held.get(using_variable) == held.get(variable)
                }) {
                    return Err(ObjectError::TypeError);
                }
                hash_iteration = Some((variable, kind, table, using));
            }
            HeldLoopClause::EqualsThen {
                variable,
                init,
                then,
            } => {
                has_iteration_driver = true;
                bindings.push(held_list(ctx, runtime, &mut held, &[variable, init])?);
                updates.extend([variable, then]);
            }
            HeldLoopClause::In {
                variable,
                sequence,
                on,
                by,
            } => {
                has_iteration_driver = true;
                let cursor = held_fresh_symbol(ctx, runtime, &mut held)?;
                let nil_index = held.len();
                held.push(Word::NIL);
                bindings.push(held_list(ctx, runtime, &mut held, &[cursor, sequence])?);
                tests.push(held_form(ctx, runtime, &mut held, "ENDP", &[cursor])?);
                let current = if on {
                    cursor
                } else {
                    held_form(ctx, runtime, &mut held, "CAR", &[cursor])?
                };
                bind_for_variable(
                    ctx,
                    runtime,
                    &mut held,
                    variable,
                    current,
                    nil_index,
                    &mut bindings,
                    &mut body,
                )?;
                let next = if let Some(by) = by {
                    held_form(ctx, runtime, &mut held, "FUNCALL", &[by, cursor])?
                } else {
                    held_form(ctx, runtime, &mut held, "CDR", &[cursor])?
                };
                updates.extend([cursor, next]);
            }
            HeldLoopClause::Across { variable, vector } => {
                has_iteration_driver = true;
                let index = held_fresh_symbol(ctx, runtime, &mut held)?;
                let vector_binding = held_fresh_symbol(ctx, runtime, &mut held)?;
                bindings.push(held_list(
                    ctx,
                    runtime,
                    &mut held,
                    &[vector_binding, vector],
                )?);
                let zero = held.len();
                held.push(Word::fixnum(0));
                let nil_index = held.len();
                held.push(Word::NIL);
                bindings.push(held_list(ctx, runtime, &mut held, &[index, zero])?);
                bindings.push(held_list(ctx, runtime, &mut held, &[variable, nil_index])?);
                let length = held_form(
                    ctx,
                    runtime,
                    &mut held,
                    "ARRAY-TOTAL-SIZE",
                    &[vector_binding],
                )?;
                tests.push(held_form(ctx, runtime, &mut held, ">=", &[index, length])?);
                let element = held_form(ctx, runtime, &mut held, "AREF", &[vector_binding, index])?;
                body.push(held_form(
                    ctx,
                    runtime,
                    &mut held,
                    "SETQ",
                    &[variable, element],
                )?);
                let one = held.len();
                held.push(Word::fixnum(1));
                updates.extend([
                    index,
                    held_form(ctx, runtime, &mut held, "+", &[index, one])?,
                ]);
            }
            HeldLoopClause::Repeat(count) => {
                has_iteration_driver = true;
                let counter = held_fresh_symbol(ctx, runtime, &mut held)?;
                bindings.push(held_list(ctx, runtime, &mut held, &[counter, count])?);
                let zero = held.len();
                held.push(Word::fixnum(0));
                tests.push(held_form(ctx, runtime, &mut held, "<=", &[counter, zero])?);
                let one = held.len();
                held.push(Word::fixnum(1));
                updates.extend([
                    counter,
                    held_form(ctx, runtime, &mut held, "-", &[counter, one])?,
                ]);
            }
            HeldLoopClause::While(test) => {
                tests.push(held_form(ctx, runtime, &mut held, "NOT", &[test])?);
            }
            HeldLoopClause::Until(test) => tests.push(test),
            HeldLoopClause::Initially(ref forms) => initially.extend(forms),
            HeldLoopClause::Finally(ref forms) => finally.extend(forms),
            HeldLoopClause::Do(_)
            | HeldLoopClause::Return(_)
            | HeldLoopClause::Accumulate { .. }
            | HeldLoopClause::Conditional { .. } => {
                expand_selectable_clause(
                    ctx,
                    runtime,
                    &mut held,
                    clause,
                    name,
                    nil,
                    it,
                    &mut bindings,
                    &mut body,
                    &mut initialized_accumulators,
                    &mut result,
                    &mut result_kind,
                )?;
            }
        }
    }
    let body = expand_body(ctx, runtime, &mut held, &body, end)?;
    let loop_body = if hash_iteration.is_some() && !has_iteration_driver {
        let mut tagbody = vec![start];
        if !tests.is_empty() {
            let test = held_form(ctx, runtime, &mut held, "OR", &tests)?;
            let go_end = held_form(ctx, runtime, &mut held, "GO", &[end])?;
            tagbody.push(held_form(ctx, runtime, &mut held, "WHEN", &[test, go_end])?);
        }
        tagbody.extend(body);
        tagbody.push(end);
        held_form(ctx, runtime, &mut held, "TAGBODY", &tagbody)?
    } else {
        let stop = if tests.is_empty() {
            let index = held.len();
            held.push(Word::NIL);
            index
        } else {
            let test = held_form(ctx, runtime, &mut held, "OR", &tests)?;
            let go_end = held_form(ctx, runtime, &mut held, "GO", &[end])?;
            held_form(ctx, runtime, &mut held, "WHEN", &[test, go_end])?
        };
        let mut tagbody = vec![start, stop];
        tagbody.extend(body);
        if !updates.is_empty() {
            tagbody.push(held_form(ctx, runtime, &mut held, "SETQ", &updates)?);
        }
        tagbody.push(held_form(ctx, runtime, &mut held, "GO", &[start])?);
        tagbody.push(end);
        held_form(ctx, runtime, &mut held, "TAGBODY", &tagbody)?
    };
    let loop_body = if let Some(iteration) = hash_iteration {
        wrap_hash_iteration(ctx, runtime, &mut held, iteration, loop_body)?
    } else {
        loop_body
    };
    let value = if matches!(result_kind, Some(AccumulatorKind::Collect)) {
        held_form(ctx, runtime, &mut held, "NREVERSE", &[result])?
    } else {
        result
    };
    let mut block_body = initially;
    block_body.push(loop_body);
    block_body.extend(finally);
    block_body.push(value);
    let block_progn = held_form(ctx, runtime, &mut held, "PROGN", &block_body)?;
    let block = held_form(
        ctx,
        runtime,
        &mut held,
        "BLOCK",
        &[name.unwrap_or(nil), block_progn],
    )?;
    let binding_list = held_list(ctx, runtime, &mut held, &bindings)?;
    let expansion = held_form(ctx, runtime, &mut held, "LET", &[binding_list, block])?;
    held_get(&held, expansion)
}
