//! Typed parser and structural expander for the ANSI CL LOOP facility.
//!
//! The registration glue intentionally lives outside this file.  The public
//! entry points here are suitable for the same adapted callback used by the
//! other macro expanders in this crate.

use crate::{elements, fresh_symbol, list, symbol};
use ncl_object::{
    BuiltinArgs, MultipleValues, ObjectError, Runtime, ThreadContext, Word, string_length,
    string_ref, symbol_name,
};

type Result<T = Word> = std::result::Result<T, ObjectError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccumulatorKind {
    Collect,
    Append,
    Nconc,
    Count,
    Sum,
    Maximize,
    Minimize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepDirection {
    From,
    UpFrom,
    DownFrom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitDirection {
    To,
    UpTo,
    Below,
    DownTo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ForClause {
    pub variable: Word,
    pub init: Word,
    pub step: Option<Word>,
    pub direction: Option<StepDirection>,
    pub limit: Option<(LimitDirection, Word)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoopClause {
    With {
        variable: Word,
        init: Word,
    },
    For(ForClause),
    Repeat(Word),
    While(Word),
    Until(Word),
    Initially(Vec<Word>),
    Finally(Vec<Word>),
    Do(Vec<Word>),
    Accumulate {
        kind: AccumulatorKind,
        form: Word,
        variable: Option<Word>,
    },
    Return(Word),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoopAst {
    pub name: Option<Word>,
    pub clauses: Vec<LoopClause>,
}

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

fn word_name(ctx: &ThreadContext, word: Word) -> Result<String> {
    let name = symbol_name(ctx, word)?;
    Ok((0..string_length(ctx, name)?)
        .map(|index| string_ref(ctx, name, index))
        .collect::<std::result::Result<String, _>>()?
        .to_ascii_uppercase())
}

fn is_keyword(ctx: &ThreadContext, word: Word) -> bool {
    word_name(ctx, word).is_ok_and(|name| {
        matches!(
            name.as_str(),
            "NAMED"
                | "WITH"
                | "FOR"
                | "AS"
                | "REPEAT"
                | "WHILE"
                | "UNTIL"
                | "INITIALLY"
                | "FINALLY"
                | "DO"
                | "COLLECT"
                | "APPEND"
                | "NCONC"
                | "COUNT"
                | "SUM"
                | "MAXIMIZE"
                | "MINIMIZE"
                | "INTO"
                | "RETURN"
                | "FROM"
                | "UPFROM"
                | "DOWNFROM"
                | "TO"
                | "UPTO"
                | "BELOW"
                | "DOWNTO"
                | "BY"
        )
    })
}

fn take_forms(ctx: &ThreadContext, input: &[Word], cursor: &mut usize) -> Vec<Word> {
    let start = *cursor;
    while *cursor < input.len() && !is_keyword(ctx, input[*cursor]) {
        *cursor += 1;
    }
    input[start..*cursor].to_vec()
}

fn required(input: &[Word], cursor: &mut usize) -> Result<Word> {
    let value = input.get(*cursor).copied().ok_or(ObjectError::TypeError)?;
    *cursor += 1;
    Ok(value)
}

fn parse_for(ctx: &ThreadContext, input: &[Word], cursor: &mut usize) -> Result<LoopClause> {
    let variable = required(input, cursor)?;
    symbol_name(ctx, variable)?;
    let mut init = Word::NIL;
    let mut step = None;
    let mut direction = None;
    let mut limit = None;
    while let Some(word) = input.get(*cursor).copied() {
        let name = word_name(ctx, word)?;
        let Some(next) = input.get(*cursor + 1).copied() else {
            return Err(ObjectError::TypeError);
        };
        match name.as_str() {
            "=" => {
                init = next;
                *cursor += 2;
            }
            "FROM" | "UPFROM" | "DOWNFROM" => {
                direction = Some(match name.as_str() {
                    "FROM" => StepDirection::From,
                    "UPFROM" => StepDirection::UpFrom,
                    _ => StepDirection::DownFrom,
                });
                init = next;
                *cursor += 2;
            }
            "THEN" | "BY" => {
                step = Some(next);
                *cursor += 2;
            }
            "TO" | "UPTO" | "BELOW" | "DOWNTO" => {
                limit = Some((
                    match name.as_str() {
                        "TO" => LimitDirection::To,
                        "UPTO" => LimitDirection::UpTo,
                        "BELOW" => LimitDirection::Below,
                        _ => LimitDirection::DownTo,
                    },
                    next,
                ));
                *cursor += 2;
            }
            _ => break,
        }
    }
    Ok(LoopClause::For(ForClause {
        variable,
        init,
        step,
        direction,
        limit,
    }))
}

/// Parse the body of a LOOP form (the operator itself is not included).
pub fn parse_loop(ctx: &ThreadContext, input: &[Word]) -> Result<LoopAst> {
    let mut cursor = 0;
    let mut name = None;
    let mut clauses = Vec::new();
    while cursor < input.len() {
        let keyword = word_name(ctx, required(input, &mut cursor)?)?;
        match keyword.as_str() {
            "NAMED" => {
                let named = required(input, &mut cursor)?;
                symbol_name(ctx, named)?;
                name = Some(named);
            }
            "WITH" => {
                let variable = required(input, &mut cursor)?;
                symbol_name(ctx, variable)?;
                let init = if input
                    .get(cursor)
                    .is_some_and(|word| word_name(ctx, *word).ok().as_deref() == Some("="))
                {
                    cursor += 1;
                    required(input, &mut cursor)?
                } else {
                    Word::NIL
                };
                clauses.push(LoopClause::With { variable, init });
            }
            "FOR" | "AS" => clauses.push(parse_for(ctx, input, &mut cursor)?),
            "REPEAT" => clauses.push(LoopClause::Repeat(required(input, &mut cursor)?)),
            "WHILE" => clauses.push(LoopClause::While(required(input, &mut cursor)?)),
            "UNTIL" => clauses.push(LoopClause::Until(required(input, &mut cursor)?)),
            "INITIALLY" => clauses.push(LoopClause::Initially(take_forms(ctx, input, &mut cursor))),
            "FINALLY" => clauses.push(LoopClause::Finally(take_forms(ctx, input, &mut cursor))),
            "DO" => clauses.push(LoopClause::Do(take_forms(ctx, input, &mut cursor))),
            "RETURN" => clauses.push(LoopClause::Return(required(input, &mut cursor)?)),
            "COLLECT" | "APPEND" | "NCONC" | "COUNT" | "SUM" | "MAXIMIZE" | "MINIMIZE" => {
                let kind = match keyword.as_str() {
                    "COLLECT" => AccumulatorKind::Collect,
                    "APPEND" => AccumulatorKind::Append,
                    "NCONC" => AccumulatorKind::Nconc,
                    "COUNT" => AccumulatorKind::Count,
                    "SUM" => AccumulatorKind::Sum,
                    "MAXIMIZE" => AccumulatorKind::Maximize,
                    _ => AccumulatorKind::Minimize,
                };
                let form = required(input, &mut cursor)?;
                let variable = if input
                    .get(cursor)
                    .is_some_and(|word| word_name(ctx, *word).ok().as_deref() == Some("INTO"))
                {
                    cursor += 1;
                    Some(required(input, &mut cursor)?)
                } else {
                    None
                };
                clauses.push(LoopClause::Accumulate {
                    kind,
                    form,
                    variable,
                });
            }
            _ => return Err(ObjectError::TypeError),
        }
    }
    Ok(LoopAst { name, clauses })
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
