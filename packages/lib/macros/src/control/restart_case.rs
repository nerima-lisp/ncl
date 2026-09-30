//! `RESTART-BIND`, `RESTART-CASE`, and `WITH-SIMPLE-RESTART` (C1), expanded
//! onto the dynamic restart stack primitives `NCL-EXT::PUSH-RESTART`/
//! `NCL-EXT::POP-RESTART` exactly as `HANDLER-BIND` expands onto
//! `NCL-EXT::PUSH-HANDLER`/`NCL-EXT::POP-HANDLER`.

use super::handler_bind::symbol_named;
use super::handler_case::{cons_list, form, fresh, intern, word_handle};
use ncl_object::{Handle, Local, ObjectError, Runtime, Scope, ThreadContext, Word};

type Result<T = Word> = std::result::Result<T, ObjectError>;

/// A restart's parsed `:report`/`:interactive`/`:test` options, defaulting
/// to `NIL` (no report string/function, no interactive reader, always
/// applicable).
struct Options<'ctx> {
    report: Handle<'ctx>,
    interactive: Handle<'ctx>,
    test: Handle<'ctx>,
}

/// One parsed `restart-case` clause: `(name (lambda-list) option* body*)`.
struct RestartClause<'ctx> {
    name: Handle<'ctx>,
    lambda_list: Handle<'ctx>,
    options: Options<'ctx>,
    body: Vec<Handle<'ctx>>,
}

fn parse_options<'ctx>(
    scope: &mut Scope<'ctx>,
    items: &[Handle<'ctx>],
) -> (Options<'ctx>, Vec<Handle<'ctx>>) {
    let nil = word_handle(scope, Word::NIL);
    let mut report = nil;
    let mut interactive = nil;
    let mut test = nil;
    let mut index = 0;
    while index + 1 < items.len() {
        let Some(key) = items.get(index).copied() else {
            break;
        };
        let key_word = scope.get(key).as_word();
        let Some(value) = items.get(index + 1).copied() else {
            break;
        };
        if symbol_named(scope.context(), key_word, "REPORT")
            || symbol_named(scope.context(), key_word, "REPORT-FUNCTION")
        {
            report = value;
        } else if symbol_named(scope.context(), key_word, "INTERACTIVE")
            || symbol_named(scope.context(), key_word, "INTERACTIVE-FUNCTION")
        {
            interactive = value;
        } else if symbol_named(scope.context(), key_word, "TEST")
            || symbol_named(scope.context(), key_word, "TEST-FUNCTION")
        {
            test = value;
        } else {
            break;
        }
        index += 2;
    }
    let rest = items.get(index..).unwrap_or(&[]).to_vec();
    (
        Options {
            report,
            interactive,
            test,
        },
        rest,
    )
}

/// Build the restart's name string from its source symbol: `(quote "NAME")`.
fn restart_name_form<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    name_symbol: Handle<'ctx>,
) -> std::result::Result<Handle<'ctx>, ObjectError> {
    let name_symbol_word = scope.get(name_symbol).as_word();
    let name_string = ncl_object::symbol_name(scope.context(), name_symbol_word)?;
    let name_string_handle = word_handle(scope, name_string);
    form(scope, runtime, "QUOTE", &[name_string_handle])
}

/// Expand a list of `(token push-restart-call)` `LET` bindings, wrapping
/// `body` in `UNWIND-PROTECT`... deliberately not: pairing `unwind-protect`
/// with a handler/restart closure that performs `return-from` trips a
/// front-end defect (see `handler_bind::expand`'s note), so this uses the
/// same `progn`-plus-explicit-pop pattern as `handler-bind`.
fn wrap_restarts<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    restarts: &[(Handle<'ctx>, Handle<'ctx>, Options<'ctx>)],
    body: Handle<'ctx>,
) -> std::result::Result<Handle<'ctx>, ObjectError> {
    let mut result = body;
    for (name, function, options) in restarts.iter().rev() {
        let name_form = restart_name_form(scope, runtime, *name)?;
        let push = form(
            scope,
            runtime,
            "NCL-EXT::PUSH-RESTART",
            &[
                name_form,
                *function,
                options.report,
                options.interactive,
                options.test,
            ],
        )?;
        let token = fresh(scope, runtime)?;
        let binding_pair = cons_list(scope, runtime, &[token, push])?;
        let binding_list = cons_list(scope, runtime, &[binding_pair])?;
        let pop = form(scope, runtime, "NCL-EXT::POP-RESTART", &[token])?;
        // `prog1`, not `progn`: this form's value is `result`'s, not `pop`'s.
        let body_form = form(scope, runtime, "PROG1", &[result, pop])?;
        result = form(scope, runtime, "LET", &[binding_list, body_form])?;
    }
    Ok(result)
}

