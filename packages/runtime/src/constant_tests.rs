use super::Runtime;
use std::collections::HashMap;

use ncl_ir::{
    ArrayElementType as IrArrayElementType, Constant, ConstantIndex, Function, FunctionId,
    SpecializedArrayElementType, StructureKind,
};
use ncl_object::{
    ArrayElementType, HeapRootedWord, Word, array_dimensions, array_row_major_ref, double_value,
    simple_vector_ref, specialized_array_element_type, specialized_array_ref,
};
#[test]
fn constant_object_aliases_survive_gc_stress_and_strict_forwarding() -> Result<(), String> {
    let mut runtime = Runtime::new().map_err(|error| format!("runtime: {error:?}"))?;
    runtime.context.set_gc_stress(true);
    runtime.context.set_strict_forwarding(true);
    let function = constant_alias_function();
    let table = runtime
        .make_constants(&function, &mut HashMap::new(), &mut HashMap::new())
        .map_err(|error| format!("constants: {error:?}"))?;

    assert_constant_object_aliases(&runtime, table)?;
    assert_specialized_constant_arrays(&runtime, table)?;
    Ok(())
}

fn constant_alias_function() -> Function {
    Function {
        id: FunctionId(0),
        name: "constant-alias".to_owned(),
        params: Vec::new(),
        return_types: Vec::new(),
        blocks: Vec::new(),
        locals: Vec::new(),
        constants: vec![
            Constant::StringBytes(b"stable".to_vec()),
            Constant::Object(ConstantIndex(0)),
            Constant::StringBytes(b"allocation".to_vec()),
            Constant::Object(ConstantIndex(0)),
            Constant::Fixnum(1),
            Constant::Fixnum(0),
            Constant::Structure {
                kind: StructureKind::SpecializedArray {
                    element_type: SpecializedArrayElementType::Bit,
                },
                elements: vec![ConstantIndex(4), ConstantIndex(5)],
            },
            Constant::Character(u32::from('A')),
            Constant::Character(u32::from('B')),
            Constant::Structure {
                kind: StructureKind::SpecializedArray {
                    element_type: SpecializedArrayElementType::Character,
                },
                elements: vec![ConstantIndex(7), ConstantIndex(8)],
            },
            Constant::Character(u32::from('A')),
            Constant::Character(u32::from('B')),
            Constant::Structure {
                kind: StructureKind::SpecializedArray {
                    element_type: SpecializedArrayElementType::BaseChar,
                },
                elements: vec![ConstantIndex(10), ConstantIndex(11)],
            },
            Constant::Fixnum(7),
            Constant::Fixnum(-8),
            Constant::Structure {
                kind: StructureKind::SpecializedArray {
                    element_type: SpecializedArrayElementType::Fixnum,
                },
                elements: vec![ConstantIndex(13), ConstantIndex(14)],
            },
            Constant::Fixnum(-7),
            Constant::Fixnum(8),
            Constant::Structure {
                kind: StructureKind::SpecializedArray {
                    element_type: SpecializedArrayElementType::Signed,
                },
                elements: vec![ConstantIndex(16), ConstantIndex(17)],
            },
            Constant::Fixnum(3),
            Constant::Fixnum(4),
            Constant::Structure {
                kind: StructureKind::SpecializedArray {
                    element_type: SpecializedArrayElementType::Unsigned,
                },
                elements: vec![ConstantIndex(19), ConstantIndex(20)],
            },
            Constant::SingleFloat(1.25),
            Constant::SingleFloat(2.5),
            Constant::Structure {
                kind: StructureKind::SpecializedArray {
                    element_type: SpecializedArrayElementType::SingleFloat,
                },
                elements: vec![ConstantIndex(22), ConstantIndex(23)],
            },
            Constant::DoubleFloat(1.25),
            Constant::DoubleFloat(2.5),
            Constant::Structure {
                kind: StructureKind::SpecializedArray {
                    element_type: SpecializedArrayElementType::DoubleFloat,
                },
                elements: vec![ConstantIndex(25), ConstantIndex(26)],
            },
            Constant::Structure {
                kind: StructureKind::SimpleVector,
                elements: vec![
                    ConstantIndex(6),
                    ConstantIndex(9),
                    ConstantIndex(12),
                    ConstantIndex(15),
                    ConstantIndex(18),
                    ConstantIndex(21),
                    ConstantIndex(24),
                    ConstantIndex(27),
                    ConstantIndex(6),
                ],
            },
        ],
        handler_regions: Vec::new(),
        debug: Vec::new(),
    }
}

