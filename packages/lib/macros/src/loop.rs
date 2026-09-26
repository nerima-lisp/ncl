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

fn progn(ctx: &mut ThreadContext, runtime: &Runtime, body: &[Word]) -> Result {
    form(ctx, runtime, "PROGN", body)
}

fn binding(ctx: &mut ThreadContext, runtime: &Runtime, variable: Word, value: Word) -> Result {
    list(ctx, runtime, &[variable, value])
}

fn expand_body(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    body: &[Word],
    end: Word,
) -> Result<Vec<Word>> {
    let finish = symbol(ctx, runtime, "LOOP-FINISH")?;
    let mut result = Vec::with_capacity(body.len());
    for word in body {
        if word.is_cons() {
            let parts = elements(ctx, *word)?;
            if parts.first().copied() == Some(finish) && parts.len() == 1 {
                result.push(form(ctx, runtime, "GO", &[end])?);
                continue;
            }
        }
        result.push(*word);
    }
    Ok(result)
}

/// Expand a parsed LOOP AST into portable CL primitive forms.
#[allow(clippy::too_many_lines)]
pub fn expand_loop_ast(ctx: &mut ThreadContext, runtime: &Runtime, ast: &LoopAst) -> Result {
    let end = fresh_symbol(ctx, runtime)?;
    let start = fresh_symbol(ctx, runtime)?;
    let mut bindings = Vec::new();
    let mut updates = Vec::new();
    let mut tests = Vec::new();
    let mut body = Vec::new();
    let mut initially = Vec::new();
    let mut finally: Vec<Word> = Vec::new();
    let mut result = Word::NIL;
    let mut accumulators = Vec::new();

    for clause in &ast.clauses {
        match *clause {
            LoopClause::With { variable, init } => {
                bindings.push(binding(ctx, runtime, variable, init)?);
            }
            LoopClause::For(spec) => {
                bindings.push(binding(ctx, runtime, spec.variable, spec.init)?);
                let step = spec.step.unwrap_or_else(|| {
                    if matches!(spec.direction, Some(StepDirection::DownFrom)) {
                        Word::fixnum(-1)
                    } else {
                        Word::fixnum(1)
                    }
                });
                {
                    let operator = if matches!(spec.direction, Some(StepDirection::DownFrom)) {
                        "-"
                    } else {
                        "+"
                    };
                    let update = form(ctx, runtime, operator, &[spec.variable, step])?;
                    updates.extend([spec.variable, update]);
                }
                if let Some((direction, limit)) = spec.limit {
                    let operator = match direction {
                        LimitDirection::To => ">",
                        LimitDirection::UpTo | LimitDirection::Below => ">=",
                        LimitDirection::DownTo => "<",
                    };
                    tests.push(form(ctx, runtime, operator, &[spec.variable, limit])?);
                }
            }
            LoopClause::Repeat(count) => {
                let counter = fresh_symbol(ctx, runtime)?;
                bindings.push(binding(ctx, runtime, counter, count)?);
                tests.push(form(ctx, runtime, "<=", &[counter, Word::fixnum(0)])?);
                updates.extend([
                    counter,
                    form(ctx, runtime, "-", &[counter, Word::fixnum(1)])?,
                ]);
            }
            LoopClause::While(test) => tests.push(form(ctx, runtime, "NOT", &[test])?),
            LoopClause::Until(test) => tests.push(test),
            LoopClause::Initially(ref forms) => initially.extend(forms),
            LoopClause::Finally(ref forms) => finally.extend(forms),
            LoopClause::Do(ref forms) => body.extend(forms),
            LoopClause::Return(value) => {
                body.push(form(
                    ctx,
                    runtime,
                    "RETURN-FROM",
                    &[ast.name.unwrap_or(Word::NIL), value],
                )?);
            }
            LoopClause::Accumulate {
                kind,
                form: value,
                variable,
            } => {
                let accumulator = variable.unwrap_or(fresh_symbol(ctx, runtime)?);
                {
                    accumulators.push((kind, accumulator));
                    let init = if matches!(kind, AccumulatorKind::Count) {
                        Word::fixnum(0)
                    } else {
                        Word::NIL
                    };
                    bindings.push(binding(ctx, runtime, accumulator, init)?);
                }
                result = accumulator;
                match kind {
                    AccumulatorKind::Collect => {
                        body.push(form(ctx, runtime, "PUSH", &[value, accumulator])?);
                    }
                    AccumulatorKind::Append => {
                        let values = form(ctx, runtime, "LIST", &[value])?;
                        let appended = form(ctx, runtime, "APPEND", &[accumulator, values])?;
                        body.push(form(ctx, runtime, "SETQ", &[accumulator, appended])?);
                    }
                    AccumulatorKind::Nconc => {
                        let concatenated = form(ctx, runtime, "NCONC", &[accumulator, value])?;
                        body.push(form(ctx, runtime, "SETQ", &[accumulator, concatenated])?);
                    }
                    AccumulatorKind::Count => {
                        body.push(form(ctx, runtime, "INCF", &[accumulator, Word::fixnum(1)])?);
                    }
                    AccumulatorKind::Sum => {
                        body.push(form(ctx, runtime, "INCF", &[accumulator, value])?);
                    }
                    AccumulatorKind::Maximize | AccumulatorKind::Minimize => {
                        let operator = if matches!(kind, AccumulatorKind::Maximize) {
                            "MAX"
                        } else {
                            "MIN"
                        };
                        let selected = form(ctx, runtime, operator, &[accumulator, value])?;
                        body.push(form(ctx, runtime, "SETQ", &[accumulator, selected])?);
                    }
                }
            }
        }
    }
    let body = expand_body(ctx, runtime, &body, end)?;
    let stop = if tests.is_empty() {
        Word::NIL
    } else {
        let test = form(ctx, runtime, "OR", &tests)?;
        let go_end = form(ctx, runtime, "GO", &[end])?;
        form(ctx, runtime, "WHEN", &[test, go_end])?
    };
    let mut tagbody = vec![start, stop];
    tagbody.extend(body);
    if !updates.is_empty() {
        tagbody.push(form(ctx, runtime, "SETQ", &updates)?);
    }
    tagbody.push(form(ctx, runtime, "GO", &[start])?);
    tagbody.push(end);
    tagbody.extend(finally);
    let mut value = result;
    for (kind, variable) in accumulators.into_iter().rev() {
        if matches!(kind, AccumulatorKind::Collect) {
            value = form(ctx, runtime, "NREVERSE", &[variable])?;
        } else {
            value = variable;
        }
    }
    let tagbody = form(ctx, runtime, "TAGBODY", &tagbody)?;
    let mut block_body = initially;
    block_body.extend([tagbody, value]);
    let block_progn = progn(ctx, runtime, &block_body)?;
    let block = form(
        ctx,
        runtime,
        "BLOCK",
        &[ast.name.unwrap_or(Word::NIL), block_progn],
    )?;
    let binding_list = list(ctx, runtime, &bindings)?;
    form(ctx, runtime, "LET", &[binding_list, block])
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
