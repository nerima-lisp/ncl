//! Structural expanders for the control macros.
#![allow(clippy::redundant_pub_crate)]

use crate::{elements, fresh_symbol, list, symbol};
use ncl_object::{
    BuiltinArgs, Handle, HeapRootedWord, HeapRootedWords, Local, MultipleValues, ObjectError,
    ObjectRef, Runtime, Scope, ThreadContext, Word, classify_object, string_length, string_ref,
    symbol_name,
};

mod case;
mod define_condition;
mod handler_bind;
mod handler_case;
mod logical;
mod restart_case;
mod typecase;

mod multiple_value_bind;
mod multiple_values;

pub(crate) use handler_bind::{binding, bindings};
pub(crate) use multiple_values::{
    expand_multiple_value_bind_adapter, expand_multiple_value_list_adapter,
};

type Result<T = Word> = std::result::Result<T, ObjectError>;

fn is_otherwise(ctx: &ThreadContext, word: Word) -> Result<bool> {
    let ObjectRef::Symbol(_) = classify_object(ctx, word) else {
        return Ok(false);
    };
    let name = symbol_name(ctx, word)?;
    let expected = "OTHERWISE";
    if string_length(ctx, name)? != expected.len() {
        return Ok(false);
    }
    expected
        .chars()
        .enumerate()
        .map(|(index, character)| Ok(string_ref(ctx, name, index)? == character))
        .collect::<Result<Vec<_>>>()
        .map(|matches| matches.into_iter().all(|matched| matched))
}

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

pub(crate) fn args(ctx: &mut ThreadContext, form: Word) -> Result<HeapRootedWords> {
    let mut values = elements(ctx, form)?;
    if values.is_empty() {
        return Err(ObjectError::TypeError);
    }
    values.remove(0);
    HeapRootedWords::new(ctx, &values)
}

pub(crate) fn progn(ctx: &mut ThreadContext, runtime: &Runtime, body: &[Word]) -> Result {
    form(ctx, runtime, "PROGN", body)
}

fn cond(ctx: &mut ThreadContext, runtime: &Runtime, clauses: &[HeapRootedWord]) -> Result {
    let Some(clause) = clauses.first() else {
        return Ok(Word::NIL);
    };
    let values = elements(ctx, clause.get())?;
    let values = HeapRootedWords::new(ctx, &values)?;
    let test = values.first().ok_or(ObjectError::TypeError)?;
    let yes = if values.len() == 1 {
        test.get()
    } else {
        let body = values
            .as_slice()
            .get(1..)
            .ok_or(ObjectError::TypeError)?
            .iter()
            .map(HeapRootedWord::get)
            .collect::<Vec<_>>();
        progn(ctx, runtime, &body)?
    };
    let yes = HeapRootedWord::new(ctx, yes)?;
    let no = cond(
        ctx,
        runtime,
        clauses.get(1..).ok_or(ObjectError::TypeError)?,
    )?;
    form(ctx, runtime, "IF", &[test.get(), yes.get(), no])
}

fn prog1(ctx: &mut ThreadContext, runtime: &Runtime, values: &[HeapRootedWord]) -> Result {
    let first = values.first().ok_or(ObjectError::TypeError)?;
    let temporary_word = fresh_symbol(ctx, runtime)?;
    let temporary = HeapRootedWord::new(ctx, temporary_word)?;
    let let_bindings = bindings(ctx, runtime, &[(temporary.get(), first.get())])?;
    let let_bindings = HeapRootedWord::new(ctx, let_bindings)?;
    let mut body = values
        .get(1..)
        .ok_or(ObjectError::TypeError)?
        .iter()
        .map(HeapRootedWord::get)
        .collect::<Vec<_>>();
    body.push(temporary.get());
    let body = progn(ctx, runtime, &body)?;
    form(ctx, runtime, "LET", &[let_bindings.get(), body])
}

