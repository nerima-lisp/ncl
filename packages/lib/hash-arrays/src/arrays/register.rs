use super::{
    ARRAY, DIMENSIONS, ELEMENT, EXTENSION, INDEX, LambdaList, OPTIONS, ObjectError, RESULT,
    Runtime, ThreadContext, VALUE, adjust_array_builtin, adjustable_array_p_builtin, aref_builtin,
    array_dimension_builtin, array_dimensions_builtin, array_displacement_builtin,
    array_element_type_builtin, array_has_fill_pointer_p_builtin, array_in_bounds_builtin,
    array_rank_builtin, array_row_major_index_builtin, array_total_size_builtin, arrayp_builtin,
    bit_and_builtin, bit_andc1_builtin, bit_andc2_builtin, bit_builtin, bit_eqv_builtin,
    bit_ior_builtin, bit_nand_builtin, bit_nor_builtin, bit_not_builtin, bit_orc1_builtin,
    bit_orc2_builtin, bit_xor_builtin, fill_pointer_builtin, make_array_builtin, register_one,
    row_major_aref_builtin, sbit_builtin, simple_bit_vector_p_builtin, simple_vector_p_builtin,
    svref_builtin, vector_builtin, vector_pop_builtin, vector_push_builtin,
    vector_push_extend_builtin, vectorp_builtin,
};

pub fn register(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
    register_arrays(runtime, ctx)?;
    register_bits(runtime, ctx)
}

fn register_arrays(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
    register_one(
        runtime,
        ctx,
        "MAKE-ARRAY",
        LambdaList::with_rest(&[DIMENSIONS], OPTIONS),
        make_array_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "AREF",
        LambdaList::with_rest(&[ARRAY], INDEX),
        aref_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "ROW-MAJOR-AREF",
        LambdaList::fixed(&[ARRAY, INDEX]),
        row_major_aref_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "SVREF",
        LambdaList::fixed(&[ARRAY, INDEX]),
        svref_builtin,
    )?;
    register_array_properties(runtime, ctx)?;
    register_array_operations(runtime, ctx)
}

fn register_array_properties(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
) -> Result<(), ObjectError> {
    register_one(
        runtime,
        ctx,
        "ARRAYP",
        LambdaList::fixed(&[ARRAY]),
        arrayp_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "VECTORP",
        LambdaList::fixed(&[ARRAY]),
        vectorp_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "SIMPLE-VECTOR-P",
        LambdaList::fixed(&[ARRAY]),
        simple_vector_p_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "SIMPLE-BIT-VECTOR-P",
        LambdaList::fixed(&[ARRAY]),
        simple_bit_vector_p_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "ARRAY-RANK",
        LambdaList::fixed(&[ARRAY]),
        array_rank_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "ARRAY-DIMENSION",
        LambdaList::fixed(&[ARRAY, INDEX]),
        array_dimension_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "ARRAY-DIMENSIONS",
        LambdaList::fixed(&[ARRAY]),
        array_dimensions_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "ARRAY-TOTAL-SIZE",
        LambdaList::fixed(&[ARRAY]),
        array_total_size_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "ARRAY-IN-BOUNDS-P",
        LambdaList::with_rest(&[ARRAY], INDEX),
        array_in_bounds_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "ADJUSTABLE-ARRAY-P",
        LambdaList::fixed(&[ARRAY]),
        adjustable_array_p_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "ARRAY-HAS-FILL-POINTER-P",
        LambdaList::fixed(&[ARRAY]),
        array_has_fill_pointer_p_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "FILL-POINTER",
        LambdaList::fixed(&[ARRAY]),
        fill_pointer_builtin,
    )?;
    Ok(())
}

fn register_array_operations(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
) -> Result<(), ObjectError> {
    register_one(
        runtime,
        ctx,
        "VECTOR",
        LambdaList::with_rest(&[], ELEMENT),
        vector_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "ADJUST-ARRAY",
        LambdaList::with_optional(&[ARRAY, DIMENSIONS], &[OPTIONS]),
        adjust_array_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "VECTOR-PUSH",
        LambdaList::fixed(&[ELEMENT, ARRAY]),
        vector_push_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "VECTOR-PUSH-EXTEND",
        LambdaList::with_optional(&[ELEMENT, ARRAY], &[EXTENSION]),
        vector_push_extend_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "VECTOR-POP",
        LambdaList::fixed(&[ARRAY]),
        vector_pop_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "ARRAY-ELEMENT-TYPE",
        LambdaList::fixed(&[ARRAY]),
        array_element_type_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "ARRAY-DISPLACEMENT",
        LambdaList::fixed(&[ARRAY]),
        array_displacement_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "ARRAY-ROW-MAJOR-INDEX",
        LambdaList::with_rest(&[ARRAY], INDEX),
        array_row_major_index_builtin,
    )?;
    Ok(())
}

fn register_bits(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
    register_one(
        runtime,
        ctx,
        "BIT",
        LambdaList::with_rest(&[ARRAY], INDEX),
        bit_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "SBIT",
        LambdaList::with_optional(&[ARRAY, INDEX], &[VALUE]),
        sbit_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "BIT-AND",
        LambdaList::with_optional(&[ARRAY, INDEX], &[RESULT]),
        bit_and_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "BIT-ANDC1",
        LambdaList::with_optional(&[ARRAY, INDEX], &[RESULT]),
        bit_andc1_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "BIT-ANDC2",
        LambdaList::with_optional(&[ARRAY, INDEX], &[RESULT]),
        bit_andc2_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "BIT-EQV",
        LambdaList::with_optional(&[ARRAY, INDEX], &[RESULT]),
        bit_eqv_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "BIT-IOR",
        LambdaList::with_optional(&[ARRAY, INDEX], &[RESULT]),
        bit_ior_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "BIT-NAND",
        LambdaList::with_optional(&[ARRAY, INDEX], &[RESULT]),
        bit_nand_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "BIT-NOR",
        LambdaList::with_optional(&[ARRAY, INDEX], &[RESULT]),
        bit_nor_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "BIT-NOT",
        LambdaList::with_optional(&[ARRAY], &[RESULT]),
        bit_not_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "BIT-ORC1",
        LambdaList::with_optional(&[ARRAY, INDEX], &[RESULT]),
        bit_orc1_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "BIT-ORC2",
        LambdaList::with_optional(&[ARRAY, INDEX], &[RESULT]),
        bit_orc2_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "BIT-XOR",
        LambdaList::with_optional(&[ARRAY, INDEX], &[RESULT]),
        bit_xor_builtin,
    )?;
    Ok(())
}
