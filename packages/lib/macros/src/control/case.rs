//! `case`/`ecase`/`ccase` (CLHS 7.6, "Data and Control Flow").

use super::{bindings, form, is_otherwise, progn};
use crate::{elements, fresh_symbol, list, symbol};
use ncl_object::{Handle, HeapRootedWord, Local, ObjectError, Runtime, Scope, ThreadContext, Word};

type Result<T = Word> = std::result::Result<T, ObjectError>;

/// Expand `case`, and, when `errorp` (`ecase`/`ccase`), signal a
/// `type-error` naming the accepted keys as a `(member ...)` type instead of
/// silently returning `nil` when nothing matches.
pub(super) fn case(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[HeapRootedWord],
    errorp: bool,
) -> Result {
    let (value, clauses) = values.split_first().ok_or(ObjectError::TypeError)?;
    let mut scope = Scope::new(ctx);
    let temporary_word = fresh_symbol(scope.context_mut(), runtime)?;
    let temporary = scope.root(Local::<Word>::from_word(temporary_word));
    let temporary_word = scope.get(temporary).as_word();
    let let_bindings = bindings(
        scope.context_mut(),
        runtime,
        &[(temporary_word, value.get())],
    )?;
    let let_bindings = scope.root(Local::<Word>::from_word(let_bindings));

    let (all_keys, parsed_clauses, has_otherwise) = prescan_clauses(&mut scope, clauses)?;

    let branches = if errorp && !has_otherwise {
        type_error_branch(&mut scope, runtime, temporary, &all_keys)?
    } else {
        scope.root(Local::<Word>::from_word(Word::NIL))
    };

    for clause in parsed_clauses.iter().rev() {
        let body_values = clause
            .body
            .iter()
            .map(|body| scope.get(*body).as_word())
            .collect::<Vec<_>>();
        let body_word = progn(scope.context_mut(), runtime, &body_values)?;
        let body = scope.root(Local::<Word>::from_word(body_word));
        let key_word = scope.get(clause.keys).as_word();
        if is_otherwise(scope.context(), key_word)? {
            let true_symbol = symbol(scope.context_mut(), runtime, "T")?;
            let body_word = scope.get(body).as_word();
            let branches_word = scope.get(branches).as_word();
            let branch = form(
                scope.context_mut(),
                runtime,
                "IF",
                &[true_symbol, body_word, branches_word],
            )?;
            scope.set(branches, Local::<Word>::from_word(branch));
            continue;
        }

        let key_values = if key_word.is_cons() {
            elements(scope.context_mut(), key_word)?
        } else {
            vec![key_word]
        };
        let key_values = key_values
            .into_iter()
            .map(|key| scope.root(Local::<Word>::from_word(key)))
            .collect::<Vec<_>>();
        let mut tests = Vec::with_capacity(key_values.len());
        for key in key_values {
            let key_word = scope.get(key).as_word();
            let quote = form(scope.context_mut(), runtime, "QUOTE", &[key_word])?;
            let quote = scope.root(Local::<Word>::from_word(quote));
            let temporary_word = scope.get(temporary).as_word();
            let quote_word = scope.get(quote).as_word();
            let test = form(
                scope.context_mut(),
                runtime,
                "EQL",
                &[temporary_word, quote_word],
            )?;
            tests.push(scope.root(Local::<Word>::from_word(test)));
        }
        let test = or_tests(&mut scope, runtime, &tests)?;
        let test = scope.root(Local::<Word>::from_word(test));
        let test_word = scope.get(test).as_word();
        let body_word = scope.get(body).as_word();
        let branches_word = scope.get(branches).as_word();
        let branch = form(
            scope.context_mut(),
            runtime,
            "IF",
            &[test_word, body_word, branches_word],
        )?;
        scope.set(branches, Local::<Word>::from_word(branch));
    }

    let let_bindings_word = scope.get(let_bindings).as_word();
    let branches_word = scope.get(branches).as_word();
    form(
        scope.context_mut(),
        runtime,
        "LET",
        &[let_bindings_word, branches_word],
    )
}

