use super::super::{
    AccumulatorKind, ConditionalKind, LoopClause, ObjectError, Result, ThreadContext, Word,
    symbol_name,
};
use super::{required, take_forms, word_name};

pub(super) fn accumulator_kind(keyword: &str) -> Option<AccumulatorKind> {
    match keyword {
        "COLLECT" => Some(AccumulatorKind::Collect),
        "APPEND" => Some(AccumulatorKind::Append),
        "NCONC" => Some(AccumulatorKind::Nconc),
        "COUNT" => Some(AccumulatorKind::Count),
        "SUM" => Some(AccumulatorKind::Sum),
        "MAXIMIZE" => Some(AccumulatorKind::Maximize),
        "MINIMIZE" => Some(AccumulatorKind::Minimize),
        // check-added-lines: allow(wildcard) any other keyword is not an accumulator.
        _ => None,
    }
}

pub(super) fn parse_accumulate(
    ctx: &ThreadContext,
    input: &[Word],
    cursor: &mut usize,
    kind: AccumulatorKind,
) -> Result<LoopClause> {
    let form = required(input, cursor)?;
    let variable = if input
        .get(*cursor)
        .is_some_and(|word| matches!(word_name(ctx, *word), Ok(name) if name == "INTO"))
    {
        *cursor += 1;
        Some(required(input, cursor)?)
    } else {
        None
    };
    if let Some(variable) = variable {
        symbol_name(ctx, variable)?;
    }
    Ok(LoopClause::Accumulate {
        kind,
        form,
        variable,
    })
}

/// Parse one of the clause kinds allowed inside a `when`/`unless`/`if`
/// conditional's `selectable-clause` list: `do`, `return`, an accumulation
/// clause, or a nested conditional.
fn parse_selectable(
    ctx: &mut ThreadContext,
    input: &[Word],
    cursor: &mut usize,
) -> Result<LoopClause> {
    let keyword = word_name(ctx, required(input, cursor)?)?;
    if let Some(kind) = accumulator_kind(&keyword) {
        return parse_accumulate(ctx, input, cursor, kind);
    }
    match keyword.as_str() {
        "DO" => Ok(LoopClause::Do(take_forms(ctx, input, cursor))),
        "RETURN" => Ok(LoopClause::Return(required(input, cursor)?)),
        "WHEN" => parse_conditional(ctx, input, cursor, ConditionalKind::When),
        "UNLESS" => parse_conditional(ctx, input, cursor, ConditionalKind::Unless),
        "IF" => parse_conditional(ctx, input, cursor, ConditionalKind::If),
        _other => Err(ObjectError::TypeError),
    }
}

fn at_keyword(ctx: &ThreadContext, input: &[Word], cursor: usize, keyword: &str) -> bool {
    input
        .get(cursor)
        .is_some_and(|word| matches!(word_name(ctx, *word), Ok(name) if name == keyword))
}

/// Parse `test-form selectable-clause+ [else selectable-clause+] [end]`,
/// with `cursor` positioned right after the `when`/`unless`/`if` keyword.
pub(super) fn parse_conditional(
    ctx: &mut ThreadContext,
    input: &[Word],
    cursor: &mut usize,
    kind: ConditionalKind,
) -> Result<LoopClause> {
    let test = required(input, cursor)?;
    let mut then = vec![parse_selectable(ctx, input, cursor)?];
    while at_keyword(ctx, input, *cursor, "AND") {
        *cursor += 1;
        then.push(parse_selectable(ctx, input, cursor)?);
    }
    let mut otherwise = Vec::new();
    if at_keyword(ctx, input, *cursor, "ELSE") {
        *cursor += 1;
        otherwise.push(parse_selectable(ctx, input, cursor)?);
        while at_keyword(ctx, input, *cursor, "AND") {
            *cursor += 1;
            otherwise.push(parse_selectable(ctx, input, cursor)?);
        }
    }
    if at_keyword(ctx, input, *cursor, "END") {
        *cursor += 1;
    }
    Ok(LoopClause::Conditional {
        kind,
        test,
        then,
        otherwise,
    })
}
