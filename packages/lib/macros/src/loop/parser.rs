use super::{
    AccumulatorKind, ConditionalKind, ForClause, HashClause, HashIterationKind, LimitDirection,
    LoopAst, LoopClause, ObjectError, Result, StepDirection, ThreadContext, Word, elements,
    string_length, string_ref, symbol_name,
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
                | "THEREIS"
                | "ALWAYS"
                | "NEVER"
                | "FROM"
                | "UPFROM"
                | "DOWNFROM"
                | "TO"
                | "UPTO"
                | "BELOW"
                | "DOWNTO"
                | "ABOVE"
                | "BY"
                | "THEN"
                | "IN"
                | "ON"
                | "ACROSS"
                | "BEING"
                | "THE"
                | "EACH"
                | "HASH-KEY"
                | "HASH-KEYS"
                | "HASH-VALUE"
                | "HASH-VALUES"
                | "OF"
                | "USING"
                | "WHEN"
                | "UNLESS"
                | "IF"
                | "AND"
                | "ELSE"
                | "END"
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

#[allow(clippy::too_many_lines)]
fn parse_for(ctx: &ThreadContext, input: &[Word], cursor: &mut usize) -> Result<LoopClause> {
    let variable = required(input, cursor)?;
    symbol_name(ctx, variable)?;
    let mut init = Word::NIL;
    let mut has_init = false;
    let mut has_equals = false;
    let mut step = None;
    let mut then = None;
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
                has_init = true;
                has_equals = true;
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
                has_init = true;
                *cursor += 2;
            }
            "THEN" => {
                if then.is_some() {
                    return Err(ObjectError::TypeError);
                }
                then = Some(next);
                *cursor += 2;
            }
            "BY" => {
                if step.is_some() {
                    return Err(ObjectError::TypeError);
                }
                step = Some(next);
                *cursor += 2;
            }
            "TO" | "UPTO" | "BELOW" | "DOWNTO" | "ABOVE" => {
                limit = Some((
                    if name == "TO" {
                        LimitDirection::To
                    } else if name == "UPTO" {
                        LimitDirection::UpTo
                    } else if name == "BELOW" {
                        LimitDirection::Below
                    } else if name == "DOWNTO" {
                        LimitDirection::DownTo
                    } else if name == "ABOVE" {
                        LimitDirection::Above
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
                        | "ABOVE"
                ) =>
            {
                break;
            }
            _other => return Err(ObjectError::TypeError),
        }
    }
    if let Some(then) = then {
        if step.is_some() || direction.is_some() || limit.is_some() {
            return Err(ObjectError::TypeError);
        }
        Ok(LoopClause::EqualsThen {
            variable,
            init,
            then,
        })
    } else if has_equals && step.is_none() && direction.is_none() && limit.is_none() {
        Ok(LoopClause::Equals { variable, init })
    } else {
        // ANSI CL: when no from-type preposition (`=`/`from`/`upfrom`/`downfrom`)
        // is present, the index starts at 0.
        if !has_init {
            init = Word::fixnum(0);
        }
        Ok(LoopClause::For(ForClause {
            variable,
            init,
            step,
            direction,
            limit,
        }))
    }
}
fn parse_sequence_for(
    ctx: &ThreadContext,
    input: &[Word],
    cursor: &mut usize,
    variable: Word,
    on: bool,
) -> Result<LoopClause> {
    let sequence = required(input, cursor)?;
    let by = if input
        .get(*cursor)
        .is_some_and(|word| matches!(word_name(ctx, *word), Ok(name) if name == "BY"))
    {
        *cursor += 1;
        Some(required(input, cursor)?)
    } else {
        None
    };
    Ok(LoopClause::In {
        variable,
        sequence,
        on,
        by,
    })
}

fn hash_kind(name: &str) -> Option<HashIterationKind> {
    if name == "HASH-KEY" || name == "HASH-KEYS" {
        Some(HashIterationKind::Key)
    } else if name == "HASH-VALUE" || name == "HASH-VALUES" {
        Some(HashIterationKind::Value)
    } else {
        None
    }
}

fn parse_hash_for(
    ctx: &mut ThreadContext,
    input: &[Word],
    cursor: &mut usize,
    variable: Word,
) -> Result<LoopClause> {
    symbol_name(ctx, variable)?;
    if input.get(*cursor).is_some_and(
        |word| matches!(word_name(ctx, *word), Ok(name) if name == "EACH" || name == "THE"),
    ) {
        *cursor += 1;
    }
    let kind_name = word_name(ctx, required(input, cursor)?)?;
    let kind = hash_kind(&kind_name).ok_or(ObjectError::TypeError)?;
    let of = word_name(ctx, required(input, cursor)?)?;
    if of != "OF" {
        return Err(ObjectError::TypeError);
    }
    let table = required(input, cursor)?;
    let using = if input
        .get(*cursor)
        .is_some_and(|word| matches!(word_name(ctx, *word), Ok(name) if name == "USING"))
    {
        *cursor += 1;
        let specification = elements(ctx, required(input, cursor)?)?;
        if specification.len() != 2 {
            return Err(ObjectError::TypeError);
        }
        let using_name = word_name(
            ctx,
            specification
                .first()
                .copied()
                .ok_or(ObjectError::TypeError)?,
        )?;
        let using_kind = hash_kind(&using_name).ok_or(ObjectError::TypeError)?;
        let using_variable = specification
            .get(1)
            .copied()
            .ok_or(ObjectError::TypeError)?;
        symbol_name(ctx, using_variable)?;
        if using_kind == kind {
            return Err(ObjectError::TypeError);
        }
        Some((using_kind, using_variable))
    } else {
        None
    };
    Ok(LoopClause::Hash(HashClause {
        variable,
        kind,
        table,
        using,
    }))
}

fn accumulator_kind(keyword: &str) -> Option<AccumulatorKind> {
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

fn parse_accumulate(
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
fn parse_conditional(
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
/// Parse the body of a LOOP form (the operator itself is not included).
#[allow(clippy::too_many_lines)]
pub fn parse_loop(ctx: &mut ThreadContext, input: &[Word]) -> Result<LoopAst> {
    let mut cursor = 0;
    let mut name = None;
    let mut clauses = Vec::new();
    while cursor < input.len() {
        if !is_keyword(ctx, input[cursor]) && input[cursor].is_cons() {
            clauses.push(LoopClause::Do(input[cursor..].to_vec()));
            break;
        }
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
                    .is_some_and(|word| matches!(word_name(ctx, *word), Ok(name) if name == "="))
                {
                    cursor += 1;
                    required(input, &mut cursor)?
                } else {
                    Word::NIL
                };
                clauses.push(LoopClause::With { variable, init });
            }
            "FOR" | "AS" => {
                let variable = input.get(cursor).copied().ok_or(ObjectError::TypeError)?;
                let next = input
                    .get(cursor + 1)
                    .copied()
                    .ok_or(ObjectError::TypeError)?;
                match word_name(ctx, next)?.as_str() {
                    "IN" => {
                        cursor += 2;
                        clauses.push(parse_sequence_for(
                            ctx,
                            input,
                            &mut cursor,
                            variable,
                            false,
                        )?);
                    }
                    "ON" => {
                        cursor += 2;
                        clauses.push(parse_sequence_for(ctx, input, &mut cursor, variable, true)?);
                    }
                    "ACROSS" => {
                        cursor += 2;
                        clauses.push(LoopClause::Across {
                            variable,
                            vector: required(input, &mut cursor)?,
                        });
                        symbol_name(ctx, variable)?;
                    }
                    "BEING" => {
                        cursor += 2;
                        clauses.push(parse_hash_for(ctx, input, &mut cursor, variable)?);
                    }
                    "=" | "FROM" | "UPFROM" | "DOWNFROM" | "BY" | "THEN" | "TO" | "UPTO"
                    | "BELOW" | "DOWNTO" | "ABOVE" => {
                        clauses.push(parse_for(ctx, input, &mut cursor)?);
                    }
                    _other => return Err(ObjectError::TypeError),
                }
            }
            "REPEAT" => clauses.push(LoopClause::Repeat(required(input, &mut cursor)?)),
            "WHILE" => clauses.push(LoopClause::While(required(input, &mut cursor)?)),
            "UNTIL" => clauses.push(LoopClause::Until(required(input, &mut cursor)?)),
            "THEREIS" => {
                let test = required(input, &mut cursor)?;
                clauses.push(LoopClause::Conditional {
                    kind: ConditionalKind::When,
                    test,
                    then: vec![LoopClause::Return(test)],
                    otherwise: Vec::new(),
                });
            }
            "ALWAYS" => clauses.push(LoopClause::Always(required(input, &mut cursor)?)),
            "NEVER" => clauses.push(LoopClause::Never(required(input, &mut cursor)?)),
            "INITIALLY" => clauses.push(LoopClause::Initially(take_forms(ctx, input, &mut cursor))),
            "FINALLY" => clauses.push(LoopClause::Finally(take_forms(ctx, input, &mut cursor))),
            "DO" => clauses.push(LoopClause::Do(take_forms(ctx, input, &mut cursor))),
            "RETURN" => clauses.push(LoopClause::Return(required(input, &mut cursor)?)),
            "WHEN" => clauses.push(parse_conditional(
                ctx,
                input,
                &mut cursor,
                ConditionalKind::When,
            )?),
            "UNLESS" => {
                clauses.push(parse_conditional(
                    ctx,
                    input,
                    &mut cursor,
                    ConditionalKind::Unless,
                )?);
            }
            "IF" => clauses.push(parse_conditional(
                ctx,
                input,
                &mut cursor,
                ConditionalKind::If,
            )?),
            _ if accumulator_kind(&keyword).is_some() => {
                let kind = accumulator_kind(&keyword).ok_or(ObjectError::TypeError)?;
                clauses.push(parse_accumulate(ctx, input, &mut cursor, kind)?);
            }
            _keyword => return Err(ObjectError::TypeError),
        }
    }
    Ok(LoopAst { name, clauses })
}