fn assert_constant_object_aliases(runtime: &Runtime, table: Word) -> Result<(), String> {
    let first = simple_vector_ref(&runtime.context, table, 0)
        .map_err(|error| format!("first constant: {error:?}"))?;
    let first_alias = simple_vector_ref(&runtime.context, table, 1)
        .map_err(|error| format!("first alias: {error:?}"))?;
    let second_alias = simple_vector_ref(&runtime.context, table, 3)
        .map_err(|error| format!("second alias: {error:?}"))?;
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(first, first_alias);
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(first, second_alias);
    // check-added-lines: allow(panic) test-only assertions
    assert_ne!(first, Word::NIL);
    Ok(())
}

fn assert_specialized_constant_arrays(runtime: &Runtime, table: Word) -> Result<(), String> {
    let array_container = simple_vector_ref(&runtime.context, table, 28)
        .map_err(|error| format!("array container: {error:?}"))?;
    let element_types = [
        ArrayElementType::Bit,
        ArrayElementType::Character,
        ArrayElementType::BaseChar,
        ArrayElementType::Fixnum,
        ArrayElementType::Signed,
        ArrayElementType::Unsigned,
        ArrayElementType::SingleFloat,
        ArrayElementType::DoubleFloat,
    ];
    for (index, expected_type) in element_types.into_iter().enumerate() {
        let array = simple_vector_ref(&runtime.context, array_container, index)
            .map_err(|error| format!("specialized array {index}: {error:?}"))?;
        // check-added-lines: allow(panic) test-only assertions
        assert_eq!(
            specialized_array_element_type(&runtime.context, array)
                .map_err(|error| format!("array element type: {error:?}"))?,
            expected_type
        );
        let first_element = specialized_array_ref(&runtime.context, array, 0)
            .map_err(|error| format!("specialized array element {index}: {error:?}"))?;
        match expected_type {
            ArrayElementType::Bit => {
                // check-added-lines: allow(panic) test-only assertions
                assert_eq!(first_element, Word::fixnum(1));
            }
            ArrayElementType::Character | ArrayElementType::BaseChar => {
                // check-added-lines: allow(panic) test-only assertions
                assert_eq!(first_element, Word::character(u32::from('A')));
            }
            ArrayElementType::Fixnum => {
                // check-added-lines: allow(panic) test-only assertions
                assert_eq!(first_element, Word::fixnum(7));
            }
            ArrayElementType::Signed => {
                // check-added-lines: allow(panic) test-only assertions
                assert_eq!(first_element, Word::fixnum(-7));
            }
            ArrayElementType::Unsigned => {
                // check-added-lines: allow(panic) test-only assertions
                assert_eq!(first_element, Word::fixnum(3));
            }
            ArrayElementType::SingleFloat | ArrayElementType::DoubleFloat => {
                let actual = double_value(
                    &runtime.context,
                    ncl_object::DoubleFloat::from_word(first_element),
                )
                .map_err(|error| format!("array float element: {error:?}"))?;
                // check-added-lines: allow(panic) test-only assertions
                assert_eq!(actual.to_bits(), 1.25_f64.to_bits());
                let second_element = specialized_array_ref(&runtime.context, array, 1)
                    .map_err(|error| format!("array float second element: {error:?}"))?;
                let second_actual = double_value(
                    &runtime.context,
                    ncl_object::DoubleFloat::from_word(second_element),
                )
                .map_err(|error| format!("array float second value: {error:?}"))?;
                // check-added-lines: allow(panic) test-only assertions
                assert_eq!(second_actual.to_bits(), 2.5_f64.to_bits());
            }
            ArrayElementType::T => {
                return Err("T is not a specialized array element type".to_owned());
            }
        }
    }
    let first_array = simple_vector_ref(&runtime.context, array_container, 0)
        .map_err(|error| format!("first array alias: {error:?}"))?;
    let second_array = simple_vector_ref(&runtime.context, array_container, 8)
        .map_err(|error| format!("second array alias: {error:?}"))?;
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(first_array, second_array);
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        specialized_array_ref(&runtime.context, first_array, 0)
            .map_err(|error| format!("first bit: {error:?}"))?,
        Word::fixnum(1)
    );
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        specialized_array_ref(&runtime.context, first_array, 1)
            .map_err(|error| format!("second bit: {error:?}"))?,
        Word::fixnum(0)
    );
    Ok(())
}

