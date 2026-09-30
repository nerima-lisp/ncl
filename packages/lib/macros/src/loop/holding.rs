//! Convert a parsed [`LoopClause`] AST into [`HeldLoopClause`]s, pushing every
//! `Word` payload onto a `held` vector so the rest of expansion only deals in
//! GC-safe indices.

use super::clause::HeldLoopClause;
use super::{LoopClause, Result, ThreadContext, Word, symbol_name};

/// Convert one parsed [`LoopClause`] into a [`HeldLoopClause`]. Recurses for
/// the selectable clauses nested inside a `when`/`unless`/`if` conditional
/// clause.
#[allow(clippy::too_many_lines)]
fn hold_clause(
    ctx: &mut ThreadContext,
    held: &mut Vec<Word>,
    clause: &LoopClause,
) -> Result<HeldLoopClause> {
    Ok(match *clause {
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
        LoopClause::Hash(spec) => {
            symbol_name(ctx, spec.variable)?;
            let variable = held.len();
            held.push(spec.variable);
            let table = held.len();
            held.push(spec.table);
            let using = spec.using.map(|(kind, variable)| {
                let index = held.len();
                held.push(variable);
                (kind, index)
            });
            HeldLoopClause::Hash {
                variable,
                kind: spec.kind,
                table,
                using,
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
            // `variable` may be a destructuring pattern rather than a plain
            // symbol; leaf symbols are validated once the pattern is walked
            // during body expansion.
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
        LoopClause::Conditional {
            kind,
            test,
            ref then,
            ref otherwise,
        } => {
            let test_index = held.len();
            held.push(test);
            let then = hold_clauses(ctx, held, then)?;
            let otherwise = hold_clauses(ctx, held, otherwise)?;
            HeldLoopClause::Conditional {
                kind,
                test: test_index,
                then,
                otherwise,
            }
        }
    })
}

/// Convert every top-level or nested-conditional clause in `clauses`.
pub(super) fn hold_clauses(
    ctx: &mut ThreadContext,
    held: &mut Vec<Word>,
    clauses: &[LoopClause],
) -> Result<Vec<HeldLoopClause>> {
    let mut result = Vec::with_capacity(clauses.len());
    for clause in clauses {
        result.push(hold_clause(ctx, held, clause)?);
    }
    Ok(result)
}