fn prescan_clauses<'scope>(
    scope: &mut Scope<'scope>,
    clauses: &[HeapRootedWord],
) -> Result<(Vec<Handle<'scope, Word>>, Vec<ParsedClause<'scope>>, bool)> {
    // Keep parsed values rooted while the remaining clauses are inspected.
    let mut all_keys = Vec::new();
    let mut parsed_clauses = Vec::with_capacity(clauses.len());
    let mut has_otherwise = false;
    for clause in clauses {
        let parts = elements(scope.context_mut(), clause.get())?;
        let (key_word, body_words) = parts.split_first().ok_or(ObjectError::TypeError)?;
        let keys = scope.root(Local::<Word>::from_word(*key_word));
        let body = body_words
            .iter()
            .copied()
            .map(|word| scope.root(Local::<Word>::from_word(word)))
            .collect::<Vec<_>>();
        let key_word = scope.get(keys).as_word();
        if is_otherwise(scope.context(), key_word)? {
            has_otherwise = true;
        } else if key_word.is_cons() {
            for key in elements(scope.context_mut(), key_word)? {
                all_keys.push(scope.root(Local::<Word>::from_word(key)));
            }
        } else {
            all_keys.push(scope.root(Local::<Word>::from_word(key_word)));
        }
        parsed_clauses.push(ParsedClause { keys, body });
    }
    Ok((all_keys, parsed_clauses, has_otherwise))
}

fn or_tests<'scope>(
    scope: &mut Scope<'scope>,
    runtime: &Runtime,
    values: &[Handle<'scope, Word>],
) -> Result {
    match values {
        [] => Ok(Word::NIL),
        [value] => Ok(scope.get(*value).as_word()),
        [value, rest @ ..] => {
            let temporary_word = fresh_symbol(scope.context_mut(), runtime)?;
            let temporary = scope.root(Local::<Word>::from_word(temporary_word));
            let temporary_word = scope.get(temporary).as_word();
            let value_word = scope.get(*value).as_word();
            let binding = bindings(
                scope.context_mut(),
                runtime,
                &[(temporary_word, value_word)],
            )?;
            let binding = scope.root(Local::<Word>::from_word(binding));
            let next = or_tests(scope, runtime, rest)?;
            let next = scope.root(Local::<Word>::from_word(next));
            let temporary_word = scope.get(temporary).as_word();
            let next_word = scope.get(next).as_word();
            let test = form(
                scope.context_mut(),
                runtime,
                "IF",
                &[temporary_word, temporary_word, next_word],
            )?;
            let test = scope.root(Local::<Word>::from_word(test));
            let binding_word = scope.get(binding).as_word();
            let test_word = scope.get(test).as_word();
            form(
                scope.context_mut(),
                runtime,
                "LET",
                &[binding_word, test_word],
            )
        }
    }
}

struct ParsedClause<'scope> {
    keys: Handle<'scope, Word>,
    body: Vec<Handle<'scope, Word>>,
}

fn type_error_branch<'scope>(
    scope: &mut Scope<'scope>,
    runtime: &Runtime,
    temporary: Handle<'scope, Word>,
    all_keys: &[Handle<'scope, Word>],
) -> Result<Handle<'scope, Word>> {
    let member = symbol(scope.context_mut(), runtime, "MEMBER")?;
    let member = scope.root(Local::<Word>::from_word(member));
    let mut expected_values = vec![scope.get(member).as_word()];
    for key in all_keys {
        expected_values.push(scope.get(*key).as_word());
    }
    let expected_type = list(scope.context_mut(), runtime, &expected_values)?;
    let expected_type = scope.root(Local::<Word>::from_word(expected_type));
    let expected_type_word = scope.get(expected_type).as_word();
    let expected = form(scope.context_mut(), runtime, "QUOTE", &[expected_type_word])?;
    let expected = scope.root(Local::<Word>::from_word(expected));
    let type_error = symbol(scope.context_mut(), runtime, "TYPE-ERROR")?;
    let error_type = form(scope.context_mut(), runtime, "QUOTE", &[type_error])?;
    let error_type = scope.root(Local::<Word>::from_word(error_type));
    let datum = symbol(scope.context_mut(), runtime, "KEYWORD::DATUM")?;
    let datum = scope.root(Local::<Word>::from_word(datum));
    let expected_type_kw = symbol(scope.context_mut(), runtime, "KEYWORD::EXPECTED-TYPE")?;
    let expected_type_kw = scope.root(Local::<Word>::from_word(expected_type_kw));
    let error_type_word = scope.get(error_type).as_word();
    let datum_word = scope.get(datum).as_word();
    let temporary_word = scope.get(temporary).as_word();
    let expected_type_kw_word = scope.get(expected_type_kw).as_word();
    let expected_word = scope.get(expected).as_word();
    let branch = form(
        scope.context_mut(),
        runtime,
        "ERROR",
        &[
            error_type_word,
            datum_word,
            temporary_word,
            expected_type_kw_word,
            expected_word,
        ],
    )?;
    Ok(scope.root(Local::<Word>::from_word(branch)))
}

#[cfg(test)]
#[path = "case/tests.rs"]
mod tests;
