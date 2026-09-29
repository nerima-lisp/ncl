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
    copy_structure(ctx, runtime, args.required(0)?)
}

fn copy_structure(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: Word,
) -> Result<Word, ObjectError> {
    let mut scope = ncl_object::Scope::new(ctx);
    let object: ncl_object::Handle<'_, Word> =
        scope.root(ncl_object::Local::from_word(source));
    let object_word = scope.get(object).as_word();
    let layout = ncl_object::structure_layout(scope.context(), object_word)?;
    let count = runtime.structure_layout_size(layout).ok_or(ObjectError::Layout)?;
    let mut slots: ncl_object::HandleVec<'_, Word> = scope.root_many(&[]);
    for index in 0..count {
        let value = ncl_object::structure_ref(scope.context(), object_word, index)?;
        slots.push(&mut scope, ncl_object::Local::from_word(value));
    }
    let slot_words = slots
        .iter()
        .map(|slot| scope.get(*slot).as_word())
        .collect::<Vec<_>>();
    ncl_object::make_structure(scope.context_mut(), runtime, layout, &slot_words)
}

#[cfg(test)]
mod structure_tests {
    use super::*;

    #[test]
    fn structure_copy_survives_gc_stress_and_strict_forwarding() -> Result<(), ObjectError> {
        let runtime = Runtime::new()?;
        let mut context = ThreadContext::new();
        context.register(&runtime)?;
        let layout = runtime.register_structure_layout(2)?;
        let source = ncl_object::make_structure(
            &mut context,
            &runtime,
            layout,
            &[Word::fixnum(11), Word::fixnum(22)],
        )?;
        context.set_strict_forwarding(true);
        context.set_gc_stress(true);
        let copy = copy_structure(&mut context, &runtime, source)?;
        assert_eq!(ncl_object::structure_ref(&context, copy, 0), Ok(Word::fixnum(11)));
        assert_eq!(ncl_object::structure_ref(&context, copy, 1), Ok(Word::fixnum(22)));
        Ok(())
    }
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
