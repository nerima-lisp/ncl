use super::super::*;

#[test]
fn array_relation_respects_unbounded_elements_and_axis_shapes() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(125))?;
    let integer = named(&mut arena, BuiltinType::Integer)?;
    let number = named(&mut arena, BuiltinType::Number)?;
    let typed_rank_one = arena.array(ArrayType {
        element_type: Some(integer),
        dimensions: ArrayDimensions::Rank(1),
        simple: false,
    })?;
    let untyped_rank_one = arena.array(ArrayType {
        element_type: None,
        dimensions: ArrayDimensions::Rank(1),
        simple: false,
    })?;
    let any_dimensions = arena.array(ArrayType {
        element_type: Some(integer),
        dimensions: ArrayDimensions::Any,
        simple: false,
    })?;
    let typed_rank_two = arena.array(ArrayType {
        element_type: Some(integer),
        dimensions: ArrayDimensions::Rank(2),
        simple: false,
    })?;
    let number_rank_one = arena.array(ArrayType {
        element_type: Some(number),
        dimensions: ArrayDimensions::Rank(1),
        simple: false,
    })?;
    let axes_two = arena.array(ArrayType {
        element_type: Some(integer),
        dimensions: ArrayDimensions::Axes(vec![None, None]),
        simple: false,
    })?;
    let constrained_axis = arena.array(ArrayType {
        element_type: Some(integer),
        dimensions: ArrayDimensions::Axes(vec![Some(4)]),
        simple: false,
    })?;
    let unconstrained_axis = arena.array(ArrayType {
        element_type: Some(integer),
        dimensions: ArrayDimensions::Axes(vec![None]),
        simple: false,
    })?;

    // CLHS array type specifiers constrain element type, rank, and dimensions.
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_array.htm
    assert_eq!(
        arena.subtype(typed_rank_one, untyped_rank_one)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(typed_rank_one, any_dimensions)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(typed_rank_one, typed_rank_one)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(typed_rank_one, number_rank_one)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(typed_rank_one, typed_rank_two)?,
        SubtypeRelation::No
    );
    assert_eq!(
        arena.subtype(axes_two, typed_rank_one)?,
        SubtypeRelation::No
    );
    assert_eq!(
        arena.subtype(typed_rank_one, constrained_axis)?,
        SubtypeRelation::No
    );
    assert_eq!(
        arena.subtype(axes_two, unconstrained_axis)?,
        SubtypeRelation::No
    );
    Ok(())
}
