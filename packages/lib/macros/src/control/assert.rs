//! Expansion of the standard `ASSERT` and `CHECK-TYPE` macros.

use super::{args, form};
use crate::{list, symbol};
use ncl_object::{
    BuiltinArgs, Handle, Local, MultipleValues, ObjectError, Runtime, Scope, ThreadContext, Word,
};

type Result<T = Word> = std::result::Result<T, ObjectError>;

fn rooted_form<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    operator: &str,
    arguments: &[Word],
) -> Result<Handle<'ctx, Word>> {
    let value = form(scope.context_mut(), runtime, operator, arguments)?;
    Ok(scope.root(Local::from_word(value)))
}

fn quote<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    value: Word,
) -> Result<Handle<'ctx, Word>> {
    rooted_form(scope, runtime, "QUOTE", &[value])
}

fn expand_assert_values(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        let mut scope = Scope::new(ctx);
        let test: Handle<'_, Word> = scope.root(Local::from_word(
            **roots.first().ok_or(ObjectError::TypeError)?,
        ));
        let datum = roots.get(2).map_or(Word::NIL, |value| **value);
        let error = rooted_form(&mut scope, runtime, "ERROR", &[datum])?;
        let test_word = scope.get(test).as_word();
        let error_word = scope.get(error).as_word();
        let result = rooted_form(
            &mut scope,
            runtime,
            "IF",
            &[test_word, Word::NIL, error_word],
        )?;
        Ok(scope.get(result).as_word())
    })
}

pub(crate) fn expand_assert(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    if values.is_empty() {
        return Err(ObjectError::TypeError);
    }
    expand_assert_values(ctx, runtime, values)
}

pub(crate) fn expand_check_type(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result {
    if !(2..=3).contains(&values.len()) {
        return Err(ObjectError::TypeError);
    }
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        let mut scope = Scope::new(ctx);
        let place: Handle<'_, Word> = scope.root(Local::from_word(
            **roots.first().ok_or(ObjectError::TypeError)?,
        ));
        let type_specifier: Handle<'_, Word> = scope.root(Local::from_word(
            **roots.get(1).ok_or(ObjectError::TypeError)?,
        ));
        let type_specifier_word = scope.get(type_specifier).as_word();
        let quoted_type = quote(&mut scope, runtime, type_specifier_word)?;
        let place_word = scope.get(place).as_word();
        let quoted_type_word = scope.get(quoted_type).as_word();
        let type_test = rooted_form(
            &mut scope,
            runtime,
            "TYPEP",
            &[place_word, quoted_type_word],
        )?;
        let place_list = list(scope.context_mut(), runtime, &[place_word])?;
        let places: Handle<'_, Word> = scope.root(Local::from_word(place_list));

        let condition_name = if roots.get(2).is_some() {
            "SIMPLE-TYPE-ERROR"
        } else {
            "TYPE-ERROR"
        };
        let condition = symbol(scope.context_mut(), runtime, condition_name)?;
        let condition = quote(&mut scope, runtime, condition)?;
        let datum_keyword = symbol(scope.context_mut(), runtime, ":DATUM")?;
        let expected_keyword = symbol(scope.context_mut(), runtime, ":EXPECTED-TYPE")?;
        let mut assert_values = vec![
            scope.get(type_test).as_word(),
            scope.get(places).as_word(),
            scope.get(condition).as_word(),
            datum_keyword,
            scope.get(place).as_word(),
            expected_keyword,
            scope.get(quoted_type).as_word(),
        ];
        if let Some(type_string) = roots.get(2) {
            let format_keyword = symbol(scope.context_mut(), runtime, ":FORMAT-CONTROL")?;
            assert_values.extend([format_keyword, **type_string]);
        }
        expand_assert_values(scope.context_mut(), runtime, &assert_values)
    })
}

pub(crate) fn expand_assert_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    input: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result {
    let form = input.get(0).ok_or(ObjectError::TypeError)?;
    let values = args(ctx, form)?;
    expand_assert(ctx, runtime, &values)
}

pub(crate) fn expand_check_type_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    input: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result {
    let form = input.get(0).ok_or(ObjectError::TypeError)?;
    let values = args(ctx, form)?;
    expand_check_type(ctx, runtime, &values)
}