fn nth_value(ctx: &mut ThreadContext, runtime: &Runtime, values: &[HeapRootedWord]) -> Result {
    let index = usize::try_from(
        values
            .first()
            .and_then(|value| value.get().as_fixnum())
            .filter(|index| *index >= 0)
            .ok_or(ObjectError::TypeError)?,
    )
    .map_err(|_| ObjectError::TypeError)?;
    let mut scope = Scope::new(ctx);
    let form_value: Handle<'_, Word> = scope.root(Local::from_word(
        values.get(1).ok_or(ObjectError::TypeError)?.get(),
    ));
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
        let test_current = scope.get(current).as_word();
        let test = form(scope.context_mut(), runtime, "CONSP", &[test_current])?;
        let test: Handle<'_, Word> = scope.root(Local::from_word(test));
        let head_current = scope.get(current).as_word();
        let head = form(scope.context_mut(), runtime, "CAR", &[head_current])?;
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
        let next_current = scope.get(current).as_word();
        let next = form(scope.context_mut(), runtime, "CDR", &[next_current])?;
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
    values: &[HeapRootedWord],
    sequential: bool,
) -> Result {
    let variable_specs = elements(ctx, values.first().ok_or(ObjectError::TypeError)?.get())?;
    let variable_specs = HeapRootedWords::new(ctx, &variable_specs)?;
    let end_clause = elements(ctx, values.get(1).ok_or(ObjectError::TypeError)?.get())?;
    let end_clause = HeapRootedWords::new(ctx, &end_clause)?;
    let end_test = end_clause.first().ok_or(ObjectError::TypeError)?;
    let mut initial = HeapRootedWords::default();
    let mut updates = HeapRootedWords::default();
    for spec in variable_specs.iter() {
        let parts = if spec.get().is_cons() {
            elements(ctx, spec.get())?
        } else {
            vec![spec.get()]
        };
        let parts = HeapRootedWords::new(ctx, &parts)?;
        let variable = parts.first().ok_or(ObjectError::TypeError)?;
        symbol_name(ctx, variable.get())?;
        if parts.len() > 3 {
            return Err(ObjectError::TypeError);
        }
        let init = parts.get(1).map_or(Word::NIL, HeapRootedWord::get);
        initial.push(ctx, variable.get())?;
        initial.push(ctx, init)?;
        if let Some(step) = parts.get(2) {
            updates.push(ctx, variable.get())?;
            updates.push(ctx, step.get())?;
        }
    }
    let loop_tag_word = fresh_symbol(ctx, runtime)?;
    let loop_tag = HeapRootedWord::new(ctx, loop_tag_word)?;
    let end_tag_word = fresh_symbol(ctx, runtime)?;
    let end_tag = HeapRootedWord::new(ctx, end_tag_word)?;
    let go_end = form(ctx, runtime, "GO", &[end_tag.get()])?;
    let exit = form(ctx, runtime, "IF", &[end_test.get(), go_end])?;
    let mut tagbody = HeapRootedWords::new(ctx, &[loop_tag.get(), exit])?;
    for value in values.get(2..).ok_or(ObjectError::TypeError)? {
        tagbody.push(ctx, value.get())?;
    }
    if !updates.is_empty() {
        let updates = updates.iter().map(HeapRootedWord::get).collect::<Vec<_>>();
        let update_form = form(
            ctx,
            runtime,
            if sequential { "SETQ" } else { "PSETQ" },
            &updates,
        )?;
        tagbody.push(ctx, update_form)?;
    }
    let repeat = form(ctx, runtime, "GO", &[loop_tag.get()])?;
    tagbody.push(ctx, repeat)?;
    tagbody.push(ctx, end_tag.get())?;
    let tagbody_values = tagbody.iter().map(HeapRootedWord::get).collect::<Vec<_>>();
    let tagbody = form(ctx, runtime, "TAGBODY", &tagbody_values)?;
    let tagbody = HeapRootedWord::new(ctx, tagbody)?;
    let result_values = end_clause
        .as_slice()
        .get(1..)
        .ok_or(ObjectError::TypeError)?
        .iter()
        .map(HeapRootedWord::get)
        .collect::<Vec<_>>();
    let results_word = progn(ctx, runtime, &result_values)?;
    let results = HeapRootedWord::new(ctx, results_word)?;
    let (initial_pairs, remainder) = initial.as_slice().as_chunks::<2>();
    if !remainder.is_empty() {
        return Err(ObjectError::Layout);
    }
    let initial_pairs = initial_pairs
        .iter()
        .map(|[variable, value]| (variable.get(), value.get()))
        .collect::<Vec<_>>();
    let initial_bindings = bindings(ctx, runtime, &initial_pairs)?;
    let binding_operator = if sequential { "LET*" } else { "LET" };
    let body = form(
        ctx,
        runtime,
        binding_operator,
        &[initial_bindings, tagbody.get(), results.get()],
    )?;
    form(ctx, runtime, "BLOCK", &[Word::NIL, body])
}

