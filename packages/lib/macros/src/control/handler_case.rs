//! `HANDLER-CASE` and `IGNORE-ERRORS`, expanded onto `HANDLER-BIND` plus
//! `BLOCK`/`RETURN-FROM` (C1).
//!
//! Every clause's handler performs the transfer itself (`return-from` to the
//! enclosing block), exactly like a hand-written
//! `(block b (handler-bind ((type (lambda (c) (return-from b ...)))) form))`
//! — so `HANDLER-BIND`'s existing first-match semantics are exactly what is
//! needed; no new runtime machinery is required.

use super::handler_bind::symbol_named;
use ncl_object::{Handle, Local, ObjectError, Runtime, Scope, ThreadContext, Word};

type Result<T = Word> = std::result::Result<T, ObjectError>;

pub(super) fn intern<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    name: &str,
) -> std::result::Result<Handle<'ctx>, ObjectError> {
    let (package, symbol_name) = name
        .split_once("::")
        .map_or(("COMMON-LISP", name), |(package, symbol)| (package, symbol));
    scope.intern(runtime, package, symbol_name)
}

pub(super) fn cons_list<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    items: &[Handle<'ctx>],
) -> std::result::Result<Handle<'ctx>, ObjectError> {
    let mut result = scope.root(Local::from_word(Word::NIL));
    for item in items.iter().rev() {
        result = scope.make_cons(runtime, *item, result)?;
    }
    Ok(result)
}

pub(super) fn form<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    name: &str,
    args: &[Handle<'ctx>],
) -> std::result::Result<Handle<'ctx>, ObjectError> {
    let operator = intern(scope, runtime, name)?;
    let rest = cons_list(scope, runtime, args)?;
    scope.make_cons(runtime, operator, rest)
}

pub(super) fn word_handle<'ctx>(scope: &mut Scope<'ctx>, word: Word) -> Handle<'ctx> {
    scope.root(Local::from_word(word))
}

pub(super) fn fresh<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
) -> std::result::Result<Handle<'ctx>, ObjectError> {
    let word = crate::fresh_symbol(scope.context_mut(), runtime)?;
    Ok(scope.root(Local::from_word(word)))
}

struct Clause<'ctx> {
    condition: Handle<'ctx>,
    binding: Option<Handle<'ctx>>,
    body: Vec<Handle<'ctx>>,
}