/// Expand `(restart-bind ((name function-form option*)*) body*)`.
pub(super) fn expand_restart_bind(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result {
    let mut scope = Scope::new(ctx);
    let clause_list_word = *values.first().ok_or(ObjectError::TypeError)?;
    let clause_locals = scope.list_to_handle_vec(Local::from_word(clause_list_word))?;
    let body_locals: Vec<Local<'_>> = values
        .get(1..)
        .ok_or(ObjectError::TypeError)?
        .iter()
        .copied()
        .map(Local::from_word)
        .collect();
    let body_handles = scope.root_many(&body_locals);
    let body = if body_handles.is_empty() {
        word_handle(&mut scope, Word::NIL)
    } else {
        form(&mut scope, runtime, "PROGN", body_handles.as_slice())?
    };

    let mut restarts = Vec::with_capacity(clause_locals.len());
    for index in 0..clause_locals.len() {
        let clause_handle = *clause_locals
            .as_slice()
            .get(index)
            .ok_or(ObjectError::TypeError)?;
        let clause_word = scope.get(clause_handle).as_word();
        let parts = scope.list_to_handle_vec(Local::from_word(clause_word))?;
        let parts_slice = parts.as_slice();
        let name = *parts_slice.first().ok_or(ObjectError::TypeError)?;
        let function = *parts_slice.get(1).ok_or(ObjectError::TypeError)?;
        let rest = parts_slice.get(2..).unwrap_or(&[]).to_vec();
        let (options, _unused) = parse_options(&mut scope, &rest);
        restarts.push((name, function, options));
    }
    let expanded = wrap_restarts(&mut scope, runtime, &restarts, body)?;
    Ok(scope.get(expanded).as_word())
}

/// Expand `(with-simple-restart (name report) body*)` into
/// `(restart-case (progn body*) (name () report))`, i.e. a restart with no
/// arguments whose invocation simply returns from the protected form.
pub(super) fn expand_with_simple_restart(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result {
    let mut scope = Scope::new(ctx);
    let heading_word = *values.first().ok_or(ObjectError::TypeError)?;
    let heading = scope.list_to_handle_vec(Local::from_word(heading_word))?;
    let heading_slice = heading.as_slice();
    let name = *heading_slice.first().ok_or(ObjectError::TypeError)?;
    let report = heading_slice.get(1).copied();

    let body_locals: Vec<Local<'_>> = values
        .get(1..)
        .ok_or(ObjectError::TypeError)?
        .iter()
        .copied()
        .map(Local::from_word)
        .collect();
    let body_handles = scope.root_many(&body_locals);
    let protected = if body_handles.is_empty() {
        word_handle(&mut scope, Word::NIL)
    } else {
        form(&mut scope, runtime, "PROGN", body_handles.as_slice())?
    };

    let empty_lambda_list = word_handle(&mut scope, Word::NIL);
    let nil_body = word_handle(&mut scope, Word::NIL);
    let mut clause_items = vec![name, empty_lambda_list];
    if let Some(report) = report {
        let report_keyword = intern_keyword(&mut scope, runtime, "REPORT")?;
        clause_items.push(report_keyword);
        clause_items.push(report);
    }
    clause_items.push(nil_body);
    let clause = cons_list(&mut scope, runtime, &clause_items)?;

    let restart_case_words = [scope.get(protected).as_word(), scope.get(clause).as_word()];
    expand_restart_case(scope.context_mut(), runtime, &restart_case_words)
}

fn intern_keyword<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    name: &str,
) -> std::result::Result<Handle<'ctx>, ObjectError> {
    scope.intern(runtime, "KEYWORD", name)
}

