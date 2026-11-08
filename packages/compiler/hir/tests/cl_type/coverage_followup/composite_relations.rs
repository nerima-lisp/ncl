use super::super::*;

#[test]
fn intersection_targets_combine_yes_no_and_unknown_relations() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(130))?;
    let integer = named(&mut arena, BuiltinType::Integer)?;
    let number = named(&mut arena, BuiltinType::Number)?;
    let character = named(&mut arena, BuiltinType::Character)?;
    let class_symbol = SymbolId::try_from_index(CompilationUnitId::new(0), 14)?;
    let class = arena.class(class_symbol)?;
    let excludes_integer = arena.intersection([number, character])?;
    let uncertain = arena.intersection([number, class])?;

    // CLHS AND intersects type sets; the HIR lattice preserves unresolved class facts.
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_and.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_char.htm
    // docs/src/design/hir.md
    assert_eq!(
        arena.subtype(integer, excludes_integer)?,
        SubtypeRelation::No
    );
    assert_eq!(arena.subtype(integer, uncertain)?, SubtypeRelation::Unknown);
    Ok(())
}

#[test]
fn subtype_cache_union_and_double_complement_paths_preserve_contract() -> Result<(), Box<dyn Error>>
{
    let mut arena = TypeArena::new(form(127))?;
    let integer = named(&mut arena, BuiltinType::Integer)?;
    let number = named(&mut arena, BuiltinType::Number)?;
    let character = named(&mut arena, BuiltinType::Character)?;
    let string = named(&mut arena, BuiltinType::String)?;
    let integer_cons = arena.cons(integer, integer)?;
    let number_cons = arena.cons(number, number)?;
    let not_integer = arena.complement(integer)?;
    let not_number = arena.complement(number)?;
    let not_character = arena.complement(character)?;
    let no_match = arena.union([character, string])?;
    let foreign = TypeId::try_from_index(form(128), 9)?;
    let class_symbol = SymbolId::try_from_index(CompilationUnitId::new(0), 12)?;
    let class = arena.class(class_symbol)?;

    // CLHS defines OR and NOT set semantics; unresolved HIR facts stay Unknown by contract.
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_or.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_not.htm
    // docs/src/design/hir.md
    assert_eq!(
        arena.subtype(integer_cons, number_cons)?,
        SubtypeRelation::Yes
    );
    assert_eq!(arena.subtype(integer, no_match)?, SubtypeRelation::Unknown);
    assert_eq!(arena.subtype(string, not_character)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(not_integer, not_number)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(class, class)?, SubtypeRelation::Yes);
    assert_eq!(
        arena.subtype(foreign, integer),
        Err(ClTypeError::UnknownType(foreign))
    );
    assert_eq!(
        arena.subtype(integer, foreign),
        Err(ClTypeError::UnknownType(foreign))
    );
    Ok(())
}
