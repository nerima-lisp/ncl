#[cfg(test)]
mod tests {
    use super::super::*;

    fn span() -> Span {
        Span::new(2, 5)
    }

    fn function(instructions: Vec<Instruction>) -> FunctionCode {
        FunctionCode {
            name: None,
            parameters: Vec::new(),
            required_escaped: Vec::new(),
            optional: Vec::new(),
            keywords: Vec::new(),
            has_keyword_section: false,
            allow_other_keys: false,
            rest: None,
            rest_escaped: false,
            auxiliary: Vec::new(),
            instructions,
        }
    }

    #[test]
    fn dotted_parts_flatten_nested_proper_lists() {
        let value = Value::dotted_list(
            vec![Value::Integer(1)],
            Value::dotted_list(
                vec![Value::Integer(2)],
                Value::list(vec![Value::Integer(3)]),
            ),
        );

        let Some((items, tail)) = destructure_dotted_parts(&value) else {
            panic!("nested dotted list should be destructurable");
        };
        assert!(matches!(
            items.as_slice(),
            [Value::Integer(1), Value::Integer(2), Value::Integer(3)]
        ));
        assert!(matches!(tail, Value::Nil));
    }

    #[test]
    fn dotted_parts_preserve_an_atom_tail() {
        let value = Value::dotted_list(vec![Value::Integer(1)], Value::Integer(2));

        let Some((items, tail)) = destructure_dotted_parts(&value) else {
            panic!("dotted list should be destructurable");
        };
        assert!(matches!(items.as_slice(), [Value::Integer(1)]));
        assert!(matches!(tail, Value::Integer(2)));
        assert!(destructure_dotted_parts(&Value::Integer(1)).is_none());
    }

    #[test]
    fn jump_target_accepts_instruction_boundary_and_rejects_end() {
        let function = function(vec![Instruction::Return]);

        assert_eq!(jump_target(&function, 0, span()), Ok(0));
        assert!(matches!(
            jump_target(&function, 1, span()),
            Err(RuntimeError::InvalidForm { message, span: Some(found) })
                if message == "compiled jump target is out of range" && found == span()
        ));
    }

    #[test]
    fn constant_value_converts_every_compiler_constant() {
        let cases = [
            (Constant::Nil, Value::Nil),
            (Constant::Boolean(true), Value::boolean(true)),
            (Constant::Integer(7), Value::Integer(7)),
            (
                Constant::Rational {
                    numerator: 3,
                    denominator: 2,
                },
                Value::rational(3, 2).unwrap(),
            ),
            (Constant::Float(1.5), Value::Float(1.5)),
            (Constant::String("text".to_owned()), Value::string("text")),
            (Constant::Character('x'), Value::Character('x')),
            (Constant::Symbol("name".to_owned()), Value::symbol("name")),
            (
                Constant::SymbolExact("Name".to_owned()),
                Value::symbol_exact("Name"),
            ),
            (Constant::Keyword("key".to_owned()), Value::keyword("key")),
            (
                Constant::KeywordExact("Key".to_owned()),
                Value::keyword_exact("Key"),
            ),
        ];

        for (constant, expected) in cases {
            assert_eq!(constant_value(&constant).to_string(), expected.to_string());
        }
    }

    #[test]
    fn pop_value_reports_empty_stack_with_operation_context() {
        let mut stack = Vec::new();
        assert!(matches!(
            pop_value(&mut stack, span(), "return"),
            Err(RuntimeError::InvalidForm { message, span: Some(found) })
                if message == "return has no value on the stack" && found == span()
        ));

        stack.push(Value::Integer(4));
        assert!(matches!(
            pop_value(&mut stack, span(), "return"),
            Ok(Value::Integer(4))
        ));
    }
}
