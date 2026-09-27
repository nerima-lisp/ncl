//! Structural expanders for the control macros.
#![allow(clippy::redundant_pub_crate)]

use crate::{elements, fresh_symbol, list, symbol};
use ncl_object::{
    BuiltinArgs, Handle, Local, MultipleValues, ObjectError, Runtime, Scope, ThreadContext, Word,
    symbol_name,
};

mod handler_bind;

mod multiple_value_bind;
mod multiple_values;

pub(crate) use handler_bind::{binding, bindings};
pub(crate) use multiple_values::{
    expand_multiple_value_bind_adapter, expand_multiple_value_list_adapter,
};

type Result<T = Word> = std::result::Result<T, ObjectError>;

pub(crate) fn form(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    args: &[Word],
) -> Result {
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        let operator = symbol(ctx, runtime, name)?;
        let mut values = Vec::with_capacity(roots.len() + 1);
        values.push(operator);
        values.extend(roots.iter().map(|value| **value));
        list(ctx, runtime, &values)
    })
}

fn args(ctx: &mut ThreadContext, form: Word) -> Result<Vec<Word>> {
    let mut values = elements(ctx, form)?;
    if values.is_empty() {
        return Err(ObjectError::TypeError);
    }
    values.remove(0);
    Ok(values)
}

pub(crate) fn progn(ctx: &mut ThreadContext, runtime: &Runtime, body: &[Word]) -> Result {
    form(ctx, runtime, "PROGN", body)
}

fn and(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        let values = roots.iter().map(|value| **value).collect::<Vec<_>>();
        match values.as_slice() {
            [] => Ok(Word::TRUE),
            [value] => Ok(*value),
            [value, rest @ ..] => {
                let next = and(ctx, runtime, rest)?;
                form(ctx, runtime, "IF", &[*value, next, Word::NIL])
            }
        }
    })
}

fn or(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        let values = roots.iter().map(|value| **value).collect::<Vec<_>>();
        match values.as_slice() {
            [] => Ok(Word::NIL),
            [value] => Ok(*value),
            [value, rest @ ..] => {
                let mut temporary = fresh_symbol(ctx, runtime)?;
                ncl_object::with_root(ctx, &mut temporary, |ctx, temporary| {
                    let let_bindings = bindings(ctx, runtime, &[(*temporary, *value)])?;
                    let next = or(ctx, runtime, rest)?;
                    let test = form(ctx, runtime, "IF", &[*temporary, *temporary, next])?;
                    form(ctx, runtime, "LET", &[let_bindings, test])
                })
            }
        }
    })
}

fn cond(ctx: &mut ThreadContext, runtime: &Runtime, clauses: &[Word]) -> Result {
    let Some(clause) = clauses.first().copied() else {
        return Ok(Word::NIL);
    };
    let values = elements(ctx, clause)?;
    let test = values.first().copied().ok_or(ObjectError::TypeError)?;
    let yes = if values.len() == 1 {
        test
    } else {
        progn(ctx, runtime, values.get(1..).ok_or(ObjectError::TypeError)?)?
    };
    let no = cond(
        ctx,
        runtime,
        clauses.get(1..).ok_or(ObjectError::TypeError)?,
    )?;
    form(ctx, runtime, "IF", &[test, yes, no])
}

