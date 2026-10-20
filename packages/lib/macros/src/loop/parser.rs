mod clause;
mod iteration;

use super::{
    ConditionalKind, LoopAst, LoopClause, ObjectError, Result, ThreadContext, Word, string_length,
    string_ref, symbol_name,
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
                | "ALWAYS"
                | "NEVER"
                | "THEREIS"
                | "INTO"
                | "RETURN"
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

/// Parse the body of a LOOP form (the operator itself is not included).
#[allow(clippy::too_many_lines)]
pub fn parse_loop(ctx: &mut ThreadContext, input: &[Word]) -> Result<LoopAst> {
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
                        clauses.push(iteration::parse_sequence_for(
                            ctx,
                            input,
                            &mut cursor,
                            variable,
                            false,
                        )?);
                    }
                    "ON" => {
                        cursor += 2;
                        clauses.push(iteration::parse_sequence_for(
                            ctx,
                            input,
                            &mut cursor,
                            variable,
                            true,
                        )?);
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
                        clauses.push(iteration::parse_hash_for(
                            ctx,
                            input,
                            &mut cursor,
                            variable,
                        )?);
                    }
                    "=" | "FROM" | "UPFROM" | "DOWNFROM" | "BY" | "THEN" | "TO" | "UPTO"
                    | "BELOW" | "DOWNTO" | "ABOVE" => {
                        clauses.push(iteration::parse_for(ctx, input, &mut cursor)?);
                    }
                    _other => return Err(ObjectError::TypeError),
                }
            }
            "REPEAT" => clauses.push(LoopClause::Repeat(required(input, &mut cursor)?)),
            "WHILE" => clauses.push(LoopClause::While(required(input, &mut cursor)?)),
            "UNTIL" => clauses.push(LoopClause::Until(required(input, &mut cursor)?)),
            "INITIALLY" => clauses.push(LoopClause::Initially(take_forms(ctx, input, &mut cursor))),
            "FINALLY" => clauses.push(LoopClause::Finally(take_forms(ctx, input, &mut cursor))),
            "DO" => clauses.push(LoopClause::Do(take_forms(ctx, input, &mut cursor))),
            "RETURN" => clauses.push(LoopClause::Return(required(input, &mut cursor)?)),
            "ALWAYS" => clauses.push(LoopClause::Always(required(input, &mut cursor)?)),
            "NEVER" => clauses.push(LoopClause::Never(required(input, &mut cursor)?)),
            "THEREIS" => clauses.push(LoopClause::Thereis(required(input, &mut cursor)?)),
            "WHEN" => clauses.push(clause::parse_conditional(
                ctx,
                input,
                &mut cursor,
                ConditionalKind::When,
            )?),
            "UNLESS" => {
                clauses.push(clause::parse_conditional(
                    ctx,
                    input,
                    &mut cursor,
                    ConditionalKind::Unless,
                )?);
            }
            "IF" => clauses.push(clause::parse_conditional(
                ctx,
                input,
                &mut cursor,
                ConditionalKind::If,
            )?),
            _ if clause::accumulator_kind(&keyword).is_some() => {
                let kind = clause::accumulator_kind(&keyword).ok_or(ObjectError::TypeError)?;
                clauses.push(clause::parse_accumulate(ctx, input, &mut cursor, kind)?);
            }
            _keyword => return Err(ObjectError::TypeError),
        }
    }
    Ok(LoopAst { name, clauses })
}