/// Expand `(handler-case protected-form clause*)`.
///
/// A clause is `(type (var) body*)`, `(type () body*)`, or the special
/// `(:no-error (value*) body*)` clause, which runs when `protected-form`
/// returns normally (its values are bound to `(value*)`).
pub(super) fn expand(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    let mut scope = Scope::new(ctx);
    let protected_word = *values.first().ok_or(ObjectError::TypeError)?;
    let protected = word_handle(&mut scope, protected_word);

    let clause_locals: Vec<Local<'_>> = values
        .get(1..)
        .ok_or(ObjectError::TypeError)?
        .iter()
        .copied()
        .map(Local::from_word)
        .collect();
    let clause_handles = scope.root_many(&clause_locals);

    let mut clauses: Vec<Clause<'_>> = Vec::new();
    let mut no_error: Option<(Handle<'_>, Vec<Handle<'_>>)> = None;
    for index in 0..clause_handles.len() {
        let clause_handle = *clause_handles
            .as_slice()
            .get(index)
            .ok_or(ObjectError::TypeError)?;
        let clause_word = scope.get(clause_handle).as_word();
        let parts = scope.list_to_handle_vec(Local::from_word(clause_word))?;
        let parts_slice = parts.as_slice();
        let head = *parts_slice.first().ok_or(ObjectError::TypeError)?;
        let binding_list = *parts_slice.get(1).ok_or(ObjectError::TypeError)?;
        let body: Vec<Handle<'_>> = parts_slice.get(2..).ok_or(ObjectError::TypeError)?.to_vec();
        let head_word = scope.get(head).as_word();
        if symbol_named(scope.context(), head_word, "NO-ERROR") {
            no_error = Some((binding_list, body));
            continue;
        }
        let binding_list_word = scope.get(binding_list).as_word();
        let binding_vars = scope.list_to_handle_vec(Local::from_word(binding_list_word))?;
        let binding = binding_vars.as_slice().first().copied();
        clauses.push(Clause {
            condition: head,
            binding,
            body,
        });
    }

    let block_tag = fresh(&mut scope, runtime)?;

    let mut handler_clause_handles: Vec<Handle<'_>> = Vec::with_capacity(clauses.len());
    for clause in &clauses {
        let condition_variable = fresh(&mut scope, runtime)?;
        let inner_body = if clause.body.is_empty() {
            word_handle(&mut scope, Word::NIL)
        } else {
            form(&mut scope, runtime, "PROGN", &clause.body)?
        };
        let inner_body = if let Some(variable) = clause.binding {
            let pair = cons_list(&mut scope, runtime, &[variable, condition_variable])?;
            let bindings_list = cons_list(&mut scope, runtime, &[pair])?;
            form(&mut scope, runtime, "LET", &[bindings_list, inner_body])?
        } else {
            inner_body
        };
        let returned = form(&mut scope, runtime, "RETURN-FROM", &[block_tag, inner_body])?;
        let lambda_list = cons_list(&mut scope, runtime, &[condition_variable])?;
        let handler_lambda = form(&mut scope, runtime, "LAMBDA", &[lambda_list, returned])?;
        let clause_form = cons_list(&mut scope, runtime, &[clause.condition, handler_lambda])?;
        handler_clause_handles.push(clause_form);
    }
    let clauses_list = cons_list(&mut scope, runtime, &handler_clause_handles)?;

    let result_form = if let Some((lambda_list, body)) = no_error {
        let noerror_body = if body.is_empty() {
            word_handle(&mut scope, Word::NIL)
        } else {
            form(&mut scope, runtime, "PROGN", &body)?
        };
        let lambda = form(&mut scope, runtime, "LAMBDA", &[lambda_list, noerror_body])?;
        form(
            &mut scope,
            runtime,
            "MULTIPLE-VALUE-CALL",
            &[lambda, protected],
        )?
    } else {
        protected
    };
    let protected_return = form(
        &mut scope,
        runtime,
        "RETURN-FROM",
        &[block_tag, result_form],
    )?;
    let handler_bind_form = form(
        &mut scope,
        runtime,
        "HANDLER-BIND",
        &[clauses_list, protected_return],
    )?;
    let block_form = form(
        &mut scope,
        runtime,
        "BLOCK",
        &[block_tag, handler_bind_form],
    )?;
    Ok(scope.get(block_form).as_word())
}

/// Expand `(ignore-errors form*)` into
/// `(handler-case (progn form*) (error (c) (values nil c)))`.
pub(super) fn expand_ignore_errors(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result {
    let mut scope = Scope::new(ctx);
    let body_locals: Vec<Local<'_>> = values.iter().copied().map(Local::from_word).collect();
    let body = scope.root_many(&body_locals);
    let protected = if body.is_empty() {
        word_handle(&mut scope, Word::NIL)
    } else {
        form(&mut scope, runtime, "PROGN", body.as_slice())?
    };
    let condition_variable = fresh(&mut scope, runtime)?;
    let binding_list = cons_list(&mut scope, runtime, &[condition_variable])?;
    let nil = word_handle(&mut scope, Word::NIL);
    let values_form = form(&mut scope, runtime, "VALUES", &[nil, condition_variable])?;
    let error_symbol = intern(&mut scope, runtime, "ERROR")?;
    let clause = cons_list(
        &mut scope,
        runtime,
        &[error_symbol, binding_list, values_form],
    )?;
    let handler_case_words = [scope.get(protected).as_word(), scope.get(clause).as_word()];
    expand(scope.context_mut(), runtime, &handler_case_words)
}