fn case(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        let value = roots.first().copied().ok_or(ObjectError::TypeError)?;
        let mut temporary = fresh_symbol(ctx, runtime)?;
        ncl_object::with_root(ctx, &mut temporary, |ctx, temporary| {
            let mut let_bindings = bindings(ctx, runtime, &[(*temporary, *value)])?;
            ncl_object::with_root(ctx, &mut let_bindings, |ctx, let_bindings| {
                let mut branches = Word::NIL;
                for clause in roots.get(1..).ok_or(ObjectError::TypeError)?.iter().rev() {
                    let parts = elements(ctx, **clause)?;
                    ncl_object::with_roots(ctx, &parts, |ctx, parts| {
                        let keys = parts.first().copied().ok_or(ObjectError::TypeError)?;
                        let body_values = parts
                            .get(1..)
                            .ok_or(ObjectError::TypeError)?
                            .iter()
                            .map(|part| **part)
                            .collect::<Vec<_>>();
                        let mut body = progn(ctx, runtime, &body_values)?;
                        ncl_object::with_root(ctx, &mut body, |ctx, body| {
                            let key_values = elements(ctx, *keys)?;
                            ncl_object::with_roots(ctx, &key_values, |ctx, key_values| {
                                let mut tests = Vec::with_capacity(key_values.len());
                                for key in key_values {
                                    let quote = form(ctx, runtime, "QUOTE", &[**key])?;
                                    tests.push(form(ctx, runtime, "EQL", &[*temporary, quote])?);
                                }
                                ncl_object::with_roots(ctx, &tests, |ctx, tests| {
                                    let test_values =
                                        tests.iter().map(|test| **test).collect::<Vec<_>>();
                                    let mut test = or(ctx, runtime, &test_values)?;
                                    ncl_object::with_root(ctx, &mut test, |ctx, test| {
                                        branches = ncl_object::with_root(
                                            ctx,
                                            &mut branches,
                                            |ctx, branches| {
                                                form(ctx, runtime, "IF", &[*test, *body, *branches])
                                            },
                                        )?;
                                        Ok(())
                                    })
                                })
                            })
                        })
                    })?;
                }
                ncl_object::with_root(ctx, &mut branches, |ctx, branches| {
                    form(ctx, runtime, "LET", &[*let_bindings, *branches])
                })
            })
        })
    })
}

fn prog1(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        let first = roots.first().copied().ok_or(ObjectError::TypeError)?;
        let mut temporary = fresh_symbol(ctx, runtime)?;
        ncl_object::with_root(ctx, &mut temporary, |ctx, temporary| {
            let mut let_bindings = bindings(ctx, runtime, &[(*temporary, *first)])?;
            ncl_object::with_root(ctx, &mut let_bindings, |ctx, let_bindings| {
                let mut body = roots
                    .get(1..)
                    .ok_or(ObjectError::TypeError)?
                    .iter()
                    .map(|value| **value)
                    .collect::<Vec<_>>();
                body.push(*temporary);
                let mut body = progn(ctx, runtime, &body)?;
                ncl_object::with_root(ctx, &mut body, |ctx, body| {
                    form(ctx, runtime, "LET", &[*let_bindings, *body])
                })
            })
        })
    })
}

fn typecase(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word], errorp: bool) -> Result {
    let value = values.first().copied().ok_or(ObjectError::TypeError)?;
    let temporary = fresh_symbol(ctx, runtime)?;
    let mut branches = Vec::with_capacity(values.len().saturating_sub(1));
    let mut types = Vec::with_capacity(values.len().saturating_sub(1));
    for clause in values.get(1..).ok_or(ObjectError::TypeError)? {
        let parts = elements(ctx, *clause)?;
        let type_specifier = parts.first().copied().ok_or(ObjectError::TypeError)?;
        let body = progn(ctx, runtime, parts.get(1..).ok_or(ObjectError::TypeError)?)?;
        let quoted_type = form(ctx, runtime, "QUOTE", &[type_specifier])?;
        let type_test = form(ctx, runtime, "TYPEP", &[temporary, quoted_type])?;
        branches.push((type_test, body));
        types.push(type_specifier);
    }
    let fallback = if errorp {
        let mut expected_types = vec![symbol(ctx, runtime, "OR")?];
        expected_types.extend(types);
        let expected_type = list(ctx, runtime, &expected_types)?;
        let expected = form(ctx, runtime, "QUOTE", &[expected_type])?;
        let type_error = symbol(ctx, runtime, "TYPE-ERROR")?;
        let error_type = form(ctx, runtime, "QUOTE", &[type_error])?;
        let datum = symbol(ctx, runtime, ":DATUM")?;
        let expected_type = symbol(ctx, runtime, ":EXPECTED-TYPE")?;
        form(
            ctx,
            runtime,
            "ERROR",
            &[error_type, datum, temporary, expected_type, expected],
        )?
    } else {
        Word::NIL
    };
    let mut branch = fallback;
    for (test, body) in branches.into_iter().rev() {
        branch = form(ctx, runtime, "IF", &[test, body, branch])?;
    }
    let let_bindings = bindings(ctx, runtime, &[(temporary, value)])?;
    form(ctx, runtime, "LET", &[let_bindings, branch])
}

