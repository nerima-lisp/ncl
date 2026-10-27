mod clause;
mod iteration;

use super::{
    ConditionalKind, LoopAst, LoopClause, ObjectError, Result, Runtime, ThreadContext, Word,
    string_length, string_ref, symbol_name,
};
use ncl_object::LispError;
fn word_name(ctx: &ThreadContext, word: Word) -> Result<String> {
    let name = symbol_name(ctx, word)?;
    Ok((0..string_length(ctx, name)?)
        .map(|index| string_ref(ctx, name, index))
        .collect::<std::result::Result<String, _>>()?
        .to_ascii_uppercase())
}

fn unsupported_clause(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> ObjectError {
    let message = format!("Unsupported LOOP clause: {name}");
    let characters = message.chars().collect::<Vec<_>>();
    match ncl_object::make_string(ctx, runtime, &characters) {
        Ok(format_control) => {
            ctx.set_pending_lisp_error(LispError::SimpleError {
                format_control,
                format_arguments: Word::NIL,
            });
            ObjectError::TypeError
        }
        Err(error) => error,
    }
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
                | "OF-TYPE"
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

fn is_for_clause_word(name: &str) -> bool {
    matches!(
        name,
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
            | "IN"
            | "ON"
            | "ACROSS"
            | "BEING"
    )
}

fn is_for_clause(ctx: &ThreadContext, word: Word) -> bool {
    word_name(ctx, word).is_ok_and(|name| is_for_clause_word(&name))
}

fn parse_optional_type_spec(
    ctx: &ThreadContext,
    input: &[Word],
    cursor: &mut usize,
) -> Result<Option<Word>> {
    let Some(word) = input.get(*cursor).copied() else {
        return Ok(None);
    };
    let name = word_name(ctx, word).ok();
    if name.as_deref() == Some("OF-TYPE") {
        *cursor += 1;
        return required(input, cursor).map(Some);
    }
    if matches!(name.as_deref(), Some("FIXNUM" | "FLOAT" | "T" | "NIL")) {
        *cursor += 1;
        return Ok(Some(word));
    }
    Ok(None)
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
pub fn parse_loop(ctx: &mut ThreadContext, runtime: &Runtime, input: &[Word]) -> Result<LoopAst> {
    let mut cursor = 0;
    let mut name = None;
    let mut clauses = Vec::new();
    while cursor < input.len() {
        let keyword = word_name(ctx, required(input, &mut cursor)?)?;
        if keyword == "NAMED" {
            let named = required(input, &mut cursor)?;
            symbol_name(ctx, named)?;
            name = Some(named);
        } else if keyword == "WITH" {
            loop {
                let variable = required(input, &mut cursor)?;
                symbol_name(ctx, variable)?;
                let type_spec = parse_optional_type_spec(ctx, input, &mut cursor)?;
                let has_init = input
                    .get(cursor)
                    .is_some_and(|word| matches!(word_name(ctx, *word), Ok(name) if name == "="));
                let init = if has_init {
                    cursor += 1;
                    required(input, &mut cursor)?
                } else {
                    Word::NIL
                };
                clauses.push(LoopClause::With {
                    variable,
                    type_spec,
                    has_init,
                    init,
                });
                if !input
                    .get(cursor)
                    .is_some_and(|word| matches!(word_name(ctx, *word), Ok(name) if name == "AND"))
                {
                    break;
                }
                cursor += 1;
            }
        } else if keyword == "FOR" || keyword == "AS" {
            let variable = input.get(cursor).copied().ok_or(ObjectError::TypeError)?;
            let next = input
                .get(cursor + 1)
                .copied()
                .ok_or(ObjectError::TypeError)?;
            let mut clause_cursor = cursor + 1;
            let parsed_type_spec = parse_optional_type_spec(ctx, input, &mut clause_cursor)?;
            let (type_spec, clause_word, clause_index) = if is_for_clause(ctx, next) {
                (None, next, cursor + 1)
            } else if let Some(clause_word) = input
                .get(clause_cursor)
                .copied()
                .filter(|word| is_for_clause(ctx, *word))
            {
                (parsed_type_spec, clause_word, clause_cursor)
            } else {
                (None, next, cursor + 1)
            };
            match word_name(ctx, clause_word)?.as_str() {
                "IN" => {
                    cursor = clause_index + 1;
                    clauses.push(iteration::parse_sequence_for(
                        ctx,
                        input,
                        &mut cursor,
                        variable,
                        type_spec,
                        false,
                    )?);
                }
                "ON" => {
                    cursor = clause_index + 1;
                    clauses.push(iteration::parse_sequence_for(
                        ctx,
                        input,
                        &mut cursor,
                        variable,
                        type_spec,
                        true,
                    )?);
                }
                "ACROSS" => {
                    cursor = clause_index + 1;
                    clauses.push(LoopClause::Across {
                        variable,
                        type_spec,
                        vector: required(input, &mut cursor)?,
                    });
                    symbol_name(ctx, variable)?;
                }
                "BEING" => {
                    cursor = clause_index + 1;
                    clauses.push(iteration::parse_being_for(
                        ctx,
                        runtime,
                        input,
                        &mut cursor,
                        variable,
                        type_spec,
                    )?);
                }
                "=" | "FROM" | "UPFROM" | "DOWNFROM" | "BY" | "THEN" | "TO" | "UPTO" | "BELOW"
                | "DOWNTO" | "ABOVE" => {
                    cursor = clause_index;
                    clauses.push(iteration::parse_for(
                        ctx,
                        input,
                        &mut cursor,
                        variable,
                        type_spec,
                    )?);
                }
                other => return Err(unsupported_clause(ctx, runtime, &format!("FOR {other}"))),
            }
        } else if keyword == "REPEAT" {
            clauses.push(LoopClause::Repeat(required(input, &mut cursor)?));
        } else if keyword == "WHILE" {
            clauses.push(LoopClause::While(required(input, &mut cursor)?));
        } else if keyword == "UNTIL" {
            clauses.push(LoopClause::Until(required(input, &mut cursor)?));
        } else if keyword == "INITIALLY" {
            clauses.push(LoopClause::Initially(take_forms(ctx, input, &mut cursor)));
        } else if keyword == "FINALLY" {
            clauses.push(LoopClause::Finally(take_forms(ctx, input, &mut cursor)));
        } else if keyword == "DO" {
            clauses.push(LoopClause::Do(take_forms(ctx, input, &mut cursor)));
        } else if keyword == "RETURN" {
            clauses.push(LoopClause::Return(required(input, &mut cursor)?));
        } else if keyword == "ALWAYS" {
            clauses.push(LoopClause::Always(required(input, &mut cursor)?));
        } else if keyword == "NEVER" {
            clauses.push(LoopClause::Never(required(input, &mut cursor)?));
        } else if keyword == "THEREIS" {
            clauses.push(LoopClause::Thereis(required(input, &mut cursor)?));
        } else if keyword == "WHEN" {
            clauses.push(clause::parse_conditional(
                ctx,
                input,
                &mut cursor,
                ConditionalKind::When,
            )?);
        } else if keyword == "UNLESS" {
            clauses.push(clause::parse_conditional(
                ctx,
                input,
                &mut cursor,
                ConditionalKind::Unless,
            )?);
        } else if keyword == "IF" {
            clauses.push(clause::parse_conditional(
                ctx,
                input,
                &mut cursor,
                ConditionalKind::If,
            )?);
        } else if let Some(kind) = clause::accumulator_kind(&keyword) {
            clauses.push(clause::parse_accumulate(ctx, input, &mut cursor, kind)?);
        } else {
            return Err(unsupported_clause(ctx, runtime, &keyword));
        }
    }
    Ok(LoopAst { name, clauses })
}
