use super::{bindings, form, is_otherwise, progn};
use crate::{elements, fresh_symbol, list, symbol};
use ncl_object::{Handle, Local, ObjectError, Runtime, Scope, ThreadContext, Word};

type Result<T = Word> = std::result::Result<T, ObjectError>;

pub(crate) fn expand(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
    errorp: bool,
) -> Result {
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        let mut scope = Scope::new(ctx);
        let value: Handle<'_, Word> = scope.root(Local::from_word(
            **roots.first().ok_or(ObjectError::TypeError)?,
        ));
        let clauses: Vec<Handle<'_, Word>> = roots
            .get(1..)
            .ok_or(ObjectError::TypeError)?
            .iter()
            .map(|clause| scope.root(Local::<Word>::from_word(**clause)))
            .collect::<Vec<_>>();
        let fresh = fresh_symbol(scope.context_mut(), runtime)?;
        let temporary: Handle<'_, Word> = scope.root(Local::from_word(fresh));
        let mut branches: Vec<(Handle<'_, Word>, Handle<'_, Word>)> = Vec::new();
        let mut types: Vec<Handle<'_, Word>> = Vec::new();
        let mut default_body = None;
        for clause in clauses {
            let clause_word = scope.get(clause).as_word();
            let parts = elements(scope.context_mut(), clause_word)?;
            let type_specifier = *parts.first().ok_or(ObjectError::TypeError)?;
            let body = progn(
                scope.context_mut(),
                runtime,
                parts.get(1..).ok_or(ObjectError::TypeError)?,
            )?;
            let body = scope.root(Local::from_word(body));
            if is_otherwise(scope.context_mut(), type_specifier)? {
                default_body = Some(body);
                continue;
            }
            let type_handle = scope.root(Local::from_word(type_specifier));
            let type_word = scope.get(type_handle).as_word();
            let quoted_type = form(scope.context_mut(), runtime, "QUOTE", &[type_word])?;
            let temporary_word = scope.get(temporary).as_word();
            let type_test = form(
                scope.context_mut(),
                runtime,
                "TYPEP",
                &[temporary_word, quoted_type],
            )?;
            let type_test = scope.root(Local::from_word(type_test));
            branches.push((type_test, body));
            types.push(type_handle);
        }
        let fallback = if let Some(body) = default_body {
            scope.get(body).as_word()
        } else if errorp {
            let mut expected_types = vec![symbol(scope.context_mut(), runtime, "OR")?];
            expected_types.extend(types.iter().map(|ty| scope.get(*ty).as_word()));
            let expected_type = list(scope.context_mut(), runtime, &expected_types)?;
            let expected = form(scope.context_mut(), runtime, "QUOTE", &[expected_type])?;
            let type_error = symbol(scope.context_mut(), runtime, "TYPE-ERROR")?;
            let error_type = form(scope.context_mut(), runtime, "QUOTE", &[type_error])?;
            let datum = symbol(scope.context_mut(), runtime, ":DATUM")?;
            let expected_type = symbol(scope.context_mut(), runtime, ":EXPECTED-TYPE")?;
            let temporary_word = scope.get(temporary).as_word();
            form(
                scope.context_mut(),
                runtime,
                "ERROR",
                &[error_type, datum, temporary_word, expected_type, expected],
            )?
        } else {
            Word::NIL
        };
        let mut branch: Handle<'_, Word> = scope.root(Local::from_word(fallback));
        for (test, body) in branches.into_iter().rev() {
            let test_word = scope.get(test).as_word();
            let body_word = scope.get(body).as_word();
            let branch_word = scope.get(branch).as_word();
            let next = form(
                scope.context_mut(),
                runtime,
                "IF",
                &[test_word, body_word, branch_word],
            )?;
            branch = scope.root(Local::from_word(next));
        }
        let temporary_word = scope.get(temporary).as_word();
        let value_word = scope.get(value).as_word();
        let binding = bindings(
            scope.context_mut(),
            runtime,
            &[(temporary_word, value_word)],
        )?;
        let branch_word = scope.get(branch).as_word();
        form(scope.context_mut(), runtime, "LET", &[binding, branch_word])
    })
}
