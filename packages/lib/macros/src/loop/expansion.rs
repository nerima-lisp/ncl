use super::clause::HeldLoopClause;
use super::held::{expand_body, held_form, held_fresh_symbol, held_get, held_list};
use super::{
    AccumulatorKind, LimitDirection, LoopAst, LoopClause, ObjectError, Result, Runtime,
    StepDirection, ThreadContext, Word, symbol_name,
};

/// Expand a parsed LOOP AST into portable CL primitive forms.
#[allow(clippy::too_many_lines)]
pub fn expand_loop_ast(ctx: &mut ThreadContext, runtime: &Runtime, ast: &LoopAst) -> Result {
    let mut held = Vec::new();
    let name = ast.name.map(|name| {
        held.push(name);
        held.len() - 1
    });
    let mut clauses: Vec<HeldLoopClause> = Vec::with_capacity(ast.clauses.len());
    for clause in &ast.clauses {
        let clause = match *clause {
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
                symbol_name(ctx, variable)?;
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
            LoopClause::Hash { .. } => HeldLoopClause::Hash,
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
        };
        clauses.push(clause);
    }
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
            HeldLoopClause::EqualsThen {
                variable,
                init,
                then,
            } => {
                bindings.push(held_list(ctx, runtime, &mut held, &[variable, init])?);
                updates.extend([variable, then]);
            }
            HeldLoopClause::In {
                variable,
                sequence,
                on,
                by,
            } => {
                let cursor = held_fresh_symbol(ctx, runtime, &mut held)?;
                let nil_index = held.len();
                held.push(Word::NIL);
                bindings.push(held_list(ctx, runtime, &mut held, &[variable, nil_index])?);
                bindings.push(held_list(ctx, runtime, &mut held, &[cursor, sequence])?);
                tests.push(held_form(ctx, runtime, &mut held, "ENDP", &[cursor])?);
                let current = if on {
                    cursor
                } else {
                    held_form(ctx, runtime, &mut held, "CAR", &[cursor])?
                };
                body.push(held_form(
                    ctx,
                    runtime,
                    &mut held,
                    "SETQ",
                    &[variable, current],
                )?);
                let next = if let Some(by) = by {
                    held_form(ctx, runtime, &mut held, "FUNCALL", &[by, cursor])?
                } else {
                    held_form(ctx, runtime, &mut held, "CDR", &[cursor])?
                };
                updates.extend([cursor, next]);
            }
            HeldLoopClause::Across { variable, vector } => {
                let index = held_fresh_symbol(ctx, runtime, &mut held)?;
                let zero = held.len();
                held.push(Word::fixnum(0));
                let nil_index = held.len();
                held.push(Word::NIL);
                bindings.push(held_list(ctx, runtime, &mut held, &[index, zero])?);
                bindings.push(held_list(ctx, runtime, &mut held, &[variable, nil_index])?);
                let length = held_form(ctx, runtime, &mut held, "ARRAY-TOTAL-SIZE", &[vector])?;
                tests.push(held_form(ctx, runtime, &mut held, ">=", &[index, length])?);
                let element = held_form(ctx, runtime, &mut held, "AREF", &[vector, index])?;
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
            HeldLoopClause::Hash => return Err(ObjectError::TypeError),
            HeldLoopClause::Repeat(count) => {
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
            HeldLoopClause::Do(ref forms) => body.extend(forms),
            HeldLoopClause::Return(value) => {
                let name = name.unwrap_or(nil);
                body.push(held_form(
                    ctx,
                    runtime,
                    &mut held,
                    "RETURN-FROM",
                    &[name, value],
                )?);
            }
            HeldLoopClause::Accumulate {
                kind,
                form: value,
                variable,
            } => {
                let accumulator = match variable {
                    Some(variable) => variable,
                    None => held_fresh_symbol(ctx, runtime, &mut held)?,
                };
                if !initialized_accumulators
                    .iter()
                    .any(|index| held.get(*index) == held.get(accumulator))
                {
                    initialized_accumulators.push(accumulator);
                    let init = match kind {
                        AccumulatorKind::Count | AccumulatorKind::Sum => Word::fixnum(0),
                        AccumulatorKind::Collect
                        | AccumulatorKind::Append
                        | AccumulatorKind::Nconc
                        | AccumulatorKind::Maximize
                        | AccumulatorKind::Minimize => Word::NIL,
                    };
                    let init_index = held.len();
                    held.push(init);
                    bindings.push(held_list(
                        ctx,
                        runtime,
                        &mut held,
                        &[accumulator, init_index],
                    )?);
                }
                result = accumulator;
                result_kind = Some(kind);
                match kind {
                    AccumulatorKind::Collect => {
                        body.push(held_form(
                            ctx,
                            runtime,
                            &mut held,
                            "PUSH",
                            &[value, accumulator],
                        )?);
                    }
                    AccumulatorKind::Append => {
                        let appended =
                            held_form(ctx, runtime, &mut held, "APPEND", &[accumulator, value])?;
                        body.push(held_form(
                            ctx,
                            runtime,
                            &mut held,
                            "SETQ",
                            &[accumulator, appended],
                        )?);
                    }
                    AccumulatorKind::Nconc => {
                        let concatenated =
                            held_form(ctx, runtime, &mut held, "NCONC", &[accumulator, value])?;
                        body.push(held_form(
                            ctx,
                            runtime,
                            &mut held,
                            "SETQ",
                            &[accumulator, concatenated],
                        )?);
                    }
                    AccumulatorKind::Count => {
                        let one = held.len();
                        held.push(Word::fixnum(1));
                        let increment =
                            held_form(ctx, runtime, &mut held, "INCF", &[accumulator, one])?;
                        body.push(held_form(
                            ctx,
                            runtime,
                            &mut held,
                            "WHEN",
                            &[value, increment],
                        )?);
                    }
                    AccumulatorKind::Sum => {
                        body.push(held_form(
                            ctx,
                            runtime,
                            &mut held,
                            "INCF",
                            &[accumulator, value],
                        )?);
                    }
                    AccumulatorKind::Maximize | AccumulatorKind::Minimize => {
                        let first = held_fresh_symbol(ctx, runtime, &mut held)?;
                        let truth = held.len();
                        held.push(Word::TRUE);
                        bindings.push(held_list(ctx, runtime, &mut held, &[first, truth])?);
                        let operator = if matches!(kind, AccumulatorKind::Maximize) {
                            "MAX"
                        } else {
                            "MIN"
                        };
                        let selected =
                            held_form(ctx, runtime, &mut held, operator, &[accumulator, value])?;
                        let selected =
                            held_form(ctx, runtime, &mut held, "SETQ", &[accumulator, selected])?;
                        let set_first =
                            held_form(ctx, runtime, &mut held, "SETQ", &[accumulator, value])?;
                        let nil = held.len();
                        held.push(Word::NIL);
                        let clear_first =
                            held_form(ctx, runtime, &mut held, "SETQ", &[first, nil])?;
                        body.push(held_form(
                            ctx,
                            runtime,
                            &mut held,
                            "IF",
                            &[first, set_first, selected],
                        )?);
                        body.push(clear_first);
                    }
                }
            }
        }
    }
    let body = expand_body(ctx, runtime, &mut held, &body, end)?;
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
    tagbody.extend(finally);
    let value = if matches!(result_kind, Some(AccumulatorKind::Collect)) {
        held_form(ctx, runtime, &mut held, "NREVERSE", &[result])?
    } else {
        result
    };
    let tagbody = held_form(ctx, runtime, &mut held, "TAGBODY", &tagbody)?;
    let mut block_body = initially;
    block_body.extend([tagbody, value]);
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
