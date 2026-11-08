use super::*;

#[test]
fn verify_input_requires_matching_arena_and_resource_owners() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(86), 0);
    let builder = form_builder(form)?;
    let hir = builder.finish()?;
    let values = HashSet::new();
    let producers = HashMap::new();
    let constants = hir.type_spec_constant_ids();
    let valid_resources = VerifyResources::new(
        form.compilation_unit(),
        hir.symbol_ids(),
        &values,
        &producers,
        constants,
    );
    assert_eq!(verify_with_resources(&hir, form, valid_resources), Ok(()));

    let foreign_form = FormId::new(form.compilation_unit(), 1);
    let mismatched_form_resources = VerifyResources::new(
        form.compilation_unit(),
        hir.symbol_ids(),
        &values,
        &producers,
        constants,
    );
    assert_eq!(
        verify_with_resources(&hir, foreign_form, mismatched_form_resources),
        Err(VerifyError::WrongForm)
    );

    let foreign_symbol = SymbolId::try_from_index(CompilationUnitId::new(87), 0)?;
    let foreign_symbols = HashSet::from([foreign_symbol]);
    let mismatched_resource_resources = VerifyResources::new(
        form.compilation_unit(),
        &foreign_symbols,
        &values,
        &producers,
        constants,
    );
    assert_eq!(
        verify_with_resources(&hir, form, mismatched_resource_resources),
        Err(VerifyError::WrongForm)
    );
    Ok(())
}

#[test]
fn verify_input_requires_each_value_producer_to_be_in_the_value_table() -> Result<(), Box<dyn Error>>
{
    let form = FormId::new(CompilationUnitId::new(88), 0);
    let mut builder = form_builder(form)?;
    let producer = builder.alloc_node(builder.root_scope(), NodeKind::Literal(Literal::True))?;
    let hir = builder.finish()?;
    let value = ValueId::try_from_index(form, 0)?;
    let values = HashSet::new();
    let producers = HashMap::from([(value, producer)]);
    let resources = VerifyResources::new(
        form.compilation_unit(),
        hir.symbol_ids(),
        &values,
        &producers,
        hir.type_spec_constant_ids(),
    );

    assert_eq!(
        verify_with_resources(&hir, form, resources),
        Err(VerifyError::InvalidValue(value))
    );
    Ok(())
}
