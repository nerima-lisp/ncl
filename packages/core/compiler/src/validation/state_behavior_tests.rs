#[cfg(test)]
mod tests {
    use crate::{CompileState, Form, FormKind, Instruction, Span};

    fn atom(source: &str) -> Form {
        Form::atom(source, Span::new(0, source.len()))
    }

    #[test]
    fn compile_state_rejects_invalid_instruction_access() {
        let span = Span::new(0, 1);
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());

        assert!(state.instruction_count(function + 1, span).is_err());
        assert!(state.emit(function + 1, Instruction::Return, span).is_err());
        assert!(state.patch_jump(function + 1, 0, 0, span).is_err());
        assert!(state.patch_jump(function, 0, 0, span).is_err());

        assert!(state.emit(function, Instruction::Return, span).is_ok());
        assert!(state.patch_jump(function, 0, 1, span).is_err());
    }

    #[test]
    fn compile_state_patches_only_jump_instructions() {
        let span = Span::new(0, 1);
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());

        assert!(
            state
                .emit(function, Instruction::JumpIfFalse(0), span)
                .is_ok()
        );
        assert!(state.emit(function, Instruction::Jump(0), span).is_ok());
        assert!(state.patch_jump(function, 0, 7, span).is_ok());
        assert!(state.patch_jump(function, 1, 9, span).is_ok());
        assert!(state.patch_jump(function, 0, 0, span).is_ok());
    }

    #[test]
    fn compile_state_collects_names_and_skips_non_name_literals() {
        let span = Span::new(0, 1);
        let mut state = CompileState::default();
        let forms = vec![
            Form::list(
                vec![
                    atom("foo"),
                    Form::new(FormKind::Vector(vec![atom("bar")]), span),
                    Form::dotted_list(vec![atom("baz")], atom("tail"), span),
                ],
                span,
            ),
            Form::new(FormKind::String("ignored".to_string()), span),
            Form::new(FormKind::Character('x'), span),
        ];

        state.collect_names(&forms);
        assert!(state.used_names.contains("FOO"));
        assert!(state.used_names.contains("BAR"));
        assert!(state.used_names.contains("BAZ"));
        assert!(state.used_names.contains("TAIL"));
        assert_eq!(state.fresh_name("TEMP"), "__NCL_TEMP_0");
        state.used_names.insert("__NCL_TEMP_1".to_string());
        assert_eq!(state.fresh_name("TEMP"), "__NCL_TEMP_2");
    }
}