#[test]
fn general_array_constants_preserve_shape_type_contents_and_cross_table_identity()
-> Result<(), String> {
    let mut runtime = Runtime::new().map_err(|error| format!("runtime: {error:?}"))?;
    runtime.context.set_gc_stress(true);
    runtime.context.set_strict_forwarding(true);
    let function = |id, name: &str| Function {
        id: FunctionId(id),
        name: name.to_owned(),
        params: Vec::new(),
        return_types: Vec::new(),
        blocks: Vec::new(),
        locals: Vec::new(),
        constants: vec![
            Constant::Character(u32::from('A')),
            Constant::Character(u32::from('B')),
            Constant::Structure {
                kind: StructureKind::Array {
                    dimensions: vec![1, 2],
                    element_type: IrArrayElementType::Character,
                },
                elements: vec![ConstantIndex(0), ConstantIndex(1)],
            },
            Constant::LiteralObject {
                identity: 42,
                value: ConstantIndex(2),
            },
        ],
        handler_regions: Vec::new(),
        debug: Vec::new(),
    };
    let mut literal_objects = HashMap::new();
    let first_table = runtime
        .make_constants(
            &function(0, "first-array-literal"),
            &mut HashMap::new(),
            &mut literal_objects,
        )
        .map_err(|error| format!("first constants: {error:?}"))?;
    let first_array = simple_vector_ref(&runtime.context, first_table, 3)
        .map_err(|error| format!("first array: {error:?}"))?;
    let first_root = HeapRootedWord::new(&runtime.context, first_array)
        .map_err(|error| format!("root first array: {error:?}"))?;

    let second_table = runtime
        .make_constants(
            &function(1, "second-array-literal"),
            &mut HashMap::new(),
            &mut literal_objects,
        )
        .map_err(|error| format!("second constants: {error:?}"))?;
    let second_array = simple_vector_ref(&runtime.context, second_table, 3)
        .map_err(|error| format!("second array: {error:?}"))?;

    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(first_root.get(), second_array);
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        array_dimensions(&runtime.context, first_root.get())
            .map_err(|error| format!("array dimensions: {error:?}"))?,
        vec![1, 2]
    );
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        ncl_object::array::array_element_type(&runtime.context, first_root.get())
            .map_err(|error| format!("array element type: {error:?}"))?,
        ArrayElementType::Character
    );
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        array_row_major_ref(&runtime.context, first_root.get(), 0)
            .map_err(|error| format!("first array element: {error:?}"))?,
        Word::character(u32::from('A'))
    );
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        array_row_major_ref(&runtime.context, first_root.get(), 1)
            .map_err(|error| format!("second array element: {error:?}"))?,
        Word::character(u32::from('B'))
    );
    Ok(())
}