fn prog_macro(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[HeapRootedWord],
    sequential: bool,
) -> Result {
    let variables = elements(ctx, values.first().ok_or(ObjectError::TypeError)?.get())?;
    let variables = HeapRootedWords::new(ctx, &variables)?;
    let mut bindings = HeapRootedWords::default();
    for variable in variables.iter() {
        let parts = if variable.get().is_cons() {
            elements(ctx, variable.get())?
        } else {
            vec![variable.get()]
        };
        let parts = HeapRootedWords::new(ctx, &parts)?;
        let name = parts.first().ok_or(ObjectError::TypeError)?;
        symbol_name(ctx, name.get())?;
        if parts.len() > 2 {
            return Err(ObjectError::TypeError);
        }
        let initial = parts.get(1).map_or(Word::NIL, HeapRootedWord::get);
        let binding = binding(ctx, runtime, name.get(), initial)?;
        bindings.push(ctx, binding)?;
    }
    let body_values = values
        .get(1..)
        .ok_or(ObjectError::TypeError)?
        .iter()
        .map(HeapRootedWord::get)
        .collect::<Vec<_>>();
    let tagbody = form(ctx, runtime, "TAGBODY", &body_values)?;
    let tagbody = HeapRootedWord::new(ctx, tagbody)?;
    let binding_form = if sequential { "LET*" } else { "LET" };
    let binding_values = bindings.iter().map(HeapRootedWord::get).collect::<Vec<_>>();
    let binding_list = list(ctx, runtime, &binding_values)?;
    let body = form(ctx, runtime, binding_form, &[binding_list, tagbody.get()])?;
    form(ctx, runtime, "BLOCK", &[Word::NIL, body])
}

