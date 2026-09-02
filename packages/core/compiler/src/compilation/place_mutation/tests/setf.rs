use super::super::*;
use ncl_syntax::read;

fn parse_items(source: &str) -> Vec<Form> {
    let mut forms =
        read(source).unwrap_or_else(|error| panic!("test source should parse: {error}"));
    let form = forms.remove(0);
    let FormKind::List(items) = form.kind else {
        panic!("expected a list form, got {form:?}");
    };
    items
}

#[test]
fn compile_setf_propagates_a_malformed_value_form_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(setf x (function))");

    let error = state.compile_setf(function, span, &items).map_or_else(
        |error| error,
        |value| {
            panic!("a value form that fails to compile must propagate its own error, got {value:?}")
        },
    );

    match error.kind {
        CompileErrorKind::Arity { operator, .. } => assert_eq!(operator, "FUNCTION"),
        other => panic!("expected the nested FUNCTION arity error to propagate, got {other:?}"),
    }
}

#[test]
fn compile_setf_uses_direct_assignment_for_symbol_places() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf x 1 |Mixed| 2 (car x) 3)");

    state
        .compile_setf(function, Span::new(0, 1), &items)
        .unwrap_or_else(|error| panic!("valid SETF places should compile: {error}"));

    let instructions = &state.functions[function].instructions;
    assert!(instructions.contains(&Instruction::Set("X".to_string())));
    assert!(instructions.contains(&Instruction::SetExact("Mixed".to_string())));
    assert!(instructions.contains(&Instruction::SetfList {
        operator: "CAR".to_string(),
        name: "X".to_string(),
        escaped: false,
    }));
}

#[test]
fn compile_setf_keeps_non_symbol_list_places_on_the_explicit_fallback() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf (car (list 1)) 2)");

    state.compile_setf(function, Span::new(0, 1), &items).unwrap();

    assert!(state.functions[function]
        .instructions
        .iter()
        .any(|instruction| matches!(instruction, Instruction::Setf(_))));
}

#[test]
fn compile_setf_uses_native_fill_pointer_for_a_symbol_place() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf (fill-pointer vector) 1)");
    state.compile_setf(function, Span::new(0, 1), &items).unwrap();
    assert!(state.functions[function].instructions.contains(&Instruction::SetfFillPointerDynamic {
        name: "VECTOR".to_string(), escaped: false,
    }));
}

#[test]
fn compile_setf_uses_native_fill_pointer_for_an_evaluated_place() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf (fill-pointer (make-array 2 :fill-pointer 0)) 1)");
    state.compile_setf(function, Span::new(0, 1), &items).unwrap();
    assert!(state.functions[function]
        .instructions
        .contains(&Instruction::SetfFillPointerValue));
}

#[test]
fn compile_setf_uses_native_nth_for_a_constant_index_and_symbol_place() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf (nth 2 xs) 9)");
    state
        .compile_setf(function, Span::new(0, 1), &items)
        .unwrap();
    assert!(
        state.functions[function]
            .instructions
            .contains(&Instruction::SetfNth {
                index: 2,
                name: "XS".to_string(),
                escaped: false,
            })
    );
}

#[test]
fn compile_setf_uses_native_nth_for_a_dynamic_index_and_symbol_place() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf (nth index xs) 9)");
    state
        .compile_setf(function, Span::new(0, 1), &items)
        .unwrap();
    assert!(
        state.functions[function]
            .instructions
            .contains(&Instruction::SetfNthDynamic {
                name: "XS".to_string(),
                escaped: false,
            })
    );
}

#[test]
fn compile_setf_uses_native_dynamic_nth_for_a_nested_list_place() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf (nth index (car xs)) 9)");
    state.compile_setf(function, Span::new(0, 1), &items).unwrap();
    assert!(state.functions[function].instructions.contains(&Instruction::SetfNestedNthDynamic {
        accessors: vec!["CAR".to_string()],
        name: "XS".to_string(),
        escaped: false,
    }));
}