#[test]
fn general_array_heap_elements_survive_gc_stress_and_strict_forwarding() -> Result<(), String> {
    let mut runtime = Runtime::new().map_err(|error| format!("runtime: {error:?}"))?;
    runtime.context.set_gc_stress(true);
    runtime.context.set_strict_forwarding(true);
    let function = Function {
        id: FunctionId(0),
        name: "general-array-heap-elements".to_owned(),
        params: Vec::new(),
        return_types: Vec::new(),
        blocks: Vec::new(),
        locals: Vec::new(),
        constants: vec![
            Constant::StringBytes(b"first".to_vec()),
            Constant::StringBytes(b"second".to_vec()),
            Constant::Structure {
                kind: StructureKind::Array {
                    dimensions: vec![2],
                    element_type: IrArrayElementType::T,
                },
                elements: vec![ConstantIndex(0), ConstantIndex(1)],
            },
        ],
        handler_regions: Vec::new(),
        debug: Vec::new(),
    };
    let table = runtime
        .make_constants(&function, &mut HashMap::new(), &mut HashMap::new())
        .map_err(|error| format!("constants: {error:?}"))?;
    let table_root = HeapRootedWord::new(&runtime.context, table)
        .map_err(|error| format!("root table: {error:?}"))?;
    let array = simple_vector_ref(&runtime.context, table_root.get(), 2)
        .map_err(|error| format!("array constant: {error:?}"))?;
    let array_root = HeapRootedWord::new(&runtime.context, array)
        .map_err(|error| format!("root array: {error:?}"))?;
    let first = simple_vector_ref(&runtime.context, table_root.get(), 0)
        .map_err(|error| format!("first string constant: {error:?}"))?;
    let second = simple_vector_ref(&runtime.context, table_root.get(), 1)
        .map_err(|error| format!("second string constant: {error:?}"))?;

    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        array_row_major_ref(&runtime.context, array_root.get(), 0)
            .map_err(|error| format!("first array element: {error:?}"))?,
        first
    );
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        array_row_major_ref(&runtime.context, array_root.get(), 1)
            .map_err(|error| format!("second array element: {error:?}"))?,
        second
    );
    Ok(())
}

#[test]
fn malformed_array_constants_return_errors() -> Result<(), String> {
    let cases = [
        vec![Constant::Structure {
            kind: StructureKind::Cons,
            elements: Vec::new(),
        }],
        vec![
            Constant::Fixnum(1),
            Constant::Structure {
                kind: StructureKind::Array {
                    dimensions: vec![2],
                    element_type: IrArrayElementType::T,
                },
                elements: vec![ConstantIndex(0)],
            },
        ],
        vec![
            Constant::Fixnum(2),
            Constant::Structure {
                kind: StructureKind::Array {
                    dimensions: vec![1],
                    element_type: IrArrayElementType::Bit,
                },
                elements: vec![ConstantIndex(0)],
            },
        ],
    ];
    let mut runtime = Runtime::new().map_err(|error| format!("runtime: {error:?}"))?;
    for (index, constants) in cases.into_iter().enumerate() {
        let id = u32::try_from(index).map_err(|error| format!("test function id: {error}"))?;
        let function = Function {
            id: FunctionId(id),
            name: "malformed-array-constant".to_owned(),
            params: Vec::new(),
            return_types: Vec::new(),
            blocks: Vec::new(),
            locals: Vec::new(),
            constants,
            handler_regions: Vec::new(),
            debug: Vec::new(),
        };
        let result = runtime.make_constants(&function, &mut HashMap::new(), &mut HashMap::new());
        // check-added-lines: allow(panic) test-only assertions
        assert!(
            result.is_err(),
            "malformed constant case {index} should fail"
        );
    }
    Ok(())
}

#[test]
fn compiled_nested_function_materializes_specialized_array_constants() -> Result<(), String> {
    let mut runtime = Runtime::new().map_err(|error| format!("runtime: {error:?}"))?;
    let value = runtime
        .compile("(equalp (funcall (lambda () #*101)) #*101)")
        .map_err(|error| format!("compile: {error:?}"))?;
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(runtime.format_result(value), "T");
    Ok(())
}