fn nth_value(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    let index = usize::try_from(
        values
            .first()
            .and_then(|value| value.as_fixnum())
            .filter(|index| *index >= 0)
            .ok_or(ObjectError::TypeError)?,
    )
    .map_err(|_| ObjectError::TypeError)?;
    let form_value = values.get(1).copied().ok_or(ObjectError::TypeError)?;
    let mut scope = Scope::new(ctx);
    let form_value: Handle<'_, Word> = scope.root(Local::from_word(form_value));
    let optional = symbol(scope.context_mut(), runtime, "&OPTIONAL")?;
    let optional: Handle<'_, Word> = scope.root(Local::from_word(optional));
    let rest = symbol(scope.context_mut(), runtime, "&REST")?;
    let rest: Handle<'_, Word> = scope.root(Local::from_word(rest));
    let rest_variable = fresh_symbol(scope.context_mut(), runtime)?;
    let rest_variable: Handle<'_, Word> = scope.root(Local::from_word(rest_variable));
    let optional_word = scope.get(optional).as_word();
    let rest_word = scope.get(rest).as_word();
    let rest_variable_word = scope.get(rest_variable).as_word();
    let lambda_list = list(
        scope.context_mut(),
        runtime,
        &[optional_word, rest_word, rest_variable_word],
    )?;
    let lambda_list: Handle<'_, Word> = scope.root(Local::from_word(lambda_list));
    let mut current = rest_variable;
    let mut selected_body = None;
    for position in 0..=index {
        let current_word = scope.get(current).as_word();
        let test = form(scope.context_mut(), runtime, "CONSP", &[current_word])?;
        let test: Handle<'_, Word> = scope.root(Local::from_word(test));
        let head = form(scope.context_mut(), runtime, "CAR", &[current_word])?;
        let head: Handle<'_, Word> = scope.root(Local::from_word(head));
        let test_word = scope.get(test).as_word();
        let head_word = scope.get(head).as_word();
        let selected = form(
            scope.context_mut(),
            runtime,
            "IF",
            &[test_word, head_word, Word::NIL],
        )?;
        let selected_handle: Handle<'_, Word> = scope.root(Local::from_word(selected));
        if position == index {
            selected_body = Some(selected_handle);
            break;
        }
        let next = form(scope.context_mut(), runtime, "CDR", &[current_word])?;
        current = scope.root(Local::from_word(next));
    }
    let selected = selected_body.ok_or(ObjectError::TypeError)?;
    let lambda_list_word = scope.get(lambda_list).as_word();
    let selected_word = scope.get(selected).as_word();
    let lambda = form(
        scope.context_mut(),
        runtime,
        "LAMBDA",
        &[lambda_list_word, selected_word],
    )?;
    let lambda: Handle<'_, Word> = scope.root(Local::from_word(lambda));
    let call_args = [scope.get(lambda).as_word(), scope.get(form_value).as_word()];
    form(
        scope.context_mut(),
        runtime,
        "MULTIPLE-VALUE-CALL",
        &call_args,
    )
}

