//! Structural expansion of `DECLAIM`.

use super::{args, form};
use crate::elements;
use ncl_object::{
    BuiltinArgs, Local, MultipleValues, ObjectError, ObjectRef, Runtime, Scope, ThreadContext,
    Word, classify_object,
};

type Result<T = Word> = std::result::Result<T, ObjectError>;

/// Expand declaration specifiers into quoted `PROCLAIM` forms.
pub(crate) fn expand_declaim(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result {
    let mut scope = Scope::new(ctx);
    let specifier_handles = scope.root_many(
        &values
            .iter()
            .copied()
            .map(Local::<Word>::from_word)
            .collect::<Vec<_>>(),
    );
    let mut proclamation_handles = Vec::with_capacity(specifier_handles.len());

    for specifier_handle in specifier_handles.iter() {
        let specifier = scope.get(*specifier_handle).as_word();
        let parts = elements(scope.context_mut(), specifier)?;
        let head = parts.first().copied().ok_or(ObjectError::TypeError)?;
        if !matches!(
            classify_object(scope.context_mut(), head),
            ObjectRef::Symbol(_)
        ) {
            return Err(ObjectError::TypeError);
        }

        let quoted = form(scope.context_mut(), runtime, "QUOTE", &[specifier])?;
        let quoted_handle = scope.root(Local::<Word>::from_word(quoted));
        let quoted_word = scope.get(quoted_handle).as_word();
        let proclamation = form(scope.context_mut(), runtime, "PROCLAIM", &[quoted_word])?;
        proclamation_handles.push(scope.root(Local::<Word>::from_word(proclamation)));
    }

    let proclamations = proclamation_handles
        .iter()
        .map(|handle| scope.get(*handle).as_word())
        .collect::<Vec<_>>();
    form(scope.context_mut(), runtime, "PROGN", &proclamations)
}

/// Adapt the macro-call ABI to [`expand_declaim`].
pub(crate) fn expand_declaim_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    input: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result {
    let form = input.get(0).ok_or(ObjectError::TypeError)?;
    let arguments = args(ctx, form)?;
    expand_declaim(ctx, runtime, &arguments)
}