#[test]
fn compile_setf_uses_native_aref_for_a_symbol_place() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf (aref xs index) 9)");
    state
        .compile_setf(function, Span::new(0, 1), &items)
        .unwrap();
    assert!(
        state.functions[function]
            .instructions
            .contains(&Instruction::SetfArefDynamic {
                rank: 1,
                operator: "AREF".to_string(),
                name: "XS".to_string(),
                escaped: false,
            })
    );
}

#[test]
fn compile_setf_uses_native_aref_for_an_evaluated_place() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf (aref (make-array 1) 0) 7)");

    state.compile_setf(function, Span::new(0, 1), &items).unwrap();

    assert!(state.functions[function]
        .instructions
        .contains(&Instruction::SetfArefValue {
            rank: 1,
            operator: "AREF".to_string(),
        }));
}

#[test]
fn compile_setf_uses_native_bit_for_an_evaluated_place() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf (bit (make-array 1 :element-type 'bit) 0) 1)");

    state.compile_setf(function, Span::new(0, 1), &items).unwrap();

    assert!(state.functions[function]
        .instructions
        .contains(&Instruction::SetfBitValue { rank: 1 }));
}

#[test]
fn compile_setf_uses_native_array_accessors_for_symbol_places() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf (svref xs index) 9 (row-major-aref ys index) 8)");
    state
        .compile_setf(function, Span::new(0, 1), &items)
        .unwrap();
    let instructions = &state.functions[function].instructions;
    assert!(instructions.contains(&Instruction::SetfArefDynamic {
        rank: 1,
        operator: "SVREF".to_string(),
        name: "XS".to_string(),
        escaped: false,
    }));
    assert!(instructions.contains(&Instruction::SetfArefDynamic {
        rank: 1,
        operator: "ROW-MAJOR-AREF".to_string(),
        name: "YS".to_string(),
        escaped: false,
    }));
}

#[test]
fn compile_setf_uses_native_bit_for_a_symbol_place() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf (bit bits index) 1)");
    state
        .compile_setf(function, Span::new(0, 1), &items)
        .unwrap();
    assert!(
        state.functions[function]
            .instructions
            .contains(&Instruction::SetfBitDynamic {
                rank: 1,
                name: "BITS".to_string(),
                escaped: false
            })
    );
}

#[test]
fn compile_setf_uses_native_bitfield_accessors_for_symbol_places() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items(
        "(setf (ldb (byte 4 4) xs) 9 (mask-field (byte 3 1) ys) 7)",
    );
    state
        .compile_setf(function, Span::new(0, 1), &items)
        .unwrap();
    let instructions = &state.functions[function].instructions;
    assert!(instructions.contains(&Instruction::SetfBitfieldDynamic {
        operator: "LDB".to_string(),
        name: "XS".to_string(),
        escaped: false,
    }));
    assert!(instructions.contains(&Instruction::SetfBitfieldDynamic {
        operator: "MASK-FIELD".to_string(),
        name: "YS".to_string(),
        escaped: false,
    }));
    let target_load = instructions
        .iter()
        .position(|instruction| matches!(instruction, Instruction::Load(name) if name == "XS"))
        .unwrap();
    let byte_spec_constants_before_target = instructions[..target_load]
        .iter()
        .filter(|instruction| {
            matches!(instruction, Instruction::Constant(Constant::Integer(value)) if *value == 4)
        })
        .count();
    assert_eq!(byte_spec_constants_before_target, 2);
}

#[test]
fn compile_setf_uses_native_bitfield_accessor_for_evaluated_place() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf (ldb (byte 4 0) (identity 0)) 3)");
    state.compile_setf(function, Span::new(0, 1), &items).unwrap();
    assert!(state.functions[function].instructions.contains(&Instruction::SetfBitfieldValue {
        operator: "LDB".to_string(),
    }));
}

#[test]
fn compile_setf_uses_native_element_accessors_for_symbol_places() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf (elt xs index) 9 (char text index) #\\X)");
    state
        .compile_setf(function, Span::new(0, 1), &items)
        .unwrap();
    let instructions = &state.functions[function].instructions;
    assert!(instructions.contains(&Instruction::SetfElementDynamic {
        operator: "ELT".to_string(),
        name: "XS".to_string(),
        escaped: false,
    }));
    assert!(instructions.contains(&Instruction::SetfElementDynamic {
        operator: "CHAR".to_string(),
        name: "TEXT".to_string(),
        escaped: false,
    }));
}

