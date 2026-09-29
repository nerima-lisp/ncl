fn structure_make_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let layout = ncl_object::StructureLayout::from(
        u32::try_from(Fixnum::try_from_word(args.required(0)?).map_err(|_| ObjectError::TypeError)?.value())
            .map_err(|_| ObjectError::TypeError)?,
    );
    let slots = (1..args.len())
        .filter_map(|index| args.get(index))
        .collect::<Vec<_>>();
    ncl_object::make_structure(ctx, runtime, layout, &slots)
}

fn structure_ref_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let index = usize::try_from(Fixnum::try_from_word(args.required(1)?).map_err(|_| ObjectError::TypeError)?.value())
        .map_err(|_| ObjectError::TypeError)?;
    ncl_object::structure_ref(ctx, args.required(0)?, index)
}

fn structure_set_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let index = usize::try_from(Fixnum::try_from_word(args.required(1)?).map_err(|_| ObjectError::TypeError)?.value())
        .map_err(|_| ObjectError::TypeError)?;
    let value = args.required(2)?;
    ncl_object::structure_set(ctx, args.required(0)?, index, value)?;
    Ok(value)
}

fn structure_predicate_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let object = args.required(0)?;
    let expected = u32::try_from(Fixnum::try_from_word(args.required(1)?).map_err(|_| ObjectError::TypeError)?.value())
        .map_err(|_| ObjectError::TypeError)?;
    let expected = ncl_object::StructureLayout::from(expected);
    Ok(if let Ok(layout) = ncl_object::structure_layout(ctx, object)
        && runtime.structure_layout_is_a(layout, expected)
    {
        Word::TRUE
    } else {
        Word::NIL
    })
}

fn structure_copy_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let object = args.required(0)?;
    let layout = ncl_object::structure_layout(ctx, object)?;
    let count = runtime.structure_layout_size(layout).ok_or(ObjectError::Layout)?;
    let slots = (0..count)
        .map(|index| ncl_object::structure_ref(ctx, object, index))
        .collect::<Result<Vec<_>, _>>()?;
    ncl_object::make_structure(ctx, runtime, layout, &slots)
}

const fn descriptor(arity: BuiltinArity) -> Builtin {
    match arity {
        BuiltinArity::One => Builtin {
            lambda_list: LambdaList::fixed(ARGS_1),
            convention: ncl_object::BuiltinConvention::Direct(Arity::exact(1)),
        },
        BuiltinArity::Two => Builtin {
            lambda_list: LambdaList::fixed(ARGS_2),
            convention: ncl_object::BuiltinConvention::Direct(Arity::exact(2)),
        },
        BuiltinArity::Three => Builtin {
            lambda_list: LambdaList::fixed(ARGS_3),
            convention: ncl_object::BuiltinConvention::Direct(Arity::exact(3)),
        },
    }
}
