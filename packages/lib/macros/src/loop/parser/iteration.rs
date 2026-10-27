use super::super::{
    ForClause, HashClause, HashIterationKind, LimitDirection, LoopClause, ObjectError,
    PackageClause, PackageIterationKind, Result, Runtime, StepDirection, ThreadContext, Word,
    elements, symbol_name,
};
use super::{required, word_name};

#[allow(clippy::too_many_lines)]
pub(super) fn parse_for(
    ctx: &ThreadContext,
    input: &[Word],
    cursor: &mut usize,
    variable: Word,
    type_spec: Option<Word>,
) -> Result<LoopClause> {
    symbol_name(ctx, variable)?;
    let mut init = Word::NIL;
    let mut has_init = false;
    let mut equals_then = false;
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
                equals_then = true;
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
    if then.is_some() || equals_then {
        if step.is_some() || direction.is_some() || limit.is_some() {
            return Err(ObjectError::TypeError);
        }
        Ok(LoopClause::EqualsThen {
            variable,
            type_spec,
            init,
            then,
        })
    } else {
        // ANSI CL: when no from-type preposition (`=`/`from`/`upfrom`/`downfrom`)
        // is present, the index starts at 0.
        if !has_init {
            init = Word::fixnum(0);
        }
        Ok(LoopClause::For(ForClause {
            variable,
            type_spec,
            has_init,
            init,
            step,
            direction,
            limit,
        }))
    }
}

pub(super) fn parse_sequence_for(
    ctx: &ThreadContext,
    input: &[Word],
    cursor: &mut usize,
    variable: Word,
    type_spec: Option<Word>,
    on: bool,
) -> Result<LoopClause> {
    if type_spec.is_some() {
        symbol_name(ctx, variable)?;
    }
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
        type_spec,
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

pub(super) fn parse_being_for(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    input: &[Word],
    cursor: &mut usize,
    variable: Word,
    type_spec: Option<Word>,
) -> Result<LoopClause> {
    let article = input
        .get(*cursor)
        .copied()
        .and_then(|word| word_name(ctx, word).ok())
        .filter(|name| name == "EACH" || name == "THE");
    let mut kind_cursor = *cursor;
    if article.is_some() {
        kind_cursor += 1;
    }
    let kind_name = word_name(ctx, required(input, &mut kind_cursor)?)?;
    let package_kind = package_iteration_kind(&kind_name);
    if package_kind.is_some() && article.is_none() {
        return Err(ObjectError::TypeError);
    }
    if let Some(kind) = package_kind {
        symbol_name(ctx, variable)?;
        *cursor = kind_cursor;
        let package = if input.get(*cursor).is_some_and(
            |word| matches!(word_name(ctx, *word), Ok(name) if name == "IN" || name == "OF"),
        ) {
            *cursor += 1;
            Some(required(input, cursor)?)
        } else {
            None
        };
        return Ok(LoopClause::Package(PackageClause {
            variable,
            type_spec,
            kind,
            package,
        }));
    }
    if hash_kind(&kind_name).is_some() {
        return parse_hash_for(ctx, input, cursor, variable, type_spec);
    }
    let article = article.as_deref().unwrap_or("");
    Err(super::unsupported_clause(
        ctx,
        runtime,
        &format!("BEING {article} {kind_name}"),
    ))
}

fn package_iteration_kind(name: &str) -> Option<PackageIterationKind> {
    if name == "SYMBOL" || name == "SYMBOLS" {
        Some(PackageIterationKind::Symbols)
    } else if name == "PRESENT-SYMBOL" || name == "PRESENT-SYMBOLS" {
        Some(PackageIterationKind::PresentSymbols)
    } else if name == "EXTERNAL-SYMBOL" || name == "EXTERNAL-SYMBOLS" {
        Some(PackageIterationKind::ExternalSymbols)
    } else {
        None
    }
}

pub(super) fn parse_hash_for(
    ctx: &mut ThreadContext,
    input: &[Word],
    cursor: &mut usize,
    variable: Word,
    type_spec: Option<Word>,
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
        type_spec,
        kind,
        table,
        using,
    }))
}
