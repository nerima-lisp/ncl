use super::super::*;
use ncl_hir::IdError;

#[test]
fn nested_intersections_flatten_to_the_same_type() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(120))?;
    let integer = named(&mut arena, BuiltinType::Integer)?;
    let number = named(&mut arena, BuiltinType::Number)?;
    let real = named(&mut arena, BuiltinType::Real)?;
    let inner = arena.intersection([integer, number])?;
    let nested = arena.intersection([inner, real])?;
    let flat = arena.intersection([integer, number, real])?;

    assert_eq!(nested, flat);
    Ok(())
}

#[test]
fn public_type_constructors_validate_child_identities() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(121))?;
    let integer = named(&mut arena, BuiltinType::Integer)?;
    let foreign_type = TypeId::try_from_index(form(122), 7)?;
    let local_symbol = SymbolId::try_from_index(CompilationUnitId::new(0), 3)?;
    let foreign_symbol = SymbolId::try_from_index(CompilationUnitId::new(1), 4)?;

    assert_eq!(
        arena.union([foreign_type]),
        Err(ClTypeError::UnknownType(foreign_type))
    );
    assert_eq!(
        arena.intersection([foreign_type]),
        Err(ClTypeError::UnknownType(foreign_type))
    );
    assert_eq!(
        arena.cons(foreign_type, integer),
        Err(ClTypeError::UnknownType(foreign_type))
    );
    assert_eq!(
        arena.cons(integer, foreign_type),
        Err(ClTypeError::UnknownType(foreign_type))
    );
    assert_eq!(
        arena.array(ArrayType {
            element_type: Some(foreign_type),
            dimensions: ArrayDimensions::Any,
            simple: false,
        }),
        Err(ClTypeError::UnknownType(foreign_type))
    );
    assert_eq!(
        arena.function(FunctionType {
            parameters: FunctionParameters::Fixed(vec![foreign_type]),
            returns: integer,
        }),
        Err(ClTypeError::UnknownType(foreign_type))
    );
    assert_eq!(
        arena.function(FunctionType {
            parameters: FunctionParameters::Any,
            returns: foreign_type,
        }),
        Err(ClTypeError::UnknownType(foreign_type))
    );
    assert_eq!(
        arena.multiple_values([foreign_type]),
        Err(ClTypeError::UnknownType(foreign_type))
    );
    assert_eq!(
        arena.opaque(foreign_symbol, []),
        Err(ClTypeError::ForeignSymbol(foreign_symbol))
    );
    assert_eq!(
        arena.opaque(local_symbol, [OpaqueOperand::Type(foreign_type)]),
        Err(ClTypeError::UnknownType(foreign_type))
    );
    assert_eq!(
        arena.opaque(local_symbol, [OpaqueOperand::Symbol(foreign_symbol)]),
        Err(ClTypeError::ForeignSymbol(foreign_symbol))
    );
    assert_eq!(
        arena.member(vec![MemberValue::OpaqueObject {
            owner: CompilationUnitId::new(1),
            index: 5,
        }]),
        Err(ClTypeError::ForeignObject(CompilationUnitId::new(1)))
    );

    let error = ClTypeError::UnknownType(foreign_type);
    assert!(error.to_string().starts_with("HIR type error:"));
    let id_error = IdError::IndexOverflow { index: usize::MAX };
    assert_eq!(ClTypeError::from(id_error), ClTypeError::Id(id_error));
    Ok(())
}

#[test]
fn numeric_bound_flags_and_range_errors_are_observable() {
    let included = NumericBound::inclusive(NumericValue::Integer(2));
    let excluded = NumericBound::exclusive(NumericValue::Integer(2));

    assert!(included.is_inclusive());
    assert!(!excluded.is_inclusive());
    assert_eq!(included.value(), NumericValue::Integer(2));
    assert_eq!(excluded.value(), NumericValue::Integer(2));
    assert!(format!("{}", NumericRangeError::EmptyInterval).starts_with("numeric range error:"));
    assert_eq!(
        NumericRange::new(
            NumericDomain::Integer,
            Some(NumericBound::inclusive(NumericValue::Integer(2))),
            Some(NumericBound::inclusive(NumericValue::Integer(1))),
        ),
        Err(NumericRangeError::EmptyInterval)
    );
}
