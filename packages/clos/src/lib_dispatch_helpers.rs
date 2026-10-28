#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MethodCombination {
    Standard,
    Plus,
    And,
    Append,
    List,
    Max,
    Min,
    Nconc,
    Or,
    Progn,
}

fn method_combination_kind(
    ctx: &ThreadContext,
    name: Word,
) -> Result<MethodCombination, ObjectError> {
    if !matches!(classify_object(ctx, name), ObjectRef::Symbol(_)) {
        return Err(ObjectError::TypeError);
    }
    let name = symbol_name_string(ctx, name)?;
    match name.to_ascii_uppercase().as_str() {
        "STANDARD" => Ok(MethodCombination::Standard),
        "+" => Ok(MethodCombination::Plus),
        "AND" => Ok(MethodCombination::And),
        "APPEND" => Ok(MethodCombination::Append),
        "LIST" => Ok(MethodCombination::List),
        "MAX" => Ok(MethodCombination::Max),
        "MIN" => Ok(MethodCombination::Min),
        "NCONC" => Ok(MethodCombination::Nconc),
        "OR" => Ok(MethodCombination::Or),
        "PROGN" => Ok(MethodCombination::Progn),
        // check-added-lines: allow(wildcard) unknown built-in combination names are invalid.
        _ => Err(ObjectError::TypeError),
    }
}

type EffectiveMethod<'ctx> = ncl_object::Handle<'ctx>;

fn dispatch_list_to_handles<'ctx>(
    scope: &mut Scope<'ctx>,
    handle: ncl_object::Handle<'ctx>,
) -> Result<ncl_object::HandleVec<'ctx>, ObjectError> {
    let word = scope.get(handle).as_word();
    scope.list_to_handle_vec(Local::from_word(word))
}

fn dispatch_push_handle<'ctx>(
    scope: &mut Scope<'ctx>,
    values: &mut ncl_object::HandleVec<'ctx>,
    handle: ncl_object::Handle<'ctx>,
) {
    let word = scope.get(handle).as_word();
    values.push(scope, Local::from_word(word));
}

fn tail_handles<'ctx>(
    scope: &mut Scope<'ctx>,
    values: &ncl_object::HandleVec<'ctx>,
) -> ncl_object::HandleVec<'ctx> {
    let words = values
        .as_slice()
        .get(1..)
        .unwrap_or(&[])
        .iter()
        .map(|handle| scope.get(*handle).as_word())
        .collect::<Vec<_>>();
    scope.root_many(
        &words
            .iter()
            .copied()
            .map(Local::from_word)
            .collect::<Vec<_>>(),
    )
}

fn dispatch_method_match(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    specializers: Word,
    arguments: &[Word],
) -> Result<Option<Vec<usize>>, ObjectError> {
    let specializers = form_elements(ctx, specializers)?;
    if specializers.len() > arguments.len() {
        return Ok(None);
    }
    let mut specificity = Vec::with_capacity(specializers.len());
    for (specializer, argument) in specializers.iter().zip(arguments) {
        let fields = form_elements(ctx, *specializer)?;
        let designator = *fields.get(1).ok_or(ObjectError::TypeError)?;
        if let Some(value) = dispatch_eql_specializer(ctx, designator)? {
            if !ncl_lib_numbers::eql_words(ctx, *argument, value) {
                return Ok(None);
            }
            specificity.push(usize::MAX);
            continue;
        }
        if designator == Word::TRUE || symbol_name_string(ctx, designator)? == "T" {
            specificity.push(0);
            continue;
        }
        let expected = class_designator(ctx, runtime, designator)?;
        let actual = class_of(ctx, runtime, *argument)?;
        if !class_is_subclass(ctx, actual, expected)? {
            return Ok(None);
        }
        let precedence = mop::class_precedence_order(ctx, actual)?;
        let rank = precedence
            .iter()
            .position(|class| *class == expected)
            .ok_or(ObjectError::Layout)?;
        specificity.push(precedence.len() - rank);
    }
    Ok(Some(specificity))
}

fn dispatch_eql_specializer(
    ctx: &ThreadContext,
    designator: Word,
) -> Result<Option<Word>, ObjectError> {
    if designator.is_cons() {
        let fields = form_elements(ctx, designator)?;
        let head = *fields.first().ok_or(ObjectError::TypeError)?;
        if symbol_name_string(ctx, head)? == "EQL" {
            if fields.len() != 2 {
                return Err(ObjectError::TypeError);
            }
            return Ok(fields.get(1).copied());
        }
        return Ok(None);
    }
    if matches!(classify_object(ctx, designator), ObjectRef::SimpleVector(_))
        && simple_vector_length(ctx, designator)? == 2
        && simple_vector_ref(ctx, designator, 0)? == Word::fixnum(1)
    {
        return Ok(Some(simple_vector_ref(ctx, designator, 1)?));
    }
    Ok(None)
}