/// Expand `(restart-case protected-form clause*)`.
///
/// A clause is `(name (lambda-list) option* body*)`. Each clause's restart
/// function performs the transfer itself (`return-from` to the enclosing
/// block, applying the clause's lambda list to whatever arguments
/// `INVOKE-RESTART` supplies), exactly like `HANDLER-CASE`'s clauses.
pub(super) fn expand_restart_case(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result {
    let mut scope = Scope::new(ctx);
    let protected = word_handle(&mut scope, *values.first().ok_or(ObjectError::TypeError)?);
    let clause_locals: Vec<Local<'_>> = values
        .get(1..)
        .ok_or(ObjectError::TypeError)?
        .iter()
        .copied()
        .map(Local::from_word)
        .collect();
    let clause_handles = scope.root_many(&clause_locals);

    let mut clauses = Vec::with_capacity(clause_handles.len());
    for index in 0..clause_handles.len() {
        let clause_handle = *clause_handles
            .as_slice()
            .get(index)
            .ok_or(ObjectError::TypeError)?;
        let clause_word = scope.get(clause_handle).as_word();
        let parts = scope.list_to_handle_vec(Local::from_word(clause_word))?;
        let parts_slice = parts.as_slice();
        let name = *parts_slice.first().ok_or(ObjectError::TypeError)?;
        let lambda_list = *parts_slice.get(1).ok_or(ObjectError::TypeError)?;
        let rest = parts_slice.get(2..).unwrap_or(&[]).to_vec();
        let (options, body) = parse_options(&mut scope, &rest);
        clauses.push(RestartClause {
            name,
            lambda_list,
            options,
            body,
        });
    }

    let block_tag = fresh(&mut scope, runtime)?;
    let mut restarts = Vec::with_capacity(clauses.len());
    for clause in &clauses {
        let args_variable = fresh(&mut scope, runtime)?;
        let body_form = if clause.body.is_empty() {
            word_handle(&mut scope, Word::NIL)
        } else {
            form(&mut scope, runtime, "PROGN", &clause.body)?
        };
        let inner_lambda = form(
            &mut scope,
            runtime,
            "LAMBDA",
            &[clause.lambda_list, body_form],
        )?;
        let applied = form(&mut scope, runtime, "APPLY", &[inner_lambda, args_variable])?;
        let returned = form(&mut scope, runtime, "RETURN-FROM", &[block_tag, applied])?;
        let rest_marker = intern_rest_marker(&mut scope, runtime)?;
        let outer_lambda_list = cons_list(&mut scope, runtime, &[rest_marker, args_variable])?;
        let function = form(
            &mut scope,
            runtime,
            "LAMBDA",
            &[outer_lambda_list, returned],
        )?;
        restarts.push((clause.name, function, clone_options(&clause.options)));
    }

    let protected_return = form(&mut scope, runtime, "RETURN-FROM", &[block_tag, protected])?;
    let wrapped = wrap_restarts(&mut scope, runtime, &restarts, protected_return)?;
    let block_form = form(&mut scope, runtime, "BLOCK", &[block_tag, wrapped])?;
    Ok(scope.get(block_form).as_word())
}

const fn clone_options<'ctx>(options: &Options<'ctx>) -> Options<'ctx> {
    Options {
        report: options.report,
        interactive: options.interactive,
        test: options.test,
    }
}

fn intern_rest_marker<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
) -> std::result::Result<Handle<'ctx>, ObjectError> {
    scope.intern(runtime, "COMMON-LISP", "&REST")
}

