use super::{
    AccumulatorKind, ForClause, LimitDirection, LoopAst, LoopClause, ObjectError, Result,
    StepDirection, ThreadContext, Word, string_length, string_ref, symbol_name,
};
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
    while let Some(word) = input.get(*cursor) {
        if is_keyword(ctx, *word) {
            break;
        }
        *cursor += 1;
    }
    input
        .get(start..*cursor)
        .map_or_else(Vec::new, ToOwned::to_owned)
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
                direction = Some(if name == "FROM" {
                    StepDirection::From
                } else if name == "UPFROM" {
                    StepDirection::UpFrom
                } else if name == "DOWNFROM" {
                    StepDirection::DownFrom
                } else {
                    return Err(ObjectError::TypeError);
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
                    if name == "TO" {
                        LimitDirection::To
                    } else if name == "UPTO" {
                        LimitDirection::UpTo
                    } else if name == "BELOW" {
                        LimitDirection::Below
                    } else if name == "DOWNTO" {
                        LimitDirection::DownTo
                    } else {
                        return Err(ObjectError::TypeError);
                    },
                    next,
                ));
                *cursor += 2;
            }
            other
                if !matches!(
                    other,
                    "=" | "FROM"
                        | "UPFROM"
                        | "DOWNFROM"
                        | "THEN"
                        | "BY"
                        | "TO"
                        | "UPTO"
                        | "BELOW"
                        | "DOWNTO"
                ) =>
            {
                break;
            }
            _other => return Err(ObjectError::TypeError),
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
                let kind = if keyword == "COLLECT" {
                    AccumulatorKind::Collect
                } else if keyword == "APPEND" {
                    AccumulatorKind::Append
                } else if keyword == "NCONC" {
                    AccumulatorKind::Nconc
                } else if keyword == "COUNT" {
                    AccumulatorKind::Count
                } else if keyword == "SUM" {
                    AccumulatorKind::Sum
                } else if keyword == "MAXIMIZE" {
                    AccumulatorKind::Maximize
                } else if keyword == "MINIMIZE" {
                    AccumulatorKind::Minimize
                } else {
                    return Err(ObjectError::TypeError);
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
            _keyword => return Err(ObjectError::TypeError),
        }
    }
    Ok(LoopAst { name, clauses })
}
