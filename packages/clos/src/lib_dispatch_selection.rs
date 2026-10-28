use super::{
    MethodCombination, ObjectError, Runtime, Scope, Word, dispatch_list_to_handles,
    dispatch_method_match,
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
