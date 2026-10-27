use super::{
    BuiltinArgs, BuiltinFunctionCaller, MultipleValues, ObjectError, Runtime, Scope, ThreadContext,
    Word, domain, keep_result, root_args, words,
};

#[derive(Clone, Copy)]
enum TreeSubstitutionMode {
    Equal { destructive: bool },
    Predicate { destructive: bool, negate: bool },
    Alist { destructive: bool },
}

fn apply_tree_substitution(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut BuiltinFunctionCaller,
    positional: &[Word],
    options: domain::selection::SelectionOptions,
    mode: TreeSubstitutionMode,
) -> Result<Word, ObjectError> {
    // `tree_substitution_entry` parsed the required positional prefix.
    match mode {
        TreeSubstitutionMode::Equal { destructive } => {
            let replacement = *positional.first().ok_or(ObjectError::TypeError)?;
            let old = *positional.get(1).ok_or(ObjectError::TypeError)?;
            let tree = *positional.get(2).ok_or(ObjectError::TypeError)?;
            if destructive {
                domain::list_substitution::nsubst(
                    ctx,
                    runtime,
                    caller,
                    replacement,
                    old,
                    tree,
                    options,
                )
            } else {
                domain::list_substitution::subst(
                    ctx,
                    runtime,
                    caller,
                    replacement,
                    old,
                    tree,
                    options,
                )
            }
        }
        TreeSubstitutionMode::Predicate {
            destructive,
            negate,
        } => {
            let replacement = *positional.first().ok_or(ObjectError::TypeError)?;
            let predicate = *positional.get(1).ok_or(ObjectError::TypeError)?;
            let tree = *positional.get(2).ok_or(ObjectError::TypeError)?;
            match (destructive, negate) {
                (false, false) => domain::list_substitution::subst_if(
                    ctx,
                    runtime,
                    caller,
                    replacement,
                    predicate,
                    tree,
                    options,
                ),
                (false, true) => domain::list_substitution::subst_if_not(
                    ctx,
                    runtime,
                    caller,
                    replacement,
                    predicate,
                    tree,
                    options,
                ),
                (true, false) => domain::list_substitution::nsubst_if(
                    ctx,
                    runtime,
                    caller,
                    replacement,
                    predicate,
                    tree,
                    options,
                ),
                (true, true) => domain::list_substitution::nsubst_if_not(
                    ctx,
                    runtime,
                    caller,
                    replacement,
                    predicate,
                    tree,
                    options,
                ),
            }
        }
        TreeSubstitutionMode::Alist { destructive } => {
            let alist = *positional.first().ok_or(ObjectError::TypeError)?;
            let tree = *positional.get(1).ok_or(ObjectError::TypeError)?;
            if destructive {
                domain::list_substitution::nsublis(ctx, runtime, caller, alist, tree, options)
            } else {
                domain::list_substitution::sublis(ctx, runtime, caller, alist, tree, options)
            }
        }
    }
}

fn tree_substitution_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
    mode: TreeSubstitutionMode,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let handles = root_args(&mut scope, args)?;
    let rooted = words(&scope, &handles);
    let minimum = match mode {
        TreeSubstitutionMode::Alist { .. } => 2,
        TreeSubstitutionMode::Equal { .. } | TreeSubstitutionMode::Predicate { .. } => 3,
    };
    let (positional, parsed_options) = domain::selection::parse_options_with_minimum(
        scope.context_mut(),
        runtime,
        &rooted,
        minimum,
        domain::selection::SelectionOptionMode::Tree,
    )?;
    let options = parsed_options.selection;
    if matches!(mode, TreeSubstitutionMode::Predicate { .. })
        && (options.test.is_some() || options.test_not.is_some())
    {
        return Err(ObjectError::TypeError);
    }
    let mut caller = BuiltinFunctionCaller;
    let result = apply_tree_substitution(
        scope.context_mut(),
        runtime,
        &mut caller,
        &positional,
        options,
        mode,
    )?;
    values.clear();
    keep_result(&mut scope, Ok(result))
}

pub fn subst_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    tree_substitution_entry(
        ctx,
        runtime,
        args,
        values,
        TreeSubstitutionMode::Equal { destructive: false },
    )
}

pub fn subst_if_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    tree_substitution_entry(
        ctx,
        runtime,
        args,
        values,
        TreeSubstitutionMode::Predicate {
            destructive: false,
            negate: false,
        },
    )
}

pub fn subst_if_not_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    tree_substitution_entry(
        ctx,
        runtime,
        args,
        values,
        TreeSubstitutionMode::Predicate {
            destructive: false,
            negate: true,
        },
    )
}

pub fn nsubst_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    tree_substitution_entry(
        ctx,
        runtime,
        args,
        values,
        TreeSubstitutionMode::Equal { destructive: true },
    )
}

pub fn nsubst_if_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    tree_substitution_entry(
        ctx,
        runtime,
        args,
        values,
        TreeSubstitutionMode::Predicate {
            destructive: true,
            negate: false,
        },
    )
}

pub fn nsubst_if_not_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    tree_substitution_entry(
        ctx,
        runtime,
        args,
        values,
        TreeSubstitutionMode::Predicate {
            destructive: true,
            negate: true,
        },
    )
}

pub fn sublis_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    tree_substitution_entry(
        ctx,
        runtime,
        args,
        values,
        TreeSubstitutionMode::Alist { destructive: false },
    )
}

pub fn nsublis_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    tree_substitution_entry(
        ctx,
        runtime,
        args,
        values,
        TreeSubstitutionMode::Alist { destructive: true },
    )
}

#[cfg(test)]
#[path = "../tests/support/tree_substitution_internal.rs"]
mod tests;
