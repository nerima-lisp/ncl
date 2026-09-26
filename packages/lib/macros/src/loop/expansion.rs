use super::{
    AccumulatorKind, BuiltinArgs, LimitDirection, LoopAst, LoopClause, MultipleValues, ObjectError,
    Result, Runtime, StepDirection, ThreadContext, Word, elements, fresh_symbol, list, parse_loop,
    symbol,
};

#[derive(Clone, Debug)]
enum HeldLoopClause {
    With {
        variable: usize,
        init: usize,
    },
    For {
        variable: usize,
        init: usize,
        step: Option<usize>,
        direction: Option<StepDirection>,
        limit: Option<(LimitDirection, usize)>,
    },
    Repeat(usize),
    While(usize),
    Until(usize),
    Initially(Vec<usize>),
    Finally(Vec<usize>),
    Do(Vec<usize>),
    Accumulate {
        kind: AccumulatorKind,
        form: usize,
        variable: Option<usize>,
    },
    Return(usize),
}

fn form(ctx: &mut ThreadContext, runtime: &Runtime, name: &str, args: &[Word]) -> Result {
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        let mut operator = symbol(ctx, runtime, name)?;
        ncl_object::with_root(ctx, &mut operator, |ctx, operator| {
            let values = std::iter::once(*operator)
                .chain(roots.iter().map(|root| **root))
                .collect::<Vec<_>>();
            list(ctx, runtime, &values)
        })
    })
}

fn held_get(held: &[Word], index: usize) -> Result<Word> {
    held.get(index).copied().ok_or(ObjectError::TypeError)
}

fn held_form(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    name: &str,
    indexes: &[usize],
) -> Result<usize> {
    let (value, refreshed) = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let args = indexes
            .iter()
            .map(|index| {
                roots
                    .get(*index)
                    .map(|root| **root)
                    .ok_or(ObjectError::TypeError)
            })
            .collect::<Result<Vec<_>>>()?;
        let value = form(ctx, runtime, name, &args)?;
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((value, refreshed))
    })?;
    *held = refreshed;
    held.push(value);
    Ok(held.len() - 1)
}

fn held_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    indexes: &[usize],
) -> Result<usize> {
    let (value, refreshed) = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let values = indexes
            .iter()
            .map(|index| {
                roots
                    .get(*index)
                    .map(|root| **root)
                    .ok_or(ObjectError::TypeError)
            })
            .collect::<Result<Vec<_>>>()?;
        let value = list(ctx, runtime, &values)?;
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((value, refreshed))
    })?;
    *held = refreshed;
    held.push(value);
    Ok(held.len() - 1)
}

fn held_fresh_symbol(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
) -> Result<usize> {
    let (value, refreshed) = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let value = fresh_symbol(ctx, runtime)?;
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((value, refreshed))
    })?;
    *held = refreshed;
    held.push(value);
    Ok(held.len() - 1)
}

fn expand_body(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    body: &[usize],
    end: usize,
) -> Result<Vec<usize>> {
    let finish = held_fresh_symbol(ctx, runtime, held)?;
    let mut result = Vec::with_capacity(body.len());
    for index in body {
        let word = held_get(held, *index)?;
        if word.is_cons() {
            let parts = elements(ctx, word)?;
            if parts.first().copied() == Some(held_get(held, finish)?) && parts.len() == 1 {
                result.push(held_form(ctx, runtime, held, "GO", &[end])?);
                continue;
            }
        }
        result.push(*index);
    }
    Ok(result)
}

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
    let mut accumulators = Vec::new();

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
                {
                    let operator = if matches!(direction, Some(StepDirection::DownFrom)) {
                        "-"
                    } else {
                        "+"
                    };
                    let update = held_form(ctx, runtime, &mut held, operator, &[variable, step])?;
                    updates.extend([variable, update]);
                }
                if let Some((limit_direction, limit)) = limit {
                    let operator = match limit_direction {
                        LimitDirection::To => ">",
                        LimitDirection::UpTo | LimitDirection::Below => ">=",
                        LimitDirection::DownTo => "<",
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
                {
                    accumulators.push((kind, accumulator));
                    let init = if matches!(kind, AccumulatorKind::Count) {
                        Word::fixnum(0)
                    } else {
                        Word::NIL
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
                        let values = held_form(ctx, runtime, &mut held, "LIST", &[value])?;
                        let appended =
                            held_form(ctx, runtime, &mut held, "APPEND", &[accumulator, values])?;
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
                        body.push(held_form(
                            ctx,
                            runtime,
                            &mut held,
                            "INCF",
                            &[accumulator, one],
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
                        let operator = if matches!(kind, AccumulatorKind::Maximize) {
                            "MAX"
                        } else {
                            "MIN"
                        };
                        let selected =
                            held_form(ctx, runtime, &mut held, operator, &[accumulator, value])?;
                        body.push(held_form(
                            ctx,
                            runtime,
                            &mut held,
                            "SETQ",
                            &[accumulator, selected],
                        )?);
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
    let mut value = result;
    for (kind, variable) in accumulators.into_iter().rev() {
        if matches!(kind, AccumulatorKind::Collect) {
            value = held_form(ctx, runtime, &mut held, "NREVERSE", &[variable])?;
        } else {
            value = variable;
        }
    }
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

/// Adapted callback for a LOOP macro function.
pub fn expand_loop_callback(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result {
    let form = args.get(0).ok_or(ObjectError::TypeError)?;
    let input = elements(ctx, form)?;
    let input = input.get(1..).ok_or(ObjectError::TypeError)?;
    let ast = parse_loop(ctx, input)?;
    expand_loop_ast(ctx, runtime, &ast)
}