fn do_macro(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
    sequential: bool,
) -> Result {
    let variable_specs = elements(ctx, values.first().copied().ok_or(ObjectError::TypeError)?)?;
    let end_clause = elements(ctx, values.get(1).copied().ok_or(ObjectError::TypeError)?)?;
    let end_test = end_clause.first().copied().ok_or(ObjectError::TypeError)?;
    let mut initial = Vec::with_capacity(variable_specs.len());
    let mut updates = Vec::with_capacity(variable_specs.len() * 2);
    for spec in variable_specs {
        let parts = if spec.is_cons() {
            elements(ctx, spec)?
        } else {
            vec![spec]
        };
        let variable = parts.first().copied().ok_or(ObjectError::TypeError)?;
        symbol_name(ctx, variable)?;
        if parts.len() > 3 {
            return Err(ObjectError::TypeError);
        }
        let init = parts.get(1).copied().unwrap_or(Word::NIL);
        initial.push((variable, init));
        if let Some(step) = parts.get(2).copied() {
            updates.push(variable);
            updates.push(step);
        }
    }
    let loop_tag = fresh_symbol(ctx, runtime)?;
    let end_tag = fresh_symbol(ctx, runtime)?;
    let go_end = form(ctx, runtime, "GO", &[end_tag])?;
    let exit = form(ctx, runtime, "IF", &[end_test, go_end])?;
    let mut tagbody = vec![loop_tag, exit];
    tagbody.extend_from_slice(values.get(2..).ok_or(ObjectError::TypeError)?);
    if !updates.is_empty() {
        tagbody.push(form(
            ctx,
            runtime,
            if sequential { "SETQ" } else { "PSETQ" },
            &updates,
        )?);
    }
    tagbody.push(form(ctx, runtime, "GO", &[loop_tag])?);
    tagbody.push(end_tag);
    let tagbody = form(ctx, runtime, "TAGBODY", &tagbody)?;
    let results = progn(
        ctx,
        runtime,
        end_clause.get(1..).ok_or(ObjectError::TypeError)?,
    )?;
    let initial_bindings = bindings(ctx, runtime, &initial)?;
    let binding_operator = if sequential { "LET*" } else { "LET" };
    let body = form(
        ctx,
        runtime,
        binding_operator,
        &[initial_bindings, tagbody, results],
    )?;
    form(ctx, runtime, "BLOCK", &[Word::NIL, body])
}

fn prog_macro(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
    sequential: bool,
) -> Result {
    let variables = elements(ctx, values.first().copied().ok_or(ObjectError::TypeError)?)?;
    let mut bindings = Vec::with_capacity(variables.len());
    for variable in variables {
        let parts = if variable.is_cons() {
            elements(ctx, variable)?
        } else {
            vec![variable]
        };
        let name = parts.first().copied().ok_or(ObjectError::TypeError)?;
        symbol_name(ctx, name)?;
        if parts.len() > 2 {
            return Err(ObjectError::TypeError);
        }
        let initial = parts.get(1).copied().unwrap_or(Word::NIL);
        bindings.push(binding(ctx, runtime, name, initial)?);
    }
    let tagbody = form(
        ctx,
        runtime,
        "TAGBODY",
        values.get(1..).ok_or(ObjectError::TypeError)?,
    )?;
    let binding_form = if sequential { "LET*" } else { "LET" };
    let binding_list = list(ctx, runtime, &bindings)?;
    let body = form(ctx, runtime, binding_form, &[binding_list, tagbody])?;
    form(ctx, runtime, "BLOCK", &[Word::NIL, body])
}

