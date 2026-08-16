use super::*;

impl CompileState {
    pub(super) fn compile_sequence(
        &mut self,
        function: FunctionId,
        forms: &[Form],
    ) -> Result<(), CompileError> {
        if forms.is_empty() {
            self.emit(
                function,
                Instruction::Constant(Constant::Nil),
                Span::new(0, 0),
            )?;
            return Ok(());
        }

        for (index, form) in forms.iter().enumerate() {
            self.compile_expression(function, form)?;
            if index + 1 < forms.len() {
                self.emit(function, Instruction::Pop, form.span)?;
            }
        }
        Ok(())
    }

    pub(super) fn compile_expression(
        &mut self,
        function: FunctionId,
        form: &Form,
    ) -> Result<(), CompileError> {
        match &form.kind {
            FormKind::Atom(atom) => {
                if let Some(constant) = literal_constant(atom) {
                    self.emit(function, Instruction::Constant(constant), form.span)?;
                } else if let Some((name, escaped)) = symbol_reference(atom) {
                    let instruction = if escaped {
                        Instruction::LoadExact(name)
                    } else {
                        Instruction::Load(name)
                    };
                    self.emit(function, instruction, form.span)?;
                } else {
                    self.emit(function, Instruction::Load(normalize_name(atom)), form.span)?;
                }
            }
            FormKind::String(value) => {
                self.emit(
                    function,
                    Instruction::Constant(Constant::String(value.clone())),
                    form.span,
                )?;
            }
            FormKind::Character(value) => {
                self.emit(
                    function,
                    Instruction::Constant(Constant::Character(*value)),
                    form.span,
                )?;
            }
            FormKind::Vector(_) => {
                self.emit(function, Instruction::Quote(form.clone()), form.span)?;
            }
            FormKind::DottedList { .. } => {
                return Err(CompileError::new(
                    CompileErrorKind::UnsupportedForm {
                        message: "dotted lists cannot be evaluated".to_string(),
                    },
                    form.span,
                ));
            }
            FormKind::List(items) => self.compile_list(function, form.span, items)?,
        }
        Ok(())
    }

    pub(super) fn compile_list(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        let Some(operator) = items.first() else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
            return Ok(());
        };

