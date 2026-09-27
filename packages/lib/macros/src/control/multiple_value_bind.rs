use crate::{elements, fresh_symbol, list, symbol};
use ncl_object::{
    Local, ObjectError, ObjectRef, Runtime, Scope, ThreadContext, Word, classify_object,
};

type Result<T = Word> = std::result::Result<T, ObjectError>;

pub(super) fn expand(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    // check-added-lines: allow(index) intentional
    let [variables, value, body @ ..] = values else {
        return Err(ObjectError::TypeError);
    };
    let mut scope = Scope::new(ctx);
    let variables = elements(scope.context_mut(), *variables)?;
    for variable in &variables {
        if !matches!(
            classify_object(scope.context_mut(), *variable),
            ObjectRef::Symbol(_)
        ) {
            return Err(ObjectError::TypeError);
        }
    }
    let variable_handles = scope.root_many(
        &variables
            .iter()
            .copied()
            .map(Local::<Word>::from_word)
            .collect::<Vec<_>>(),
    );
    let value_handle = scope.root(Local::<Word>::from_word(*value));
    let body_handles = scope.root_many(
        &body
            .iter()
            .copied()
            .map(Local::<Word>::from_word)
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
    let body_values = body_handles
        .iter()
        .map(|handle| scope.get(*handle).as_word())
        .collect::<Vec<_>>();
    let body = super::progn(scope.context_mut(), runtime, &body_values)?;
    let lambda_form = super::form(scope.context_mut(), runtime, "LAMBDA", &[lambda, body])?;
    let value = scope.get(value_handle).as_word();
    super::form(
        scope.context_mut(),
        runtime,
        "MULTIPLE-VALUE-CALL",
        &[lambda_form, value],
    )
}