fn named(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word], kind: Kind) -> Result {
    match kind {
        Kind::When => {
            let test = values.first().copied().ok_or(ObjectError::TypeError)?;
            let body = progn(ctx, runtime, values.get(1..).ok_or(ObjectError::TypeError)?)?;
            form(ctx, runtime, "IF", &[test, body, Word::NIL])
        }
        Kind::Unless => {
            let test = values.first().copied().ok_or(ObjectError::TypeError)?;
            let body = progn(ctx, runtime, values.get(1..).ok_or(ObjectError::TypeError)?)?;
            form(ctx, runtime, "IF", &[test, Word::NIL, body])
        }
        Kind::And => and(ctx, runtime, values),
        Kind::Or => or(ctx, runtime, values),
        Kind::Cond => cond(ctx, runtime, values),
        Kind::Case | Kind::Ecase => case(ctx, runtime, values),
        Kind::Prog1 => prog1(ctx, runtime, values),
        Kind::Prog2 => {
            let first = values.first().copied().ok_or(ObjectError::TypeError)?;
            let rest = prog1(ctx, runtime, values.get(1..).ok_or(ObjectError::TypeError)?)?;
            form(ctx, runtime, "PROGN", &[first, rest])
        }
        Kind::Return => {
            let value = values.first().copied().unwrap_or(Word::NIL);
            form(ctx, runtime, "RETURN-FROM", &[Word::NIL, value])
        }
        Kind::Typecase => typecase(ctx, runtime, values, false),
        Kind::Etypecase => typecase(ctx, runtime, values, true),
        Kind::NthValue => nth_value(ctx, runtime, values),
        Kind::Do => do_macro(ctx, runtime, values, false),
        Kind::DoStar => do_macro(ctx, runtime, values, true),
        Kind::HandlerBind => handler_bind::expand(ctx, runtime, values),
        Kind::Prog => prog_macro(ctx, runtime, values, false),
        Kind::ProgStar => prog_macro(ctx, runtime, values, true),
    }
}

#[derive(Clone, Copy)]
enum Kind {
    When,
    Unless,
    And,
    Or,
    Cond,
    Case,
    Ecase,
    Typecase,
    Etypecase,
    Prog,
    ProgStar,
    Prog1,
    Prog2,
    Return,
    NthValue,
    Do,
    DoStar,
    HandlerBind,
}

macro_rules! callbacks {
    ($($legacy:ident, $adapter:ident, $kind:expr);+ $(;)?) => { $(
        fn $legacy(runtime: &Runtime, ctx: &mut ThreadContext, input: &[Word], _: &mut MultipleValues) -> Result {
            let values = args(ctx, input.first().copied().ok_or(ObjectError::TypeError)?)?;
            named(ctx, runtime, &values, $kind)
        }
        pub fn $adapter(ctx: &mut ThreadContext, runtime: &Runtime, input: &BuiltinArgs<'_>, values: &mut MultipleValues) -> Result {
            let words = (0..input.len()).filter_map(|index| input.get(index)).collect::<Vec<_>>();
            $legacy(runtime, ctx, &words, values)
        }
    )+ }
}

callbacks! { expand_when, expand_when_adapter, Kind::When;
expand_unless, expand_unless_adapter, Kind::Unless;
expand_and, expand_and_adapter, Kind::And;
expand_or, expand_or_adapter, Kind::Or;
expand_cond, expand_cond_adapter, Kind::Cond;
expand_case, expand_case_adapter, Kind::Case;
expand_ecase, expand_ecase_adapter, Kind::Ecase;
expand_ccase, expand_ccase_adapter, Kind::Ecase;
expand_typecase, expand_typecase_adapter, Kind::Typecase;
expand_etypecase, expand_etypecase_adapter, Kind::Etypecase;
expand_ctypecase, expand_ctypecase_adapter, Kind::Etypecase;
expand_prog, expand_prog_adapter, Kind::Prog;
expand_prog_star, expand_prog_star_adapter, Kind::ProgStar;
expand_prog1, expand_prog1_adapter, Kind::Prog1;
expand_prog2, expand_prog2_adapter, Kind::Prog2;
expand_return, expand_return_adapter, Kind::Return;
expand_nth_value, expand_nth_value_adapter, Kind::NthValue;
expand_do, expand_do_adapter, Kind::Do;
expand_do_star, expand_do_star_adapter, Kind::DoStar;
expand_handler_bind, expand_handler_bind_adapter, Kind::HandlerBind }
