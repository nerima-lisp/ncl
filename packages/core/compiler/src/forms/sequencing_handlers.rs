#[cfg(test)]
use crate::CompileErrorKind;
use crate::{
    CompileError, CompileState, Constant, Form, FormKind, FunctionId, Instruction, Span,
    compile_eval_when_executes,
};

impl CompileState {
    pub(crate) fn is_string_form(form: &Form) -> bool {
        match &form.kind {
            FormKind::String(_) => true,
            FormKind::Literal(value) => value.downcast_ref::<String>().is_some(),
            _ => false,
        }
    }

    fn is_declaration_form(form: &Form) -> bool {
        let FormKind::List(items) = &form.kind else {
            return false;
        };
        let Some(operator) = items.first() else {
            return false;
        };
        Self::symbol_name(operator, "declaration operator").is_ok_and(|name| name == "DECLARE")
    }

    pub(super) fn compile_progn(
        &mut self,
        function: FunctionId,
        items: &[Form],
    ) -> Result<(), CompileError> {
        let forms = items.get(1..).unwrap_or(&[]);
        self.compile_sequence(function, forms)
    }

    pub(crate) fn special_declaration_names(
        forms: &[Form],
    ) -> Result<Vec<(String, bool)>, CompileError> {
        let mut names = Vec::new();
        let mut index = 0;
        let mut documentation_seen = false;
        while let Some(form) = forms.get(index) {
            if !documentation_seen
                && Self::is_string_form(form)
                && forms.get(index + 1).is_some_and(Self::is_declaration_form)
            {
                documentation_seen = true;
                index += 1;
                continue;
            }
            let FormKind::List(items) = &form.kind else {
                break;
            };
            let Some(operator) = items.first() else {
                break;
            };
            let Ok(operator_name) = Self::symbol_name(operator, "declaration operator") else {
                break;
            };
            if operator_name != "DECLARE" {
                break;
            }
            names.extend(Self::special_names_in_declare(items)?);
            index += 1;
        }
        Ok(names)
    }

    fn special_names_in_declare(items: &[Form]) -> Result<Vec<(String, bool)>, CompileError> {
        let mut names = Vec::new();
        for declaration in items.iter().skip(1) {
            let FormKind::List(specification) = &declaration.kind else {
                continue;
            };
            let Some(kind) = specification.first() else {
                continue;
            };
            let Ok(kind_name) = Self::symbol_name(kind, "declaration name") else {
                continue;
            };
            if kind_name != "SPECIAL" {
                continue;
            }
            for name in specification.iter().skip(1) {
                names.push(Self::symbol_name_info(name, "special declaration name")?);
            }
        }
        Ok(names)
    }

    pub(crate) fn emit_special_declarations(
        &mut self,
        function: FunctionId,
        names: &[(String, bool)],
        span: Span,
    ) -> Result<(), CompileError> {
        for (name, escaped) in names {
            let instruction = if *escaped {
                Instruction::DeclareSpecialExact(name.clone())
            } else {
                Instruction::DeclareSpecial(name.clone())
            };
            self.emit(function, instruction, span)?;
        }
        Ok(())
    }

    pub(super) fn compile_declare(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        let names = Self::special_names_in_declare(items)?;
        self.emit_special_declarations(function, &names, span)?;
        self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        Ok(())
    }

    pub(super) fn compile_locally(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.emit(function, Instruction::EnterScope, span)?;
        self.compile_sequence(function, items.get(1..).unwrap_or(&[]))?;
        self.emit(function, Instruction::ExitScope, span)?;
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

    pub(super) fn compile_with_compilation_unit(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(
                items,
                "WITH-COMPILATION-UNIT",
                "at least one",
                span,
            ));
        }
        self.compile_sequence(function, items.get(2..).unwrap_or(&[]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_declare_reports_an_internal_error_for_an_invalid_function_id() {
        let mut state = CompileState::default();
        let span = Span::new(0, 1);

        let error = state.compile_declare(99, span, &[]).map_or_else(
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
}