fn named(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[HeapRootedWord],
    kind: Kind,
) -> Result {
    match kind {
        Kind::When => {
            let test = values.first().ok_or(ObjectError::TypeError)?;
            let body_values = values
                .get(1..)
                .ok_or(ObjectError::TypeError)?
                .iter()
                .map(HeapRootedWord::get)
                .collect::<Vec<_>>();
            let body = progn(ctx, runtime, &body_values)?;
            form(ctx, runtime, "IF", &[test.get(), body, Word::NIL])
        }
        Kind::Unless => {
            let test = values.first().ok_or(ObjectError::TypeError)?;
            let body_values = values
                .get(1..)
                .ok_or(ObjectError::TypeError)?
                .iter()
                .map(HeapRootedWord::get)
                .collect::<Vec<_>>();
            let body = progn(ctx, runtime, &body_values)?;
            form(ctx, runtime, "IF", &[test.get(), Word::NIL, body])
        }
        Kind::And => logical::and(ctx, runtime, values),
        Kind::Or => logical::or(ctx, runtime, values),
        Kind::Cond => cond(ctx, runtime, values),
        Kind::Case => {
            let words = values.iter().map(HeapRootedWord::get).collect::<Vec<_>>();
            case::case(ctx, runtime, &words, false)
        }
        Kind::Ecase => {
            let words = values.iter().map(HeapRootedWord::get).collect::<Vec<_>>();
            case::case(ctx, runtime, &words, true)
        }
        Kind::Prog1 => prog1(ctx, runtime, values),
        Kind::Prog2 => {
            let first = values.first().ok_or(ObjectError::TypeError)?;
            let rest = prog1(ctx, runtime, values.get(1..).ok_or(ObjectError::TypeError)?)?;
            form(ctx, runtime, "PROGN", &[first.get(), rest])
        }
        Kind::Return => {
            let value = values.first().map_or(Word::NIL, HeapRootedWord::get);
            form(ctx, runtime, "RETURN-FROM", &[Word::NIL, value])
        }
        Kind::Typecase => {
            let words = values.iter().map(HeapRootedWord::get).collect::<Vec<_>>();
            typecase::expand(ctx, runtime, &words, false)
        }
        Kind::Etypecase => {
            let words = values.iter().map(HeapRootedWord::get).collect::<Vec<_>>();
            typecase::expand(ctx, runtime, &words, true)
        }
        Kind::NthValue => nth_value(ctx, runtime, values),
        Kind::Do => do_macro(ctx, runtime, values, false),
        Kind::DoStar => do_macro(ctx, runtime, values, true),
        Kind::HandlerBind => handler_bind::expand(ctx, runtime, values),
        Kind::HandlerCase => {
            let words = values.iter().map(HeapRootedWord::get).collect::<Vec<_>>();
            handler_case::expand(ctx, runtime, &words)
        }
        Kind::IgnoreErrors => {
            let words = values.iter().map(HeapRootedWord::get).collect::<Vec<_>>();
            handler_case::expand_ignore_errors(ctx, runtime, &words)
        }
        Kind::RestartBind => {
            let words = values.iter().map(HeapRootedWord::get).collect::<Vec<_>>();
            restart_case::expand_restart_bind(ctx, runtime, &words)
        }
        Kind::RestartCase => {
            let words = values.iter().map(HeapRootedWord::get).collect::<Vec<_>>();
            restart_case::expand_restart_case(ctx, runtime, &words)
        }
        Kind::WithSimpleRestart => {
            let words = values.iter().map(HeapRootedWord::get).collect::<Vec<_>>();
            restart_case::expand_with_simple_restart(ctx, runtime, &words)
        }
        Kind::CheckType => {
            let words = values.iter().map(HeapRootedWord::get).collect::<Vec<_>>();
            restart_case::expand_check_type(ctx, runtime, &words)
        }
        Kind::Assert => {
            let words = values.iter().map(HeapRootedWord::get).collect::<Vec<_>>();
            restart_case::expand_assert(ctx, runtime, &words)
        }
        Kind::DefineCondition => define_condition::expand(ctx, runtime, values),
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
    HandlerCase,
    IgnoreErrors,
    RestartBind,
    RestartCase,
    WithSimpleRestart,
    CheckType,
    Assert,
    DefineCondition,
}

macro_rules! callbacks {
    ($($legacy:ident, $adapter:ident, $kind:expr);+ $(;)?) => { $(
        fn $legacy(runtime: &Runtime, ctx: &mut ThreadContext, input: &[Word], _: &mut MultipleValues) -> Result {
            let values = args(ctx, input.first().copied().ok_or(ObjectError::TypeError)?)?;
            named(ctx, runtime, values.as_slice(), $kind)
        }
        pub fn $adapter(ctx: &mut ThreadContext, runtime: &Runtime, input: &BuiltinArgs<'_>, values: &mut MultipleValues) -> Result {
            let form = input.get(0).ok_or(ObjectError::TypeError)?;
            let form = HeapRootedWord::new(ctx, form)?;
            $legacy(runtime, ctx, &[form.get()], values)
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
expand_handler_bind, expand_handler_bind_adapter, Kind::HandlerBind;
expand_handler_case, expand_handler_case_adapter, Kind::HandlerCase;
expand_ignore_errors, expand_ignore_errors_adapter, Kind::IgnoreErrors;
expand_restart_bind, expand_restart_bind_adapter, Kind::RestartBind;
expand_restart_case, expand_restart_case_adapter, Kind::RestartCase;
expand_with_simple_restart, expand_with_simple_restart_adapter, Kind::WithSimpleRestart;
expand_check_type, expand_check_type_adapter, Kind::CheckType;
expand_assert, expand_assert_adapter, Kind::Assert;
expand_define_condition, expand_define_condition_adapter, Kind::DefineCondition }