#[test]
fn compile_setf_uses_native_element_accessor_for_an_evaluated_place() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf (char (copy-seq \"abc\") 1) #\\X)");
    state
        .compile_setf(function, Span::new(0, 1), &items)
        .unwrap();
    let instructions = &state.functions[function].instructions;
    assert!(instructions.contains(&Instruction::SetfElementValue {
        operator: "CHAR".to_string(),
    }));
}

#[test]
fn compile_setf_uses_native_subseq_for_symbol_places() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items =
        parse_items("(setf (subseq xs start end) replacement (subseq ys start) replacement)");
    state
        .compile_setf(function, Span::new(0, 1), &items)
        .unwrap();
    let instructions = &state.functions[function].instructions;
    assert!(instructions.contains(&Instruction::SetfSubseqDynamic {
        has_end: true,
        name: "XS".to_string(),
        escaped: false,
    }));
    assert!(instructions.contains(&Instruction::SetfSubseqDynamic {
        has_end: false,
        name: "YS".to_string(),
        escaped: false,
    }));
}

#[test]
fn compile_setf_uses_native_getf_for_symbol_places() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf (getf plist :key) value)");
    state
        .compile_setf(function, Span::new(0, 1), &items)
        .unwrap();
    assert!(
        state.functions[function]
            .instructions
            .contains(&Instruction::SetfGetfDynamic {
                name: "PLIST".to_string(),
                escaped: false,
            })
    );
}

#[test]
fn compile_setf_keeps_evaluated_getf_places_on_the_alias_safe_fallback() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf (getf (car cells) :key) value)");
    state.compile_setf(function, Span::new(0, 1), &items).unwrap();
    assert!(state.functions[function]
        .instructions
        .iter()
        .any(|instruction| matches!(instruction, Instruction::Setf(_))));
}

#[test]
fn compile_setf_keeps_evaluated_subseq_places_on_the_alias_safe_fallback() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf (subseq (car cells) start end) replacement)");
    state.compile_setf(function, Span::new(0, 1), &items).unwrap();
    assert!(state.functions[function]
        .instructions
        .iter()
        .any(|instruction| matches!(instruction, Instruction::Setf(_))));
}

#[test]
fn compile_setf_uses_native_symbol_plist_for_symbol_places() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf (symbol-plist target) '(:key 42))");
    state.compile_setf(function, items[0].span, &items).unwrap();
    assert!(state.functions[function]
        .instructions
        .contains(&Instruction::SetfSymbolPlistDynamic));
}

#[test]
fn compile_setf_uses_native_symbol_plist_for_an_evaluated_place() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf (symbol-plist (if flag 'target 'other)) '(:key 42))");
    state.compile_setf(function, items[0].span, &items).unwrap();
    assert!(state.functions[function]
        .instructions
        .contains(&Instruction::SetfSymbolPlistValue));
}

#[test]
fn compile_setf_uses_native_get_for_general_places() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf (get target :key) 42)");
    state.compile_setf(function, items[0].span, &items).unwrap();
    assert!(
        state.functions[function]
            .instructions
            .contains(&Instruction::SetfGetDynamic)
    );
}

#[test]
fn compile_setf_uses_native_gethash_for_general_places() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf (gethash key table) value)");
    state.compile_setf(function, items[0].span, &items).unwrap();
    assert!(
        state.functions[function]
            .instructions
            .contains(&Instruction::SetfGethashDynamic)
    );
}

#[test]
fn compile_setf_uses_native_slot_value_for_general_places() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let items = parse_items("(setf (slot-value object 'name) value)");
    state.compile_setf(function, items[0].span, &items).unwrap();
    assert!(state.functions[function]
        .instructions
        .contains(&Instruction::SetfSlotValueDynamic));
}