        let operator_name = match &operator.kind {
            FormKind::Atom(name) => special_operator_name(name),
            _ => None,
        };
        if let Some(name) = operator_name.as_deref() {
            match name {
                "QUOTE" => return self.compile_quote(function, span, items),
                "QUASIQUOTE" => return self.compile_quasiquote(function, span, items),
                "DECLARE" => return self.compile_declare(function, span),
                "LOCALLY" => return self.compile_progn(function, items),
                "WITH-COMPILATION-UNIT" => return self.compile_progn(function, items),
                "EVAL-WHEN" => return self.compile_eval_when(function, span, items),
                "LOAD-TIME-VALUE" => {
                    return self.compile_runtime_definition(function, span, items);
                }
                "NTH-VALUE" => {
                    return self.compile_nth_value(function, span, items);
                }
                "DECLAIM" | "PROCLAIM" => return self.compile_declare(function, span),
                "THE" => return self.compile_the(function, span, items),
                "IF" => return self.compile_if(function, span, items),
                "PROGN" => return self.compile_progn(function, items),
                "PROG1" => return self.compile_prog1(function, span, items),
                "PROG2" => return self.compile_prog2(function, span, items),
                "PROG" => return self.compile_prog(function, span, items, false),
                "PROG*" => return self.compile_prog(function, span, items, true),
                "VALUES" => return self.compile_values(function, span, items),
                "IGNORE-ERRORS" => return self.compile_ignore_errors(function, span, items),
                "HANDLER-CASE" => return self.compile_handler_case(function, span, items),
                "HANDLER-BIND" => return self.compile_handler_bind(function, span, items),
                "RESTART-BIND" => return self.compile_restart_bind(function, span, items),
                "CATCH" => return self.compile_catch(function, span, items),
                "WITH-SIMPLE-RESTART" => {
                    return self.compile_with_simple_restart(function, span, items);
                }
                "WITH-CONDITION-RESTARTS" => {
                    return self.compile_with_condition_restarts(function, span, items);
                }
                "WITH-OPEN-FILE" => {
                    return self.compile_with_open_file(function, span, items);
                }
                "WITH-OUTPUT-TO-STRING" => {
                    return self.compile_with_output_to_string(function, span, items);
                }
                "WITH-INPUT-FROM-STRING" => {
                    return self.compile_with_input_from_string(function, span, items);
                }
                "RESTART-CASE" => return self.compile_restart_case(function, span, items),
                "PROGV" => return self.compile_progv(function, span, items),
                "THROW" => return self.compile_throw(function, span, items),
                "UNWIND-PROTECT" => {
                    return self.compile_unwind_protect(function, span, items);
                }
                "BLOCK" => return self.compile_block(function, span, items),
                "RETURN" => return self.compile_return(function, span, items),
                "RETURN-FROM" => return self.compile_return_from(function, span, items),
                "TAGBODY" => return self.compile_tagbody(function, span, items),
                "GO" => return self.compile_go(function, span, items),
                "MULTIPLE-VALUE-BIND" => {
                    return self.compile_multiple_value_bind(function, span, items);
                }
                "MULTIPLE-VALUE-CALL" => {
                    return self.compile_multiple_value_call(function, span, items);
                }
                "MULTIPLE-VALUE-LIST" => {
                    return self.compile_multiple_value_list(function, span, items);
                }
                "MULTIPLE-VALUE-PROG1" => {
                    return self.compile_multiple_value_prog1(function, span, items);
                }
                "AND" => return self.compile_and(function, span, items),
                "OR" => return self.compile_or(function, span, items),
                "WHEN" => return self.compile_when(function, span, items, true),
                "UNLESS" => return self.compile_when(function, span, items, false),
                "COND" => return self.compile_cond(function, span, items),
                "CASE" | "ECASE" => return self.compile_case(function, span, items),
                "TYPECASE" | "ETYPECASE" => {
                    return self.compile_typecase(function, span, items);
                }
                "LAMBDA" => return self.compile_lambda(function, span, items),
                "FUNCTION" => return self.compile_function(function, span, items),
                "DEFINE" => return self.compile_define(function, span, items),
                "DEFINE-SYMBOL-MACRO" => {
                    return self.compile_runtime_definition(function, span, items);
                }
                "DEFUN" => return self.compile_defun(function, span, items),
                "SETQ" => return self.compile_setq(function, span, items),
                "PSETQ" => return self.compile_psetq(function, span, items),
                "MULTIPLE-VALUE-SETQ" => {
                    return self.compile_multiple_value_setq(function, span, items);
                }
                "SETF" => return self.compile_setf(function, span, items),
                "PSETF" | "PUSHNEW" | "REMF" | "ROTATEF" | "SHIFTF" => {
                    return self.compile_runtime_definition(function, span, items);
                }
                "PUSH" => {
                    if matches!(
                        items.get(2).map(|place| &place.kind),
                        Some(FormKind::Atom(_)) | None
                    ) {
                        return self.compile_push_symbol(function, span, items);
                    }
                    return self.compile_runtime_definition(function, span, items);
                }
                "POP" => {
                    if matches!(
                        items.get(1).map(|place| &place.kind),
                        Some(FormKind::Atom(_)) | None
                    ) {
                        return self.compile_pop_symbol(function, span, items);
                    }
                    return self.compile_runtime_definition(function, span, items);
                }
                "INCF" => {
                    if matches!(
                        items.get(1).map(|place| &place.kind),
                        Some(FormKind::Atom(_))
                    ) {
                        return self.compile_modify_symbol(function, span, items, "INCF", "+");
                    }
                    return self.compile_runtime_definition(function, span, items);
                }
                "DECF" => {
                    if matches!(
                        items.get(1).map(|place| &place.kind),
                        Some(FormKind::Atom(_))
                    ) {
                        return self.compile_modify_symbol(function, span, items, "DECF", "-");
                    }
                    return self.compile_runtime_definition(function, span, items);
                }
                "DEFVAR" => return self.compile_defvar(function, span, items, false),
                "DEFPARAMETER" => return self.compile_defvar(function, span, items, true),
                "DEFCONSTANT" => return self.compile_runtime_definition(function, span, items),
                "DEFSTRUCT" => return self.compile_defstruct(function, span, items),
                "DEFINE-CONDITION" => {
                    return self.compile_runtime_definition(function, span, items);
                }
                "DEFCLASS" | "DEFGENERIC" | "DEFMETHOD" => {
                    return self.compile_runtime_definition(function, span, items);
                }
                "DEFSETF"
                | "DEFINE-COMPILER-MACRO"
                | "DEFINE-MODIFY-MACRO"
                | "DEFINE-SETF-EXPANDER"
                | "GET-SETF-EXPANSION" => {
                    return self.compile_runtime_definition(function, span, items);
                }
                "EVAL" => return self.compile_eval(function, span, items),
                "FUNCALL" => return self.compile_funcall(function, span, items),
                "APPLY" => return self.compile_apply(function, span, items),
                "MAP-INTO" => return self.compile_map_into(function, span, items),
                "MAPCAR" => return self.compile_mapcar(function, span, items),
                "DESTRUCTURING-BIND" => {
                    return self.compile_destructuring_bind(function, span, items);
                }
                "LET" => return self.compile_let(function, span, items, false),
                "LET*" => return self.compile_let(function, span, items, true),
                "FLET" => return self.compile_flet(function, span, items, false),
                "LABELS" => return self.compile_flet(function, span, items, true),
                "DOTIMES" => return self.compile_dotimes(function, span, items),
                "DOLIST" => return self.compile_dolist(function, span, items),
                "DO" => return self.compile_do(function, span, items, false),
                "DO*" => return self.compile_do(function, span, items, true),
                _ => {}
            }
        }

