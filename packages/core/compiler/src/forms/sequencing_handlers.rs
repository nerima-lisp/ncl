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

    pub(super) fn compile_locally(
        &mut self,
        function: FunctionId,
        items: &[Form],
    ) -> Result<(), CompileError> {
        let saved_special_names = self.special_names.clone();
        let declarations = (|| {
            for form in items.iter().skip(1) {
                let FormKind::List(declaration) = &form.kind else {
                    break;
                };
                let Some(Form {
                    kind: FormKind::Atom(operator),
                    ..
                }) = declaration.first()
                else {
                    break;
                };
                if !operator.eq_ignore_ascii_case("DECLARE") {
                    break;
                }
                for spec in declaration.iter().skip(1) {
                    let FormKind::List(spec) = &spec.kind else {
                        continue;
                    };
                    let Some(Form {
                        kind: FormKind::Atom(kind),
                        ..
                    }) = spec.first()
                    else {
                        continue;
                    };
                    if !kind.eq_ignore_ascii_case("SPECIAL") {
                        continue;
                    }
                    for name in spec.iter().skip(1) {
                        let (name, escaped) =
                            Self::symbol_name_info(name, "special declaration name")?;
                        self.register_special(name, escaped);
                    }
                }
            }
            Ok::<(), CompileError>(())
        })();
        if let Err(error) = declarations {
            self.special_names = saved_special_names;
            return Err(error);
        }
        let result = self.compile_sequence(function, items.get(1..).unwrap_or(&[]));
        self.special_names = saved_special_names;
        result
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
                    return Err(CompileError::new(
                        crate::CompileErrorKind::InvalidForm {
                            message: "declaration must be a proper list".into(),
                        },
                        declaration.span,
                    ));
                };
                let Some(Form {
                    kind: FormKind::Atom(kind),
                    ..
                }) = declaration_items.first()
                else {
                    return Err(CompileError::new(
                        crate::CompileErrorKind::InvalidForm {
                            message: "declaration must name a declaration type".into(),
                        },
                        declaration.span,
                    ));
                };
                if kind.eq_ignore_ascii_case("SPECIAL")
                    || kind.eq_ignore_ascii_case("IGNORE")
                    || kind.eq_ignore_ascii_case("IGNORABLE")
                {
                    for name in declaration_items.iter().skip(1) {
                        let (name, escaped) = Self::symbol_name_info(
                            name,
                            if kind.eq_ignore_ascii_case("SPECIAL") {
                                "special declaration name"
                            } else {
                                "ignored declaration name"
                            },
                        )?;
                        if kind.eq_ignore_ascii_case("SPECIAL") {
                            self.register_special(name, escaped);
                        }
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

    #[test]
    fn compile_locally_restores_special_names_when_declaration_is_invalid() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let forms = ncl_syntax::read("(locally (declare (special (x))))")
            .unwrap_or_else(|error| panic!("test source should parse: {error}"));
        let before = state.special_names.clone();

        assert!(state.compile_sequence(function, &forms).is_err());
        assert_eq!(state.special_names, before);
    }

    #[test]
    fn compile_declaim_accepts_standard_ignore_declarations() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let forms = ncl_syntax::read("(declaim (ignore x) (ignorable y))")
            .unwrap_or_else(|error| panic!("standard declarations should parse: {error}"));

        state
            .compile_sequence(function, &forms)
            .unwrap_or_else(|error| panic!("IGNORE declarations should compile: {error}"));
    }

    #[test]
    fn compile_declaim_rejects_non_list_declarations() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let forms = ncl_syntax::read("(declaim special)")
            .unwrap_or_else(|error| panic!("test source should parse: {error}"));

        let error = state
            .compile_sequence(function, &forms)
            .map_or_else(|error| error, |_| panic!("malformed declaration should fail"));

        assert!(matches!(error.kind, CompileErrorKind::InvalidForm { .. }));
    }
}
