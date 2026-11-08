use super::{
    MethodCombination, MultipleValues, ObjectError, Runtime, Scope, Word, dispatch_list_to_handles,
    dispatch_method_match, dispatch_push_handle, lisp_equal_word,
};

pub struct ApplicableMethods {
    pub before: Vec<Word>,
    pub primary: Vec<Word>,
    pub after: Vec<Word>,
    pub around: Vec<Word>,
}

pub fn collect_applicable_methods<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    combination: ncl_object::Handle<'ctx>,
    combination_kind: MethodCombination,
    order: ncl_object::Handle<'ctx>,
    arguments: &ncl_object::HandleVec<'ctx>,
    entries: &ncl_object::HandleVec<'ctx>,
) -> Result<ApplicableMethods, ObjectError> {
    let around_qualifier = scope.intern(runtime, "KEYWORD", "AROUND")?;
    let before_qualifier = scope.intern(runtime, "KEYWORD", "BEFORE")?;
    let after_qualifier = scope.intern(runtime, "KEYWORD", "AFTER")?;
    let argument_words = arguments
        .iter()
        .map(|handle| scope.get(*handle).as_word())
        .collect::<Vec<_>>();
    let mut matches = Vec::new();
    for entry in entries.iter().copied() {
        let fields = dispatch_list_to_handles(scope, entry)?;
        let specializers = scope
            .get(*fields.as_slice().first().ok_or(ObjectError::TypeError)?)
            .as_word();
        let function = scope
            .get(*fields.as_slice().get(2).ok_or(ObjectError::TypeError)?)
            .as_word();
        let qualifier_list = *fields.as_slice().get(4).ok_or(ObjectError::TypeError)?;
        if let Some(specificity) =
            dispatch_method_match(scope.context_mut(), runtime, specializers, &argument_words)?
        {
            matches.push((specificity, qualifier_list, function));
        }
    }
    matches.sort_by(|left, right| right.0.cmp(&left.0));
    let mut groups = ApplicableMethods {
        before: Vec::new(),
        primary: Vec::new(),
        after: Vec::new(),
        around: Vec::new(),
    };
    for (_, qualifier_list, function) in matches {
        let qualifiers = dispatch_list_to_handles(scope, qualifier_list)?;
        let qualifier = match qualifiers.as_slice() {
            [] if combination_kind == MethodCombination::Standard => None,
            [qualifier] => Some(scope.get(*qualifier).as_word()),
            // check-added-lines: allow(wildcard) multiple qualifier objects are invalid here.
            _ => return Err(ObjectError::TypeError),
        };
        match combination_kind {
            MethodCombination::Standard => match qualifier {
                None => groups.primary.push(function),
                Some(qualifier) if qualifier == scope.get(around_qualifier).as_word() => {
                    groups.around.push(function);
                }
                Some(qualifier) if qualifier == scope.get(before_qualifier).as_word() => {
                    groups.before.push(function);
                }
                Some(qualifier) if qualifier == scope.get(after_qualifier).as_word() => {
                    groups.after.push(function);
                }
                // check-added-lines: allow(wildcard) standard combination rejects other qualifiers.
                Some(_) => return Err(ObjectError::TypeError),
            },
            // check-added-lines: allow(wildcard) Standard was handled above; remaining combinations are short.
            _ => match qualifier {
                Some(qualifier) if qualifier == scope.get(around_qualifier).as_word() => {
                    groups.around.push(function);
                }
                Some(qualifier) if qualifier == scope.get(combination).as_word() => {
                    groups.primary.push(function);
                }
                // check-added-lines: allow(wildcard) short combinations reject other qualifiers.
                _ => return Err(ObjectError::TypeError),
            },
        }
    }
    groups.after.reverse();
    let most_specific_last = scope.intern(runtime, "KEYWORD", "MOST-SPECIFIC-LAST")?;
    if combination_kind != MethodCombination::Standard
        && scope.get(order).as_word() == scope.get(most_specific_last).as_word()
    {
        groups.primary.reverse();
    }
    Ok(groups)
}

pub fn collect_custom_method_groups<'ctx, C: ncl_object::FunctionCaller>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    group_specs: ncl_object::Handle<'ctx>,
    arguments: &ncl_object::HandleVec<'ctx>,
    entries: &ncl_object::HandleVec<'ctx>,
    caller: &mut C,
    values: &mut MultipleValues,
) -> Result<ncl_object::HandleVec<'ctx>, ObjectError> {
    let specs = dispatch_list_to_handles(scope, group_specs)?;
    let argument_words = arguments
        .iter()
        .map(|argument| scope.get(*argument).as_word())
        .collect::<Vec<_>>();
    let mut groups = scope.root_many(&[]);
    for spec in specs.iter().copied() {
        let fields = dispatch_list_to_handles(scope, spec)?;
        let pattern = fields.as_slice().get(1).copied();
        let mut applicable = Vec::new();
        for entry in entries.iter().copied() {
            let method = dispatch_list_to_handles(scope, entry)?;
            let specializers = scope
                .get(*method.as_slice().first().ok_or(ObjectError::TypeError)?)
                .as_word();
            let qualifiers = *method.as_slice().get(4).ok_or(ObjectError::TypeError)?;
            let Some(specificity) =
                dispatch_method_match(scope.context_mut(), runtime, specializers, &argument_words)?
            else {
                continue;
            };
            if custom_qualifier_match(scope, runtime, pattern, qualifiers, caller, values)? {
                applicable.push((specificity, entry));
            }
        }
        applicable.sort_by(|left, right| right.0.cmp(&left.0));
        let mut methods = scope.root_many(&[]);
        for (_, method) in applicable {
            dispatch_push_handle(scope, &mut methods, method);
        }
        let method_list = scope.make_list(runtime, &methods)?;
        dispatch_push_handle(scope, &mut groups, method_list);
    }
    Ok(groups)
}

fn custom_qualifier_match<'ctx, C: ncl_object::FunctionCaller>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    pattern: Option<ncl_object::Handle<'ctx>>,
    qualifiers: ncl_object::Handle<'ctx>,
    caller: &mut C,
    values: &mut MultipleValues,
) -> Result<bool, ObjectError> {
    let Some(pattern) = pattern else {
        return Ok(scope.get(qualifiers).as_word() == Word::NIL);
    };
    let pattern_word = scope.get(pattern).as_word();
    if pattern_word == Word::NIL || pattern_word.is_cons() {
        return lisp_equal_word(
            scope.context(),
            pattern_word,
            scope.get(qualifiers).as_word(),
            0,
        );
    }
    let function = super::symbol_function(scope.context(), pattern_word)?;
    let function = scope.root(ncl_object::Local::from_word(function));
    let args = scope.root_many(&[ncl_object::Local::from_word(
        scope.get(qualifiers).as_word(),
    )]);
    let result = scope.call_function(runtime, function, &args, caller, values)?;
    Ok(scope.get(result).as_word() != Word::NIL)
}
