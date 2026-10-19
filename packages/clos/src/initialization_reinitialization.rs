fn reinitialize_instance_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let instance_handle: Handle<'_, Word> = scope.root(Local::from_word(args.required(0)?));
    let instance_word = scope.get(instance_handle).as_word();
    let initarg_words = args
        .as_slice()
        .get(1..)
        .ok_or_else(|| type_error(scope.context_mut(), instance_word, ObjectType::Instance))?;
    let initarg_locals = initarg_words
        .iter()
        .copied()
        .map(Local::from_word)
        .collect::<Vec<_>>();
    let initargs = scope.root_many(&initarg_locals);
    let instance = instance_argument(scope.context_mut(), instance_word)?;
    let class: Handle<'_, Word> = scope.root(Local::from_word(ncl_object::instance_class(
        scope.context(),
        instance,
    )?));
    update_supplied_slots(&mut scope, instance, class, &initargs)?;
    values.clear();
    Ok(scope.get(instance_handle).as_word())
}

fn update_instance_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    reinitialize_instance_builtin(ctx, runtime, args, values)
}
