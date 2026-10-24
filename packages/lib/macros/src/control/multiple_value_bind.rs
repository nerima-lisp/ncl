use crate::{elements, fresh_symbol, list, symbol};
use ncl_object::{
    HeapRootedWords, Local, ObjectError, ObjectRef, Runtime, Scope, ThreadContext, Word,
    classify_object,
};

type Result<T = Word> = std::result::Result<T, ObjectError>;

pub(super) fn expand(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[ncl_object::HeapRootedWord],
) -> Result {
    // check-added-lines: allow(index) intentional
    let [variables, value, body @ ..] = values else {
        return Err(ObjectError::TypeError);
    };
    let mut scope = Scope::new(ctx);
    let variable_words = elements(scope.context_mut(), variables.get())?;
    let variables = HeapRootedWords::new(scope.context_mut(), &variable_words)?;
    for variable in variables.iter() {
        if !matches!(
            classify_object(scope.context_mut(), variable.get()),
            ObjectRef::Symbol(_)
        ) {
            return Err(ObjectError::TypeError);
        }
    }
    let variable_handles = scope.root_many(
        &variables
            .iter()
            .map(|variable| Local::<Word>::from_word(variable.get()))
            .collect::<Vec<_>>(),
    );
    let value_handle = scope.root(Local::<Word>::from_word(value.get()));
    let body_handles = scope.root_many(
        &body
            .iter()
            .map(|form| Local::<Word>::from_word(form.get()))
            .collect::<Vec<_>>(),
    );
    let rest = fresh_symbol(scope.context_mut(), runtime)?;
    let rest_handle = scope.root(Local::<Word>::from_word(rest));
    let rest_marker = symbol(scope.context_mut(), runtime, "&REST")?;
    let rest_marker_handle = scope.root(Local::<Word>::from_word(rest_marker));
    let mut lambda_values = variable_handles
        .iter()
        .map(|handle| scope.get(*handle).as_word())
        .collect::<Vec<_>>();
    lambda_values.push(scope.get(rest_marker_handle).as_word());
    lambda_values.push(scope.get(rest_handle).as_word());
    let lambda = list(scope.context_mut(), runtime, &lambda_values)?;
    let lambda_handle = scope.root(Local::<Word>::from_word(lambda));
    let body_values = body_handles
        .iter()
        .map(|handle| scope.get(*handle).as_word())
        .collect::<Vec<_>>();
    let body = super::progn(scope.context_mut(), runtime, &body_values)?;
    let lambda = scope.get(lambda_handle).as_word();
    let lambda_form = super::form(scope.context_mut(), runtime, "LAMBDA", &[lambda, body])?;
    let value = scope.get(value_handle).as_word();
    super::form(
        scope.context_mut(),
        runtime,
        "MULTIPLE-VALUE-CALL",
        &[lambda_form, value],
    )
}
