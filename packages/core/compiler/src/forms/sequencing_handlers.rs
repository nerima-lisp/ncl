#[cfg(test)]
use crate::CompileErrorKind;
use crate::{
    CompileError, CompileState, Constant, Form, FunctionId, Instruction, Span,
    compile_eval_when_executes,
};
use ncl_syntax::FormKind;

impl CompileState {
    pub(super) fn compile_with_compilation_unit(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(CompileError::new(
                crate::CompileErrorKind::InvalidForm {
                    message: "WITH-COMPILATION-UNIT requires an options form".into(),
                },
                span,
            ));
        }
        if !matches!(items[1].kind, FormKind::List(_)) {
            return Err(CompileError::new(
                crate::CompileErrorKind::ExpectedList {
                    context: "WITH-COMPILATION-UNIT options".into(),
                },
                items[1].span,
            ));
        }
        self.compile_sequence(function, items.get(2..).unwrap_or(&[]))
    }

    pub(super) fn compile_progn(
        &mut self,
        function: FunctionId,
        items: &[Form],
    ) -> Result<(), CompileError> {
        let forms = items.get(1..).unwrap_or(&[]);
        self.compile_sequence(function, forms)
    }

    pub(super) fn compile_declare(
        &mut self,
        function: FunctionId,
        span: Span,
        operator: &str,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if matches!(operator, "DECLAIM" | "PROCLAIM") {
            for declaration in items.iter().skip(1) {
                let declaration = match &declaration.kind {
                    FormKind::List(items) if items.len() == 2 => match &items[0].kind {
                        FormKind::Atom(name) if name.eq_ignore_ascii_case("QUOTE") => &items[1],
                        _ => declaration,
                    },
                    _ => declaration,
                };
                let FormKind::List(declaration_items) = &declaration.kind else {
                    continue;
                };
                let Some(Form {
                    kind: FormKind::Atom(kind),
                    ..
                }) = declaration_items.first()
                else {
                    continue;
                };
                if !kind.eq_ignore_ascii_case("SPECIAL") {
                    continue;
                }
                for name in declaration_items.iter().skip(1) {
                    if let Ok((name, escaped)) =
                        Self::symbol_name_info(name, "special declaration name")
                    {
                        self.register_special(name, escaped);
                    }
                }
            }
        }
        self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        Ok(())
    }

    pub(super) fn compile_eval_when(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(items, "EVAL-WHEN", "at least one", span));
        }
        if compile_eval_when_executes(&items[1])? {
            self.compile_sequence(function, items.get(2..).unwrap_or(&[]))
        } else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_declare_reports_an_internal_error_for_an_invalid_function_id() {
        let mut state = CompileState::default();
        let span = Span::new(0, 1);

        let error = state.compile_declare(99, span, "DECLARE", &[]).map_or_else(
            |error| error,
            |value| panic!("an unknown function id cannot receive instructions, got {value:?}"),
        );

        assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
    }

    #[test]
    fn compile_eval_when_propagates_a_malformed_situations_error() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let items = vec![Form::atom("EVAL-WHEN", span), Form::atom("EXECUTE", span)];

        let error = state.compile_eval_when(function, span, &items).map_or_else(
            |error| error,
            |value| panic!("a non-list situations form should fail to compile, got {value:?}"),
        );

        assert!(matches!(error.kind, CompileErrorKind::ExpectedList { .. }));
    }

    #[test]
    fn compile_with_compilation_unit_rejects_non_list_options() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let items = vec![
            Form::atom("WITH-COMPILATION-UNIT", span),
            Form::atom("BAD-OPTIONS", span),
        ];

        let error = state
            .compile_with_compilation_unit(function, span, &items)
            .map_or_else(|error| error, |_| panic!("non-list options should fail"));

        assert!(matches!(error.kind, CompileErrorKind::ExpectedList { .. }));
    }

    #[test]
    fn compile_eval_when_emits_nil_for_situations_that_do_not_execute() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let situations = Form::list(Vec::new(), span);
        let items = vec![
            Form::atom("EVAL-WHEN", span),
            situations,
            Form::atom("1", span),
        ];

        state
            .compile_eval_when(function, span, &items)
            .unwrap_or_else(|error| {
                panic!("no EXECUTE situation compiles to a NIL constant: {error}")
            });

        assert_eq!(
            state.functions[function].instructions,
            vec![Instruction::Constant(Constant::Nil)]
        );
    }

    #[test]
    fn compile_declaim_registers_global_special_names() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let forms = ncl_syntax::read("(declaim (special *x*)) (let ((*x* 1)) *x*)")
            .unwrap_or_else(|error| panic!("test source should parse: {error}"));

        state
            .compile_sequence(function, &forms)
            .unwrap_or_else(|error| panic!("DECLAIM should compile: {error}"));

        assert!(
            state.functions[function]
                .instructions
                .contains(&Instruction::DefineDynamicSpecial("*X*".to_string()))
        );
    }
}