/// Expand `(check-type place type &optional string)` into a `TYPEP` guard
/// that signals a `TYPE-ERROR` (via the ordinary `ERROR` path, since
/// generated code does not yet convert builtin type failures into
/// conditions — see C3, out of this lane's scope) offering a `STORE-VALUE`
/// restart.
///
/// Phase 1 simplification: the restart is established and returns the
/// supplied value, but does not `SETF` it back into `place` or loop back to
/// re-test it. `place` is frequently not a `setf`-able place at all in
/// practice (for example a literal, as in `(check-type 1 integer)`), and
/// `SETF`'s own expander must resolve it at macroexpansion time regardless
/// of whether the type check ever fails at runtime; unconditionally
/// generating a `SETF` would make `CHECK-TYPE` fail to compile for exactly
/// the common case where nothing is ever stored. A full fix threads
/// `GET-SETF-EXPANSION` through this macro so the store only needs to be
/// valid on the branch where it can run, which is a larger change than this
/// lane's C1 scope.
pub(super) fn expand_check_type(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result {
    let mut scope = Scope::new(ctx);
    let place = word_handle(&mut scope, *values.first().ok_or(ObjectError::TypeError)?);
    let type_spec = word_handle(&mut scope, *values.get(1).ok_or(ObjectError::TypeError)?);

    let quoted_type = form(&mut scope, runtime, "QUOTE", &[type_spec])?;
    let typep = form(&mut scope, runtime, "TYPEP", &[place, quoted_type])?;
    let error_condition = build_type_error_form(&mut scope, runtime, place, quoted_type)?;
    let store_value_variable = fresh(&mut scope, runtime)?;
    let lambda_list = cons_list(&mut scope, runtime, &[store_value_variable])?;
    let restart_function = form(
        &mut scope,
        runtime,
        "LAMBDA",
        &[lambda_list, store_value_variable],
    )?;
    let restart_name = intern(&mut scope, runtime, "STORE-VALUE")?;
    let clause = cons_list(&mut scope, runtime, &[restart_name, restart_function])?;
    let restart_bind_clauses = cons_list(&mut scope, runtime, &[clause])?;
    let signal_error = form(&mut scope, runtime, "ERROR", &[error_condition])?;
    let restart_bind_form = form(
        &mut scope,
        runtime,
        "RESTART-BIND",
        &[restart_bind_clauses, signal_error],
    )?;
    let unless_form = form(&mut scope, runtime, "UNLESS", &[typep, restart_bind_form])?;
    Ok(scope.get(unless_form).as_word())
}

fn build_type_error_form<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    datum: Handle<'ctx>,
    expected_type: Handle<'ctx>,
) -> std::result::Result<Handle<'ctx>, ObjectError> {
    let type_error_symbol = intern(scope, runtime, "TYPE-ERROR")?;
    let quoted_type_error = form(scope, runtime, "QUOTE", &[type_error_symbol])?;
    let datum_keyword = intern_keyword(scope, runtime, "DATUM")?;
    let expected_keyword = intern_keyword(scope, runtime, "EXPECTED-TYPE")?;
    form(
        scope,
        runtime,
        "MAKE-CONDITION",
        &[
            quoted_type_error,
            datum_keyword,
            datum,
            expected_keyword,
            expected_type,
        ],
    )
}

/// Expand `(assert test-form (place*) datum-form*)` (the `(place*)` and
/// `datum-form*` are both optional) into a loop-free `UNLESS` that signals
/// an error offering a `CONTINUE` restart. Phase 1 simplification: like
/// `CHECK-TYPE`, this does not automatically re-test `test-form` after
/// `CONTINUE`; continuing simply resumes after the `ASSERT` form.
pub(super) fn expand_assert(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    let mut scope = Scope::new(ctx);
    let test_form = word_handle(&mut scope, *values.first().ok_or(ObjectError::TypeError)?);
    let has_datum = values.len() > 2;
    let error_call = if has_datum {
        let datum_words: Vec<Local<'_>> = values
            .get(2..)
            .ok_or(ObjectError::TypeError)?
            .iter()
            .copied()
            .map(Local::from_word)
            .collect();
        let datum_handles = scope.root_many(&datum_words);
        form(&mut scope, runtime, "ERROR", datum_handles.as_slice())?
    } else {
        let message = ncl_object::make_string(
            scope.context_mut(),
            runtime,
            &"assertion failed".chars().collect::<Vec<_>>(),
        )?;
        let message_handle = word_handle(&mut scope, message);
        form(&mut scope, runtime, "ERROR", &[message_handle])?
    };
    let continue_name = intern(&mut scope, runtime, "CONTINUE")?;
    let empty_lambda_list = word_handle(&mut scope, Word::NIL);
    let nil_body = word_handle(&mut scope, Word::NIL);
    let clause = cons_list(
        &mut scope,
        runtime,
        &[continue_name, empty_lambda_list, nil_body],
    )?;
    // Call `expand_restart_case` directly rather than building a
    // `(RESTART-CASE ...)` form for later re-expansion: `RESTART-CASE`'s own
    // clauses are separate top-level arguments (unlike `RESTART-BIND`'s,
    // which are one wrapped list), so this needs exactly
    // `[protected-form, clause]`, not a clause further wrapped in a list.
    let restart_case_words = [scope.get(error_call).as_word(), scope.get(clause).as_word()];
    let restart_case_word = expand_restart_case(scope.context_mut(), runtime, &restart_case_words)?;
    let restart_case_form = word_handle(&mut scope, restart_case_word);
    let unless_form = form(
        &mut scope,
        runtime,
        "UNLESS",
        &[test_form, restart_case_form],
    )?;
    Ok(scope.get(unless_form).as_word())
}
