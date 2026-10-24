//! `case`/`ecase`/`ccase` (CLHS 7.6, "Data and Control Flow").

use super::logical::or;
use super::{bindings, form, is_otherwise, progn};
use crate::{elements, fresh_symbol, list, symbol};
use ncl_object::{HeapRootedWord, HeapRootedWords, ObjectError, Runtime, ThreadContext, Word};

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
    let value = values.first().ok_or(ObjectError::TypeError)?;
    let temporary_word = fresh_symbol(ctx, runtime)?;
    let temporary = HeapRootedWord::new(ctx, temporary_word)?;
    let let_bindings = bindings(ctx, runtime, &[(temporary.get(), value.get())])?;
    let let_bindings = HeapRootedWord::new(ctx, let_bindings)?;

    // Pre-scan clauses to collect every key named, so a `case` compiled with
    // `errorp` can report the accepted values as a `(member ...)` type.
    let mut all_keys = HeapRootedWords::default();
    let mut has_otherwise = false;
    for clause in values.get(1..).ok_or(ObjectError::TypeError)? {
        let parts = elements(ctx, clause.get())?;
        let keys = parts.first().copied().ok_or(ObjectError::TypeError)?;
        if is_otherwise(ctx, keys)? {
            has_otherwise = true;
            continue;
        }
        if keys.is_cons() {
            for key in elements(ctx, keys)? {
                all_keys.push(ctx, key)?;
            }
        } else {
            all_keys.push(ctx, keys)?;
        }
    }

    let branches = if errorp && !has_otherwise {
        type_error_branch(ctx, runtime, &temporary, &all_keys)?
    } else {
        HeapRootedWord::new(ctx, Word::NIL)?
    };

    for clause in values.get(1..).ok_or(ObjectError::TypeError)?.iter().rev() {
        let parts = elements(ctx, clause.get())?;
        let parts = HeapRootedWords::new(ctx, &parts)?;
        let keys = parts.first().ok_or(ObjectError::TypeError)?;
        let body_values = parts
            .as_slice()
            .get(1..)
            .ok_or(ObjectError::TypeError)?
            .iter()
            .map(HeapRootedWord::get)
            .collect::<Vec<_>>();
        let body = progn(ctx, runtime, &body_values)?;
        let body = HeapRootedWord::new(ctx, body)?;
        if is_otherwise(ctx, keys.get())? {
            let true_symbol = symbol(ctx, runtime, "T")?;
            let branch = form(
                ctx,
                runtime,
                "IF",
                &[true_symbol, body.get(), branches.get()],
            )?;
            branches.set(branch);
            continue;
        }

        let key_values = if keys.get().is_cons() {
            elements(ctx, keys.get())?
        } else {
            vec![keys.get()]
        };
        let key_values = HeapRootedWords::new(ctx, &key_values)?;
        let mut tests = HeapRootedWords::default();
        for key in key_values.iter() {
            let quote = form(ctx, runtime, "QUOTE", &[key.get()])?;
            let quote = HeapRootedWord::new(ctx, quote)?;
            let test = form(ctx, runtime, "EQL", &[temporary.get(), quote.get()])?;
            tests.push(ctx, test)?;
        }
        let test = or(ctx, runtime, tests.as_slice())?;
        let test = HeapRootedWord::new(ctx, test)?;
        let branch = form(
            ctx,
            runtime,
            "IF",
            &[test.get(), body.get(), branches.get()],
        )?;
        branches.set(branch);
    }

    form(ctx, runtime, "LET", &[let_bindings.get(), branches.get()])
}

fn type_error_branch(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    temporary: &HeapRootedWord,
    all_keys: &HeapRootedWords,
) -> Result<HeapRootedWord> {
    let member = symbol(ctx, runtime, "MEMBER")?;
    let mut expected_types = HeapRootedWords::new(ctx, &[member])?;
    for key in all_keys.iter() {
        expected_types.push(ctx, key.get())?;
    }
    let expected_values = expected_types
        .iter()
        .map(HeapRootedWord::get)
        .collect::<Vec<_>>();
    let expected_type = list(ctx, runtime, &expected_values)?;
    let expected_type = HeapRootedWord::new(ctx, expected_type)?;
    let expected = form(ctx, runtime, "QUOTE", &[expected_type.get()])?;
    let expected = HeapRootedWord::new(ctx, expected)?;
    let type_error = symbol(ctx, runtime, "TYPE-ERROR")?;
    let error_type = form(ctx, runtime, "QUOTE", &[type_error])?;
    let error_type = HeapRootedWord::new(ctx, error_type)?;
    let datum = symbol(ctx, runtime, "KEYWORD::DATUM")?;
    let datum = HeapRootedWord::new(ctx, datum)?;
    let expected_type_kw = symbol(ctx, runtime, "KEYWORD::EXPECTED-TYPE")?;
    let expected_type_kw = HeapRootedWord::new(ctx, expected_type_kw)?;
    let branch = form(
        ctx,
        runtime,
        "ERROR",
        &[
            error_type.get(),
            datum.get(),
            temporary.get(),
            expected_type_kw.get(),
            expected.get(),
        ],
    )?;
    HeapRootedWord::new(ctx, branch)
}
