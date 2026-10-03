use super::clause::HeldLoopClause;
use super::conditional::{bind_for_variable, expand_selectable_clause, find_it_word};
use super::hash::wrap_hash_iteration;
use super::held::{expand_body, held_form, held_fresh_symbol, held_get, held_list};
use super::holding::hold_clauses;
use super::{
    AccumulatorKind, LimitDirection, LoopAst, ObjectError, Result, Runtime, StepDirection,
    ThreadContext, Word,
};

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
            HeldLoopClause::Equals { variable, init } => {
                bindings.push(held_list(ctx, runtime, &mut held, &[variable, nil])?);
                body.push(held_form(
                    ctx,
                    runtime,
                    &mut held,
                    "SETQ",
                    &[variable, init],
                )?);
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
                let next = if let Some(by) = by {
                    held_form(ctx, runtime, &mut held, "FUNCALL", &[by, cursor])?
                } else {
                    held_form(ctx, runtime, &mut held, "CDR", &[cursor])?
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