        if let FormKind::Atom(name) = &operator.kind {
            let (reference_name, escaped) =
                symbol_reference(name).unwrap_or_else(|| (normalize_name(name), false));
            let local_function =
                self.is_local_function(&Self::local_function_key(&reference_name, escaped));
            self.emit(
                function,
                if local_function && escaped {
                    Instruction::FunctionLoadExact(reference_name)
                } else if local_function {
                    Instruction::FunctionLoad(reference_name)
                } else if escaped {
                    Instruction::FunctionLoadExact(reference_name)
                } else {
                    Instruction::FunctionLoad(reference_name)
                },
                operator.span,
            )?;
            for item in items.iter().skip(1) {
                self.compile_expression(function, item)?;
            }
        } else {
            for item in items {
                self.compile_expression(function, item)?;
            }
        }
        self.emit(
            function,
            Instruction::Call(items.len().saturating_sub(1)),
            span,
        )?;
        Ok(())
    }

    pub(super) fn compile_quote(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.require_arity(items, "QUOTE", "one", 1, span)?;
        let Some(argument) = items.get(1) else {
            return Err(self.internal_error(span, "missing quote argument after arity check"));
        };
        self.emit(
            function,
            Instruction::Quote(argument.clone()),
            argument.span,
        )?;
        Ok(())
    }

    pub(super) fn compile_quasiquote(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.require_arity(items, "QUASIQUOTE", "one", 1, span)?;
        let Some(argument) = items.get(1) else {
            return Err(self.internal_error(span, "missing quasiquote argument after arity check"));
        };
        self.emit(
            function,
            Instruction::QuasiQuote(argument.clone()),
            argument.span,
        )?;
        Ok(())
    }

    pub(super) fn compile_if(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if !(items.len() == 3 || items.len() == 4) {
            return Err(self.arity_error(items, "IF", "two or three", span));
        }
        let Some(condition) = items.get(1) else {
            return Err(self.internal_error(span, "missing if condition after arity check"));
        };
        let Some(then_branch) = items.get(2) else {
            return Err(self.internal_error(span, "missing if branch after arity check"));
        };

        self.compile_expression(function, condition)?;
        let false_jump = self.emit(
            function,
            Instruction::JumpIfFalse(usize::MAX),
            condition.span,
        )?;
        self.compile_expression(function, then_branch)?;
        let end_jump = self.emit(function, Instruction::Jump(usize::MAX), then_branch.span)?;
        let else_target = self.instruction_count(function, span)?;
        self.patch_jump(function, false_jump, else_target, condition.span)?;

        if let Some(else_branch) = items.get(3) {
            self.compile_expression(function, else_branch)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        }
        let end_target = self.instruction_count(function, span)?;
        self.patch_jump(function, end_jump, end_target, span)?;
        Ok(())
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
    ) -> Result<(), CompileError> {
        self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        Ok(())
    }

    pub(super) fn compile_the(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.require_arity(items, "THE", "two", 2, span)?;
        let Some(type_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing THE type after arity check"));
        };
        let Some(value_form) = items.get(2) else {
            return Err(self.internal_error(span, "missing THE value after arity check"));
        };
        self.emit(
            function,
            Instruction::FunctionLoad("__NCL_THE_CHECK".to_string()),
            span,
        )?;
        self.compile_expression(function, value_form)?;
        self.emit(
            function,
            Instruction::Quote(type_form.clone()),
            type_form.span,
        )?;
        self.emit(function, Instruction::Call(2), span)?;
        Ok(())
    }

    pub(super) fn compile_eval_when(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "EVAL-WHEN", "at least one", span));
        }
        if compile_eval_when_executes(&items[1])? {
            self.compile_sequence(function, items.get(2..).unwrap_or(&[]))
        } else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
            Ok(())
        }
    }

    pub(super) fn compile_prog1(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "PROG1", "at least one", span));
        }

        let Some(first) = items.get(1) else {
            return Err(self.internal_error(span, "missing PROG1 form after arity check"));
        };
        let retained = self.fresh_name("PROG1_VALUE");

        self.emit(function, Instruction::EnterScope, first.span)?;
        self.compile_expression(function, first)?;
        self.emit(function, Instruction::Define(retained.clone()), first.span)?;
        self.emit(function, Instruction::Pop, first.span)?;

        let tail = items.get(2..).unwrap_or(&[]);
        if !tail.is_empty() {
            self.compile_sequence(function, tail)?;
            self.emit(function, Instruction::Pop, span)?;
        }

        self.emit(function, Instruction::Load(retained), span)?;
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }

    pub(super) fn compile_prog2(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(self.arity_error(items, "PROG2", "at least two", span));
        }

        let Some(first) = items.get(1) else {
            return Err(self.internal_error(span, "missing first PROG2 form after arity check"));
        };
        let Some(second) = items.get(2) else {
            return Err(self.internal_error(span, "missing second PROG2 form after arity check"));
        };
        let retained = self.fresh_name("PROG2_VALUE");

        self.emit(function, Instruction::EnterScope, first.span)?;
        self.compile_expression(function, first)?;
        self.emit(function, Instruction::Pop, first.span)?;
        self.compile_expression(function, second)?;
        self.emit(function, Instruction::Define(retained.clone()), second.span)?;
        self.emit(function, Instruction::Pop, second.span)?;

        let tail = items.get(3..).unwrap_or(&[]);
        if !tail.is_empty() {
            self.compile_sequence(function, tail)?;
            self.emit(function, Instruction::Pop, span)?;
        }

        self.emit(function, Instruction::Load(retained), span)?;
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }

    pub(super) fn compile_prog(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        sequential: bool,
    ) -> Result<(), CompileError> {
        let operator = if sequential { "PROG*" } else { "PROG" };
        if items.len() < 2 {
            return Err(self.arity_error(items, operator, "at least one", span));
        }
        let Some(binding_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing PROG bindings after arity check"));
        };
        let FormKind::List(binding_forms) = &binding_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "PROG bindings".to_string(),
                },
                binding_form.span,
            ));
        };

        let mut names = HashSet::new();
        let mut parsed = Vec::with_capacity(binding_forms.len());
        for binding in binding_forms {
            let (name_form, init) = match &binding.kind {
                FormKind::Atom(_) => (binding, None),
                FormKind::List(parts) => {
                    if !(1..=2).contains(&parts.len()) {
                        return Err(CompileError::new(
                            CompileErrorKind::InvalidForm {
                                message: "PROG binding needs a name and optional value".to_string(),
                            },
                            binding.span,
                        ));
                    }
                    let Some(name_form) = parts.first() else {
                        return Err(self.internal_error(binding.span, "missing PROG binding name"));
                    };
                    (name_form, parts.get(1).cloned())
                }
                _ => {
                    return Err(CompileError::new(
                        CompileErrorKind::ExpectedSymbol {
                            context: "PROG binding name".to_string(),
                        },
                        binding.span,
                    ));
                }
            };
            let (name, escaped) = self.symbol_name_info(name_form, "PROG binding name")?;
            let key = if escaped {
                format!("\0{name}")
            } else {
                name.clone()
            };
            if !names.insert(key) {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "PROG binding names must be unique".to_string(),
                    },
                    name_form.span,
                ));
            }
            parsed.push((name, escaped, init));
        }

        let prog_function = self.reserve_function(None, Vec::new());
        self.emit(prog_function, Instruction::EnterScope, binding_form.span)?;

        if sequential {
            for (name, escaped, init) in &parsed {
                if let Some(init) = init {
                    self.compile_expression(prog_function, init)?;
                } else {
                    self.emit(
                        prog_function,
                        Instruction::Constant(Constant::Nil),
                        binding_form.span,
                    )?;
                }
                let define = if *escaped {
                    Instruction::DefineExact(name.clone())
                } else {
                    Instruction::Define(name.clone())
                };
                self.emit(prog_function, define, binding_form.span)?;
                self.emit(prog_function, Instruction::Pop, binding_form.span)?;
            }
        } else {
            let mut initial_temporaries = Vec::with_capacity(parsed.len());
            for (_, _, init) in &parsed {
                if let Some(init) = init {
                    self.compile_expression(prog_function, init)?;
                } else {
                    self.emit(
                        prog_function,
                        Instruction::Constant(Constant::Nil),
                        binding_form.span,
                    )?;
                }
                let temporary = self.fresh_name("PROG_INIT");
                self.emit(
                    prog_function,
                    Instruction::Define(temporary.clone()),
                    binding_form.span,
                )?;
                self.emit(prog_function, Instruction::Pop, binding_form.span)?;
                initial_temporaries.push(temporary);
            }
            for ((name, escaped, _), temporary) in parsed.iter().zip(initial_temporaries) {
                self.emit(
                    prog_function,
                    Instruction::Load(temporary),
                    binding_form.span,
                )?;
                let define = if *escaped {
                    Instruction::DefineExact(name.clone())
                } else {
                    Instruction::Define(name.clone())
                };
                self.emit(prog_function, define, binding_form.span)?;
                self.emit(prog_function, Instruction::Pop, binding_form.span)?;
            }
        }

        self.compile_tagbody_forms(prog_function, span, items.get(2..).unwrap_or(&[]))?;
        self.emit(prog_function, Instruction::ExitScope, span)?;
        self.emit(prog_function, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::Block {
                function: prog_function,
                name: "NIL".to_string(),
            },
            span,
        )?;
        Ok(())
    }

    pub(super) fn compile_values(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        for item in items.get(1..).unwrap_or(&[]) {
            self.compile_expression(function, item)?;
            self.emit(function, Instruction::Primary, item.span)?;
        }
        self.emit(
            function,
            Instruction::Values(items.len().saturating_sub(1)),
            span,
        )?;
        Ok(())
    }

    pub(super) fn compile_nth_value(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.require_arity(items, "NTH-VALUE", "two", 2, span)?;
        let Some(index_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing NTH-VALUE index form after arity check"));
        };
        let Some(value_form) = items.get(2) else {
            return Err(self.internal_error(span, "missing NTH-VALUE value form after arity check"));
        };
        self.compile_expression(function, index_form)?;
        self.compile_expression(function, value_form)?;
        self.emit(function, Instruction::NthValue(index_form.span), span)?;
        Ok(())
    }

    pub(super) fn compile_multiple_value_list(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.require_arity(items, "MULTIPLE-VALUE-LIST", "one", 1, span)?;
        let Some(value_form) = items.get(1) else {
            return Err(self.internal_error(
                span,
                "missing MULTIPLE-VALUE-LIST value form after arity check",
            ));
        };
        self.compile_expression(function, value_form)?;
        self.emit(function, Instruction::MultipleValueList, value_form.span)?;
        Ok(())
    }
}
