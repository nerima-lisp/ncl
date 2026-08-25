use super::*;

#[path = "state_conditionals.rs"]
mod state_conditionals;
#[path = "state_definitions.rs"]
mod state_definitions;
#[path = "state_destructuring.rs"]
mod state_destructuring;
#[path = "state_errors.rs"]
mod state_errors;
#[path = "state_iteration.rs"]
mod state_iteration;
#[path = "state_names.rs"]
mod state_names;

impl CompileState {
    fn is_local_function(&self, name: &str) -> bool {
        self.local_function_scopes
            .iter()
            .rev()
            .any(|scope| scope.contains(name))
    }

    pub(crate) fn reserve_function(
        &mut self,
        name: Option<String>,
        parameters: Vec<String>,
    ) -> FunctionId {
        let required_escaped = vec![false; parameters.len()];
        self.reserve_function_with_rest(name, parameters, required_escaped, None, false)
    }

    fn reserve_function_with_rest(
        &mut self,
        name: Option<String>,
        parameters: Vec<String>,
        required_escaped: Vec<bool>,
        rest: Option<String>,
        rest_escaped: bool,
    ) -> FunctionId {
        let function = self.functions.len();
        self.functions.push(FunctionCode {
            name,
            parameters,
            required_escaped,
            optional: Vec::new(),
            keywords: Vec::new(),
            has_keyword_section: false,
            allow_other_keys: false,
            rest,
            rest_escaped,
            auxiliary: Vec::new(),
            instructions: Vec::new(),
        });
        function
    }

    fn local_function_key(name: &str, escaped: bool) -> String {
        if escaped {
            format!("\0{name}")
        } else {
            normalize_name(name)
        }
    }

    pub(crate) fn emit(
        &mut self,
        function: FunctionId,
        instruction: Instruction,
        span: Span,
    ) -> Result<usize, CompileError> {
        let Some(code) = self.functions.get_mut(function) else {
            return Err(self.internal_error(span, "invalid function id while emitting bytecode"));
        };
        let position = code.instructions.len();
        code.instructions.push(instruction);
        Ok(position)
    }

    pub(super) fn instruction_count(
        &self,
        function: FunctionId,
        span: Span,
    ) -> Result<usize, CompileError> {
        self.functions
            .get(function)
            .map(|code| code.instructions.len())
            .ok_or_else(|| self.internal_error(span, "invalid function id while reading bytecode"))
    }

    pub(super) fn patch_jump(
        &mut self,
        function: FunctionId,
        instruction: usize,
        target: usize,
        span: Span,
    ) -> Result<(), CompileError> {
        let Some(code) = self.functions.get_mut(function) else {
            return Err(self.internal_error(span, "invalid function id while patching jump"));
        };
        let Some(operation) = code.instructions.get_mut(instruction) else {
            return Err(self.internal_error(span, "invalid jump instruction position"));
        };
        match operation {
            Instruction::JumpIfFalse(value) | Instruction::Jump(value) => {
                *value = target;
                Ok(())
            }
            _ => Err(self.internal_error(span, "attempted to patch a non-jump instruction")),
        }
    }

    pub(crate) fn collect_names(&mut self, forms: &[Form]) {
        for form in forms {
            self.collect_form_names(form);
        }
    }

    fn collect_form_names(&mut self, form: &Form) {
        match &form.kind {
            FormKind::Atom(name) => {
                self.used_names.insert(normalize_name(name));
            }
            FormKind::List(items) | FormKind::Vector(items) => {
                for item in items {
                    self.collect_form_names(item);
                }
            }
            FormKind::DottedList { items, tail } => {
                for item in items {
                    self.collect_form_names(item);
                }
                self.collect_form_names(tail);
            }
            FormKind::String(_) | FormKind::Character(_) => {}
        }
    }

    pub(super) fn fresh_name(&mut self, prefix: &str) -> String {
        loop {
            let candidate = format!("__NCL_{prefix}_{}", self.temporary_counter);
            self.temporary_counter += 1;
            if self.used_names.insert(candidate.clone()) {
                return candidate;
            }
        }
    }

    pub(crate) fn compile_sequence(
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

    fn compile_list(
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
                "EVAL-WHEN" => return self.compile_eval_when(function, span, items),
                "LOAD-TIME-VALUE" => {
                    return self.compile_runtime_definition(function, span, items);
                }
                "NTH-VALUE" => {
                    return self.compile_runtime_definition(function, span, items);
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
                "PSETF" | "PUSH" | "POP" | "PUSHNEW" | "ROTATEF" | "SHIFTF" => {
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
                "DEFCLASS" | "DEFGENERIC" | "DEFMETHOD" => {
                    return self.compile_runtime_definition(function, span, items);
                }
                "DEFSETF"
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

    fn compile_quote(
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

    fn compile_quasiquote(
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

    fn compile_if(
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

    fn compile_progn(&mut self, function: FunctionId, items: &[Form]) -> Result<(), CompileError> {
        let forms = items.get(1..).unwrap_or(&[]);
        self.compile_sequence(function, forms)
    }

    fn compile_declare(&mut self, function: FunctionId, span: Span) -> Result<(), CompileError> {
        self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        Ok(())
    }

    fn compile_the(
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

    fn compile_eval_when(
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

    fn compile_prog1(
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

    fn compile_prog2(
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

    fn compile_prog(
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

    fn compile_values(
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

    fn compile_multiple_value_list(
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

    fn compile_ignore_errors(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        let child = self.reserve_function(None, Vec::new());
        self.compile_sequence(child, &items[1..])?;
        self.emit(child, Instruction::Return, span)?;
        self.emit(function, Instruction::IgnoreErrors(child), span)?;
        Ok(())
    }

    fn compile_handler_case(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(self.arity_error(items, "HANDLER-CASE", "at least two", span));
        }

        let protected = items
            .get(1)
            .ok_or_else(|| self.internal_error(span, "missing HANDLER-CASE protected form"))?;
        let protected_function = self.reserve_function(None, Vec::new());
        self.compile_expression(protected_function, protected)?;
        self.emit(protected_function, Instruction::Return, protected.span)?;

        let mut clauses = Vec::with_capacity(items.len().saturating_sub(2));
        for clause in &items[2..] {
            let FormKind::List(clause_items) = &clause.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "handler-case clause".to_string(),
                    },
                    clause.span,
                ));
            };
            if clause_items.len() < 2 {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "handler-case clause needs a condition and variable list"
                            .to_string(),
                    },
                    clause.span,
                ));
            }
            let condition = self.condition_name(&clause_items[0], "handler-case condition")?;
            let FormKind::List(variable_items) = &clause_items[1].kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "handler-case variable list".to_string(),
                    },
                    clause_items[1].span,
                ));
            };
            if variable_items.len() > 1 {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "handler-case variable list accepts at most one variable"
                            .to_string(),
                    },
                    clause_items[1].span,
                ));
            }
            let variable = variable_items
                .first()
                .map(|form| self.symbol_name_info(form, "handler-case variable"))
                .transpose()?;
            let parameters = variable
                .as_ref()
                .map(|(name, _)| name.clone())
                .into_iter()
                .collect();
            let required_escaped = variable
                .as_ref()
                .map(|(_, escaped)| *escaped)
                .into_iter()
                .collect();
            let clause_function =
                self.reserve_function_with_rest(None, parameters, required_escaped, None, false);
            self.compile_sequence(clause_function, &clause_items[2..])?;
            self.emit(clause_function, Instruction::Return, clause.span)?;
            clauses.push(HandlerCaseClause {
                condition,
                variable: variable.map(|(name, _)| name),
                function: clause_function,
            });
        }

        self.emit(
            function,
            Instruction::HandlerCase {
                protected: protected_function,
                clauses,
            },
            span,
        )?;
        Ok(())
    }

    fn compile_handler_bind(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "HANDLER-BIND", "at least one", span));
        }
        let handler_form = items
            .get(1)
            .ok_or_else(|| self.internal_error(span, "missing HANDLER-BIND handler list"))?;
        let FormKind::List(handler_items) = &handler_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "handler-bind handler list".to_string(),
                },
                handler_form.span,
            ));
        };

        let mut handlers = Vec::with_capacity(handler_items.len());
        for handler in handler_items {
            let FormKind::List(handler_clause) = &handler.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "handler-bind clause".to_string(),
                    },
                    handler.span,
                ));
            };
            if handler_clause.len() != 2 {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "handler-bind clause needs a condition and handler".to_string(),
                    },
                    handler.span,
                ));
            }
            let condition = self.condition_name(&handler_clause[0], "handler-bind condition")?;
            let condition_variable = self.fresh_name("HANDLER_CONDITION");
            let clause_function = self.reserve_function(None, vec![condition_variable.clone()]);
            self.compile_expression(clause_function, &handler_clause[1])?;
            self.compile_expression(
                clause_function,
                &Form::atom(condition_variable, handler_clause[1].span),
            )?;
            self.emit(
                clause_function,
                Instruction::Call(1),
                handler_clause[1].span,
            )?;
            self.emit(clause_function, Instruction::Return, handler.span)?;
            handlers.push(HandlerBindClause {
                condition,
                function: clause_function,
            });
        }

        let body_function = self.reserve_function(None, Vec::new());
        self.compile_sequence(body_function, items.get(2..).unwrap_or(&[]))?;
        self.emit(body_function, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::HandlerBind {
                body: body_function,
                handlers,
            },
            span,
        )?;
        Ok(())
    }

    fn compile_restart_bind(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "RESTART-BIND", "at least one", span));
        }
        let binding_form = items
            .get(1)
            .ok_or_else(|| self.internal_error(span, "missing RESTART-BIND binding list"))?;
        let FormKind::List(binding_items) = &binding_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "restart-bind binding list".to_string(),
                },
                binding_form.span,
            ));
        };

        let mut bindings = Vec::with_capacity(binding_items.len());
        for binding in binding_items {
            let FormKind::List(binding_clause) = &binding.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "restart-bind clause".to_string(),
                    },
                    binding.span,
                ));
            };
            if binding_clause.len() != 2 {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "restart-bind clause needs a name and function".to_string(),
                    },
                    binding.span,
                ));
            }
            let name = self.control_name(&binding_clause[0], "RESTART-BIND restart name")?;
            let binding_function = self.reserve_function(None, Vec::new());
            self.compile_expression(binding_function, &binding_clause[1])?;
            self.emit(
                binding_function,
                Instruction::Return,
                binding_clause[1].span,
            )?;
            bindings.push(RestartBindClause {
                name,
                function: binding_function,
            });
        }

        let body_function = self.reserve_function(None, Vec::new());
        self.compile_sequence(body_function, items.get(2..).unwrap_or(&[]))?;
        self.emit(body_function, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::RestartBind {
                body: body_function,
                bindings,
            },
            span,
        )?;
        Ok(())
    }

    fn compile_catch(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "CATCH", "at least one", span));
        }

        let tag_function = self.reserve_function(None, Vec::new());
        self.compile_expression(tag_function, &items[1])?;
        self.emit(tag_function, Instruction::Return, items[1].span)?;

        let body_function = self.reserve_function(None, Vec::new());
        self.compile_sequence(body_function, items.get(2..).unwrap_or(&[]))?;
        self.emit(body_function, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::Catch {
                tag: tag_function,
                body: body_function,
            },
            span,
        )?;
        Ok(())
    }

    fn compile_with_simple_restart(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "WITH-SIMPLE-RESTART", "at least one", span));
        }

        let clause = &items[1];
        let FormKind::List(parts) = &clause.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "WITH-SIMPLE-RESTART restart clause".to_string(),
                },
                clause.span,
            ));
        };
        if parts.len() < 2 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "WITH-SIMPLE-RESTART restart clause needs a name and report format"
                        .to_string(),
                },
                clause.span,
            ));
        }

        let name = self.control_name(&parts[0], "WITH-SIMPLE-RESTART name")?;
        let body = self.reserve_function(None, Vec::new());
        self.compile_sequence(body, items.get(2..).unwrap_or(&[]))?;
        self.emit(body, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::WithSimpleRestart { name, body },
            span,
        )?;
        Ok(())
    }

    fn compile_restart_case(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(self.arity_error(items, "RESTART-CASE", "at least two", span));
        }

        let protected = items
            .get(1)
            .ok_or_else(|| self.internal_error(span, "missing RESTART-CASE protected form"))?;
        let protected_function = self.reserve_function(None, Vec::new());
        self.compile_expression(protected_function, protected)?;
        self.emit(protected_function, Instruction::Return, protected.span)?;

        let mut clauses = Vec::with_capacity(items.len().saturating_sub(2));
        for clause in &items[2..] {
            let FormKind::List(clause_items) = &clause.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "restart-case clause".to_string(),
                    },
                    clause.span,
                ));
            };
            if clause_items.len() < 2 {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "restart-case clause needs a name, lambda list, and body"
                            .to_string(),
                    },
                    clause.span,
                ));
            }
            let name = self.control_name(&clause_items[0], "RESTART-CASE restart name")?;
            let lambda_list = self.parameters(&clause_items[1])?;
            let clause_function = self.reserve_function_with_rest(
                None,
                lambda_list.required.clone(),
                lambda_list.required_escaped.clone(),
                lambda_list.rest.clone(),
                lambda_list.rest_escaped,
            );
            let optional = self.compile_optional_parameters(&lambda_list.optional)?;
            self.functions[clause_function].optional = optional;
            let keywords = self.compile_keyword_parameters(&lambda_list.keywords)?;
            self.functions[clause_function].keywords = keywords;
            self.functions[clause_function].has_keyword_section = lambda_list.has_keyword_section;
            self.functions[clause_function].allow_other_keys = lambda_list.allow_other_keys;
            let auxiliary = self.compile_auxiliary_parameters(&lambda_list.auxiliary)?;
            self.functions[clause_function].auxiliary = auxiliary;
            self.compile_sequence(clause_function, &clause_items[2..])?;
            self.emit(clause_function, Instruction::Return, clause.span)?;
            clauses.push(RestartCaseClause {
                name,
                function: clause_function,
            });
        }

        self.emit(
            function,
            Instruction::RestartCase {
                protected: protected_function,
                clauses,
            },
            span,
        )?;
        Ok(())
    }

    fn compile_with_condition_restarts(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 4 {
            return Err(self.arity_error(items, "WITH-CONDITION-RESTARTS", "at least three", span));
        }

        let condition = self.reserve_function(None, Vec::new());
        self.compile_expression(condition, &items[1])?;
        self.emit(condition, Instruction::Return, items[1].span)?;

        let restarts = self.reserve_function(None, Vec::new());
        self.compile_expression(restarts, &items[2])?;
        self.emit(restarts, Instruction::Return, items[2].span)?;

        let body = self.reserve_function(None, Vec::new());
        self.compile_sequence(body, &items[3..])?;
        self.emit(body, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::WithConditionRestarts {
                condition,
                restarts,
                body,
            },
            span,
        )?;
        Ok(())
    }

    fn compile_throw(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() != 3 {
            return Err(self.arity_error(items, "THROW", "two", span));
        }

        self.compile_expression(function, &items[1])?;
        self.compile_expression(function, &items[2])?;
        self.emit(function, Instruction::Throw, span)?;
        Ok(())
    }

    fn compile_progv(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(self.arity_error(items, "PROGV", "at least two", span));
        }

        let symbols_function = self.reserve_function(None, Vec::new());
        self.compile_expression(symbols_function, &items[1])?;
        self.emit(symbols_function, Instruction::Return, items[1].span)?;

        let values_function = self.reserve_function(None, Vec::new());
        self.compile_expression(values_function, &items[2])?;
        self.emit(values_function, Instruction::Return, items[2].span)?;

        let body_function = self.reserve_function(None, Vec::new());
        self.compile_sequence(body_function, items.get(3..).unwrap_or(&[]))?;
        self.emit(body_function, Instruction::Return, span)?;

        self.emit(
            function,
            Instruction::Progv {
                symbols: symbols_function,
                values: values_function,
                body: body_function,
            },
            span,
        )?;
        Ok(())
    }

    fn compile_unwind_protect(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "UNWIND-PROTECT", "at least one", span));
        }

        let protected = items.get(1).ok_or_else(|| {
            self.internal_error(
                span,
                "missing UNWIND-PROTECT protected form after arity check",
            )
        })?;
        let protected_function = self.reserve_function(None, Vec::new());
        self.compile_expression(protected_function, protected)?;
        self.emit(protected_function, Instruction::Return, protected.span)?;

        let cleanup_function = self.reserve_function(None, Vec::new());
        self.compile_sequence(cleanup_function, items.get(2..).unwrap_or(&[]))?;
        self.emit(cleanup_function, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::UnwindProtect {
                protected: protected_function,
                cleanup: cleanup_function,
            },
            span,
        )?;
        Ok(())
    }

    fn compile_with_open_file(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "WITH-OPEN-FILE", "at least one", span));
        }
        let binding_form = items.get(1).ok_or_else(|| {
            self.internal_error(span, "missing WITH-OPEN-FILE binding after arity check")
        })?;
        let FormKind::List(binding) = &binding_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "WITH-OPEN-FILE binding".to_string(),
                },
                binding_form.span,
            ));
        };
        if binding.len() < 2 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "WITH-OPEN-FILE binding needs a stream variable and pathname"
                        .to_string(),
                },
                binding_form.span,
            ));
        }
        self.symbol_name(&binding[0], "WITH-OPEN-FILE stream variable")?;

        let mut open_items = Vec::with_capacity(binding.len());
        open_items.push(Form::atom("OPEN", binding_form.span));
        open_items.extend(binding[1..].iter().cloned());
        let open_form = Form::list(open_items, binding_form.span);
        let generated_binding = Form::list(
            vec![Form::list(
                vec![binding[0].clone(), open_form],
                binding_form.span,
            )],
            binding_form.span,
        );
        let body = if items.len() > 2 {
            let mut body_items = Vec::with_capacity(items.len() - 1);
            body_items.push(Form::atom("PROGN", span));
            body_items.extend(items[2..].iter().cloned());
            Form::list(body_items, span)
        } else {
            Form::atom("NIL", span)
        };
        let close_form = Form::list(vec![Form::atom("CLOSE", span), binding[0].clone()], span);
        let protected_form = Form::list(
            vec![Form::atom("UNWIND-PROTECT", span), body, close_form],
            span,
        );
        let expanded = Form::list(
            vec![Form::atom("LET", span), generated_binding, protected_form],
            span,
        );
        self.compile_expression(function, &expanded)
    }

    fn compile_block(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "BLOCK", "at least one", span));
        }
        let name = self.control_name(
            items
                .get(1)
                .ok_or_else(|| self.internal_error(span, "missing BLOCK name after arity check"))?,
            "BLOCK name",
        )?;
        let child = self.reserve_function(None, Vec::new());
        self.compile_sequence(child, items.get(2..).unwrap_or(&[]))?;
        self.emit(child, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::Block {
                function: child,
                name,
            },
            span,
        )?;
        Ok(())
    }

    fn compile_return(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if !(items.len() == 1 || items.len() == 2) {
            return Err(self.arity_error(items, "RETURN", "zero or one", span));
        }
        if let Some(value) = items.get(1) {
            self.compile_expression(function, value)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        }
        self.emit(
            function,
            Instruction::ReturnFrom {
                name: "NIL".to_string(),
            },
            span,
        )?;
        Ok(())
    }

    fn compile_return_from(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if !(items.len() == 2 || items.len() == 3) {
            return Err(self.arity_error(items, "RETURN-FROM", "one or two", span));
        }
        let name = self.control_name(
            items.get(1).ok_or_else(|| {
                self.internal_error(span, "missing RETURN-FROM name after arity check")
            })?,
            "RETURN-FROM name",
        )?;
        if let Some(value) = items.get(2) {
            self.compile_expression(function, value)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        }
        self.emit(function, Instruction::ReturnFrom { name }, span)?;
        Ok(())
    }

    fn compile_tagbody(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.compile_tagbody_forms(function, span, items.get(1..).unwrap_or(&[]))
    }

    fn compile_tagbody_forms(
        &mut self,
        function: FunctionId,
        span: Span,
        forms: &[Form],
    ) -> Result<(), CompileError> {
        let child = self.reserve_function(None, Vec::new());
        let mut tags = Vec::new();

        for form in forms {
            if let Some(tag) = tag_name(form) {
                if tags.iter().any(|(existing, _)| existing == &tag) {
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidForm {
                            message: format!("duplicate TAGBODY tag {tag}"),
                        },
                        form.span,
                    ));
                }
                let position = self.instruction_count(child, form.span)?;
                tags.push((tag, position));
            } else {
                self.compile_expression(child, form)?;
                self.emit(child, Instruction::Pop, form.span)?;
            }
        }

        self.emit(child, Instruction::Constant(Constant::Nil), span)?;
        self.emit(child, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::TagBody {
                function: child,
                tags,
            },
            span,
        )?;
        Ok(())
    }

    fn compile_go(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.require_arity(items, "GO", "one", 1, span)?;
        let tag = self.control_tag(
            items
                .get(1)
                .ok_or_else(|| self.internal_error(span, "missing GO tag after arity check"))?,
            "GO tag",
        )?;
        self.emit(function, Instruction::Go { tag }, span)?;
        Ok(())
    }

    fn compile_multiple_value_bind(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(self.arity_error(items, "MULTIPLE-VALUE-BIND", "at least two", span));
        }
        let Some(variable_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing MULTIPLE-VALUE-BIND variables"));
        };
        let FormKind::List(variables) = &variable_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "MULTIPLE-VALUE-BIND variables".to_string(),
                },
                variable_form.span,
            ));
        };
        let mut names = Vec::with_capacity(variables.len());
        for variable in variables {
            names.push(self.symbol_name_info(variable, "MULTIPLE-VALUE-BIND variable")?);
        }
        let Some(value_form) = items.get(2) else {
            return Err(self.internal_error(span, "missing MULTIPLE-VALUE-BIND value form"));
        };

        self.emit(function, Instruction::EnterScope, variable_form.span)?;
        self.compile_expression(function, value_form)?;
        let has_exact = names.iter().any(|(_, escaped)| *escaped);
        let instruction = if has_exact {
            Instruction::BindValuesExact(names)
        } else {
            Instruction::BindValues(names.into_iter().map(|(name, _)| name).collect())
        };
        self.emit(function, instruction, value_form.span)?;
        self.compile_sequence(function, items.get(3..).unwrap_or(&[]))?;
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }

    fn compile_multiple_value_call(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "MULTIPLE-VALUE-CALL", "at least one", span));
        }
        let Some(function_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing MULTIPLE-VALUE-CALL function"));
        };
        self.compile_expression(function, function_form)?;
        self.emit(function, Instruction::Primary, function_form.span)?;
        for value_form in items.get(2..).unwrap_or(&[]) {
            self.compile_expression(function, value_form)?;
        }
        self.emit(
            function,
            Instruction::MultipleValueCall(items.len().saturating_sub(2)),
            span,
        )?;
        Ok(())
    }

    fn compile_multiple_value_prog1(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "MULTIPLE-VALUE-PROG1", "at least one", span));
        }
        let Some(first) = items.get(1) else {
            return Err(
                self.internal_error(span, "missing MULTIPLE-VALUE-PROG1 form after arity check")
            );
        };
        let retained = self.fresh_name("MULTIPLE_VALUE_PROG1_VALUE");

        self.emit(function, Instruction::EnterScope, first.span)?;
        self.compile_expression(function, first)?;
        self.emit(
            function,
            Instruction::DefineValues(retained.clone()),
            first.span,
        )?;
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

    fn compile_lambda(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "lambda needs parameters and a body".to_string(),
                },
                operator_span(items, span),
            ));
        }
        let Some(parameter_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing lambda parameters after arity check"));
        };
        let lambda_list = self.parameters(parameter_form)?;
        let child = self.reserve_function_with_rest(
            None,
            lambda_list.required.clone(),
            lambda_list.required_escaped.clone(),
            lambda_list.rest.clone(),
            lambda_list.rest_escaped,
        );
        let optional = self.compile_optional_parameters(&lambda_list.optional)?;
        self.functions[child].optional = optional;
        let keywords = self.compile_keyword_parameters(&lambda_list.keywords)?;
        self.functions[child].keywords = keywords;
        self.functions[child].has_keyword_section = lambda_list.has_keyword_section;
        self.functions[child].allow_other_keys = lambda_list.allow_other_keys;
        let auxiliary = self.compile_auxiliary_parameters(&lambda_list.auxiliary)?;
        self.functions[child].auxiliary = auxiliary;
        let body = items.get(2..).unwrap_or(&[]);
        self.compile_sequence(child, body)?;
        self.emit(child, Instruction::Return, span)?;
        self.emit(function, Instruction::MakeClosure(child), span)?;
        Ok(())
    }

    fn compile_function(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.require_arity(items, "FUNCTION", "one", 1, span)?;
        let Some(argument) = items.get(1) else {
            return Err(self.internal_error(span, "missing function argument after arity check"));
        };
        if matches!(argument.kind, FormKind::Atom(_)) {
            let (name, escaped) = self.symbol_name_info(argument, "function name")?;
            let local_function = self.is_local_function(&Self::local_function_key(&name, escaped));
            self.emit(
                function,
                if local_function && escaped {
                    Instruction::FunctionLoadExact(name)
                } else if local_function {
                    Instruction::FunctionLoad(name)
                } else if escaped {
                    Instruction::FunctionLoadExact(name)
                } else {
                    Instruction::FunctionLoad(name)
                },
                argument.span,
            )?;
        } else {
            self.compile_expression(function, argument)?;
        }
        Ok(())
    }

    fn compile_funcall(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "FUNCALL", "at least one", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::Call(items.len().saturating_sub(2)),
            span,
        )?;
        Ok(())
    }

    fn compile_eval(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.require_arity(items, "EVAL", "one", 1, span)?;
        let Some(argument) = items.get(1) else {
            return Err(self.internal_error(span, "missing eval argument after arity check"));
        };
        self.compile_expression(function, argument)?;
        self.emit(function, Instruction::Eval(argument.span), span)?;
        Ok(())
    }

    fn compile_apply(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(self.arity_error(items, "APPLY", "at least two", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::Apply(items.len().saturating_sub(2)),
            span,
        )?;
        Ok(())
    }

    fn compile_mapcar(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(self.arity_error(items, "MAPCAR", "at least two", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::MapCar(items.len().saturating_sub(2)),
            span,
        )?;
        Ok(())
    }

    fn compile_map_into(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(self.arity_error(items, "MAP-INTO", "at least two", span));
        }
        let destination = items[1].clone();
        self.emit(
            function,
            Instruction::FunctionLoad("MAP-INTO".to_string()),
            items[0].span,
        )?;
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::Call(items.len().saturating_sub(1)),
            span,
        )?;
        self.emit(
            function,
            Instruction::MapIntoSetf(destination.clone()),
            destination.span,
        )?;
        Ok(())
    }

    fn compile_destructuring_pattern(
        &self,
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<DestructurePattern, CompileError> {
        match &form.kind {
            FormKind::Atom(_) => Ok(DestructurePattern::Name(
                self.compile_destructuring_binding_name(form, seen, "destructuring pattern name")?,
            )),
            FormKind::List(items) => Ok(DestructurePattern::List(
                items
                    .iter()
                    .map(|item| self.compile_destructuring_pattern(item, seen))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            FormKind::DottedList { items, tail } => Ok(DestructurePattern::Dotted {
                items: items
                    .iter()
                    .map(|item| self.compile_destructuring_pattern(item, seen))
                    .collect::<Result<Vec<_>, _>>()?,
                tail: Box::new(self.compile_destructuring_pattern(tail, seen)?),
            }),
            _ => Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "destructuring pattern must be a symbol or list".to_string(),
                },
                form.span,
            )),
        }
    }

    fn compile_destructuring_default(&mut self, form: &Form) -> Result<FunctionId, CompileError> {
        let default_function = self.reserve_function(None, Vec::new());
        self.compile_expression(default_function, form)?;
        self.emit(default_function, Instruction::Return, form.span)?;
        Ok(default_function)
    }

    fn compile_destructuring_optional_parameter(
        &mut self,
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<DestructureOptionalParameter, CompileError> {
        let nil = || Form::atom("NIL", form.span);
        let (pattern, init_form, supplied_p) = match &form.kind {
            FormKind::Atom(_) => (self.compile_destructuring_pattern(form, seen)?, nil(), None),
            FormKind::List(items) if (1..=3).contains(&items.len()) => {
                let pattern = self.compile_destructuring_pattern(&items[0], seen)?;
                let init_form = items.get(1).cloned().unwrap_or_else(nil);
                let supplied_p = items
                    .get(2)
                    .map(|item| {
                        self.compile_destructuring_binding_name(
                            item,
                            seen,
                            "destructuring supplied-p name",
                        )
                    })
                    .transpose()?;
                (pattern, init_form, supplied_p)
            }
            FormKind::List(_) => {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "destructuring optional parameter must contain one to three items"
                            .to_string(),
                    },
                    form.span,
                ));
            }
            _ => {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "destructuring optional parameter must be a symbol or list"
                            .to_string(),
                    },
                    form.span,
                ));
            }
        };
        let default_function = self.compile_destructuring_default(&init_form)?;
        Ok(DestructureOptionalParameter {
            pattern,
            default_function,
            supplied_p,
        })
    }

    fn compile_destructuring_keyword_parameter(
        &mut self,
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<DestructureKeywordParameter, CompileError> {
        let nil = || Form::atom("NIL", form.span);
        let (keyword_name, pattern, trailing_start) = match &form.kind {
            FormKind::Atom(_) => {
                let name = self.compile_destructuring_binding_name(
                    form,
                    seen,
                    "destructuring keyword parameter name",
                )?;
                let keyword_name = normalize_name(&name);
                (keyword_name, DestructurePattern::Name(name), 0)
            }
            FormKind::List(items) if !items.is_empty() => {
                if let FormKind::List(key_specification) = &items[0].kind {
                    if key_specification.len() != 2 {
                        return Err(CompileError::new(
                            CompileErrorKind::InvalidForm {
                                message: "destructuring keyword designator must contain a keyword and variable"
                                    .to_string(),
                            },
                            items[0].span,
                        ));
                    }
                    let keyword_name =
                        self.compile_destructuring_keyword_name(&key_specification[0])?;
                    let pattern =
                        self.compile_destructuring_pattern(&key_specification[1], seen)?;
                    (keyword_name, pattern, 1)
                } else if matches!(&items[0].kind, FormKind::Atom(name) if name.starts_with(':')) {
                    let keyword_name = self.compile_destructuring_keyword_name(&items[0])?;
                    if items.len() < 2 {
                        return Err(CompileError::new(
                            CompileErrorKind::InvalidForm {
                                message: "destructuring keyword parameter needs a variable"
                                    .to_string(),
                            },
                            form.span,
                        ));
                    }
                    let pattern = self.compile_destructuring_pattern(&items[1], seen)?;
                    (keyword_name, pattern, 2)
                } else {
                    let pattern = self.compile_destructuring_pattern(&items[0], seen)?;
                    let DestructurePattern::Name(name) = &pattern else {
                        return Err(CompileError::new(
                            CompileErrorKind::InvalidForm {
                                message:
                                    "destructuring keyword parameter must have a variable name"
                                        .to_string(),
                            },
                            items[0].span,
                        ));
                    };
                    (normalize_name(name), pattern, 1)
                }
            }
            FormKind::List(_) => unreachable!(),
            _ => {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "destructuring keyword parameter must be a symbol or list"
                            .to_string(),
                    },
                    form.span,
                ));
            }
        };

        let item_count = match &form.kind {
            FormKind::Atom(_) => 0,
            FormKind::List(items) => items.len(),
            _ => unreachable!(),
        };
        if item_count > trailing_start + 2 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "destructuring keyword parameter contains too many items".to_string(),
                },
                form.span,
            ));
        }
        let (init_form, supplied_p) = match &form.kind {
            FormKind::Atom(_) => (nil(), None),
            FormKind::List(items) => (
                items.get(trailing_start).cloned().unwrap_or_else(nil),
                items
                    .get(trailing_start + 1)
                    .map(|item| {
                        self.compile_destructuring_binding_name(
                            item,
                            seen,
                            "destructuring supplied-p name",
                        )
                    })
                    .transpose()?,
            ),
            _ => unreachable!(),
        };
        let default_function = self.compile_destructuring_default(&init_form)?;
        Ok(DestructureKeywordParameter {
            keyword_name,
            pattern,
            default_function,
            supplied_p,
        })
    }

    fn compile_destructuring_auxiliary_parameter(
        &mut self,
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<DestructureAuxiliaryParameter, CompileError> {
        let nil = || Form::atom("NIL", form.span);
        let (name, init_form) = match &form.kind {
            FormKind::Atom(_) => (
                self.compile_destructuring_binding_name(
                    form,
                    seen,
                    "destructuring auxiliary parameter name",
                )?,
                nil(),
            ),
            FormKind::List(items) if (1..=2).contains(&items.len()) => (
                self.compile_destructuring_binding_name(
                    &items[0],
                    seen,
                    "destructuring auxiliary parameter name",
                )?,
                items.get(1).cloned().unwrap_or_else(nil),
            ),
            FormKind::List(_) => {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "destructuring auxiliary parameter must contain one or two items"
                            .to_string(),
                    },
                    form.span,
                ));
            }
            _ => {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "destructuring auxiliary parameter must be a symbol or list"
                            .to_string(),
                    },
                    form.span,
                ));
            }
        };
        let default_function = self.compile_destructuring_default(&init_form)?;
        Ok(DestructureAuxiliaryParameter {
            name,
            default_function,
        })
    }

    fn compile_destructuring_lambda_list(
        &mut self,
        form: &Form,
    ) -> Result<DestructureLambdaList, CompileError> {
        let FormKind::List(parameters) = &form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "destructuring lambda list".to_string(),
                },
                form.span,
            ));
        };
        let mut lambda_list = DestructureLambdaList {
            whole: None,
            required: Vec::new(),
            optional: Vec::new(),
            keywords: Vec::new(),
            has_keyword_section: false,
            allow_other_keys: false,
            rest: None,
            auxiliary: Vec::new(),
        };
        let mut seen = HashSet::new();
        let mut section = DestructureLambdaListSection::Required;
        let mut index = 0;
        while index < parameters.len() {
            let parameter = &parameters[index];
            if let FormKind::Atom(name) = &parameter.kind {
                let marker = normalize_name(name);
                match marker.as_str() {
                    "&WHOLE" => {
                        if index != 0
                            || lambda_list.whole.is_some()
                            || index + 1 >= parameters.len()
                        {
                            return Err(CompileError::new(
                                CompileErrorKind::InvalidForm {
                                    message:
                                        "&whole must be the first marker followed by one parameter"
                                            .to_string(),
                                },
                                parameter.span,
                            ));
                        }
                        lambda_list.whole = Some(self.compile_destructuring_binding_name(
                            &parameters[index + 1],
                            &mut seen,
                            "destructuring whole parameter name",
                        )?);
                        index += 2;
                    }
                    "&OPTIONAL" => {
                        if section != DestructureLambdaListSection::Required {
                            return Err(CompileError::new(
                                CompileErrorKind::InvalidForm {
                                    message:
                                        "&optional is out of order in destructuring lambda list"
                                            .to_string(),
                                },
                                parameter.span,
                            ));
                        }
                        section = DestructureLambdaListSection::Optional;
                        index += 1;
                    }
                    "&REST" | "&BODY" => {
                        if lambda_list.rest.is_some()
                            || matches!(
                                section,
                                DestructureLambdaListSection::Rest
                                    | DestructureLambdaListSection::Keyword
                                    | DestructureLambdaListSection::Auxiliary
                            )
                            || index + 1 >= parameters.len()
                        {
                            return Err(CompileError::new(
                                CompileErrorKind::InvalidForm {
                                    message: "&rest or &body must be followed by one parameter"
                                        .to_string(),
                                },
                                parameter.span,
                            ));
                        }
                        lambda_list.rest = Some(self.compile_destructuring_binding_name(
                            &parameters[index + 1],
                            &mut seen,
                            "destructuring rest parameter name",
                        )?);
                        section = DestructureLambdaListSection::Rest;
                        index += 2;
                    }
                    "&KEY" => {
                        if lambda_list.has_keyword_section
                            || matches!(
                                section,
                                DestructureLambdaListSection::Keyword
                                    | DestructureLambdaListSection::Auxiliary
                            )
                        {
                            return Err(CompileError::new(
                                CompileErrorKind::InvalidForm {
                                    message: "&key is out of order or repeated in destructuring lambda list"
                                        .to_string(),
                                },
                                parameter.span,
                            ));
                        }
                        lambda_list.has_keyword_section = true;
                        section = DestructureLambdaListSection::Keyword;
                        index += 1;
                    }
                    "&ALLOW-OTHER-KEYS" => {
                        if section != DestructureLambdaListSection::Keyword
                            || lambda_list.allow_other_keys
                        {
                            return Err(CompileError::new(
                                CompileErrorKind::InvalidForm {
                                    message: "&allow-other-keys requires a keyword section"
                                        .to_string(),
                                },
                                parameter.span,
                            ));
                        }
                        lambda_list.allow_other_keys = true;
                        index += 1;
                    }
                    "&AUX" => {
                        if section == DestructureLambdaListSection::Auxiliary {
                            return Err(CompileError::new(
                                CompileErrorKind::InvalidForm {
                                    message: "&aux is repeated in destructuring lambda list"
                                        .to_string(),
                                },
                                parameter.span,
                            ));
                        }
                        section = DestructureLambdaListSection::Auxiliary;
                        index += 1;
                    }
                    "&ENVIRONMENT" => {
                        return Err(CompileError::new(
                            CompileErrorKind::InvalidForm {
                                message: "&environment is not supported in destructuring-bind"
                                    .to_string(),
                            },
                            parameter.span,
                        ));
                    }
                    _ if marker.starts_with('&') => {
                        return Err(CompileError::new(
                            CompileErrorKind::InvalidForm {
                                message: "unsupported marker in destructuring lambda list"
                                    .to_string(),
                            },
                            parameter.span,
                        ));
                    }
                    _ => {
                        if section == DestructureLambdaListSection::Rest {
                            return Err(CompileError::new(
                                CompileErrorKind::InvalidForm {
                                    message: "destructuring rest parameter must be followed by a keyword or auxiliary section"
                                        .to_string(),
                                },
                                parameter.span,
                            ));
                        }
                        match section {
                            DestructureLambdaListSection::Required => lambda_list
                                .required
                                .push(self.compile_destructuring_pattern(parameter, &mut seen)?),
                            DestructureLambdaListSection::Optional => lambda_list.optional.push(
                                self.compile_destructuring_optional_parameter(
                                    parameter, &mut seen,
                                )?,
                            ),
                            DestructureLambdaListSection::Keyword => {
                                if lambda_list.allow_other_keys {
                                    return Err(CompileError::new(
                                        CompileErrorKind::InvalidForm {
                                            message: "&allow-other-keys must be the last keyword-list marker"
                                                .to_string(),
                                        },
                                        parameter.span,
                                    ));
                                }
                                let specification = self.compile_destructuring_keyword_parameter(
                                    parameter, &mut seen,
                                )?;
                                if lambda_list
                                    .keywords
                                    .iter()
                                    .any(|item| item.keyword_name == specification.keyword_name)
                                {
                                    return Err(CompileError::new(
                                        CompileErrorKind::InvalidForm {
                                            message: "destructuring keyword names must be unique"
                                                .to_string(),
                                        },
                                        parameter.span,
                                    ));
                                }
                                lambda_list.keywords.push(specification);
                            }
                            DestructureLambdaListSection::Auxiliary => lambda_list.auxiliary.push(
                                self.compile_destructuring_auxiliary_parameter(
                                    parameter, &mut seen,
                                )?,
                            ),
                            DestructureLambdaListSection::Rest => unreachable!(),
                        }
                        index += 1;
                    }
                }
                continue;
            }

            if section == DestructureLambdaListSection::Rest {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "destructuring rest parameter must be followed by a keyword or auxiliary section"
                            .to_string(),
                    },
                    parameter.span,
                ));
            }
            match section {
                DestructureLambdaListSection::Required => lambda_list
                    .required
                    .push(self.compile_destructuring_pattern(parameter, &mut seen)?),
                DestructureLambdaListSection::Optional => lambda_list
                    .optional
                    .push(self.compile_destructuring_optional_parameter(parameter, &mut seen)?),
                DestructureLambdaListSection::Keyword => {
                    if lambda_list.allow_other_keys {
                        return Err(CompileError::new(
                            CompileErrorKind::InvalidForm {
                                message: "&allow-other-keys must be the last keyword-list marker"
                                    .to_string(),
                            },
                            parameter.span,
                        ));
                    }
                    let specification =
                        self.compile_destructuring_keyword_parameter(parameter, &mut seen)?;
                    if lambda_list
                        .keywords
                        .iter()
                        .any(|item| item.keyword_name == specification.keyword_name)
                    {
                        return Err(CompileError::new(
                            CompileErrorKind::InvalidForm {
                                message: "destructuring keyword names must be unique".to_string(),
                            },
                            parameter.span,
                        ));
                    }
                    lambda_list.keywords.push(specification);
                }
                DestructureLambdaListSection::Auxiliary => lambda_list
                    .auxiliary
                    .push(self.compile_destructuring_auxiliary_parameter(parameter, &mut seen)?),
                DestructureLambdaListSection::Rest => unreachable!(),
            }
            index += 1;
        }

        Ok(lambda_list)
    }

    fn compile_destructuring_bind(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(self.arity_error(items, "DESTRUCTURING-BIND", "two or more", span));
        }
        let mut seen = HashSet::new();
        let specification = match &items[1].kind {
            FormKind::List(_) => {
                DestructureSpec::LambdaList(self.compile_destructuring_lambda_list(&items[1])?)
            }
            _ => {
                DestructureSpec::Pattern(self.compile_destructuring_pattern(&items[1], &mut seen)?)
            }
        };
        self.emit(function, Instruction::EnterScope, items[1].span)?;
        self.compile_expression(function, &items[2])?;
        self.emit(
            function,
            Instruction::Destructure(specification),
            items[1].span,
        )?;
        self.compile_sequence(function, items.get(3..).unwrap_or(&[]))?;
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }

    fn compile_let(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        sequential: bool,
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(CompileError::new(
                CompileErrorKind::Arity {
                    operator: if sequential { "LET*" } else { "LET" }.to_string(),
                    expected: "at least one".to_string(),
                    actual: items.len().saturating_sub(1),
                },
                operator_span(items, span),
            ));
        }
        let Some(binding_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing let bindings after arity check"));
        };
        let FormKind::List(bindings) = &binding_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "let bindings".to_string(),
                },
                binding_form.span,
            ));
        };

        let mut parsed = Vec::with_capacity(bindings.len());
        let mut names = HashSet::new();
        for binding in bindings {
            let FormKind::List(binding_items) = &binding.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "let binding".to_string(),
                    },
                    binding.span,
                ));
            };
            if !(binding_items.len() == 1 || binding_items.len() == 2) {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "let binding needs a name and optional value".to_string(),
                    },
                    binding.span,
                ));
            }
            let Some(name_form) = binding_items.first() else {
                return Err(self.internal_error(binding.span, "missing let binding name"));
            };
            let name = self.symbol_name(name_form, "let binding name")?;
            if !sequential && !names.insert(name.clone()) {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "let bindings must have distinct names".to_string(),
                    },
                    name_form.span,
                ));
            }
            parsed.push((name, binding_items.get(1)));
        }

        self.emit(function, Instruction::EnterScope, binding_form.span)?;
        if sequential {
            for (name, value) in &parsed {
                if let Some(value) = value {
                    self.compile_expression(function, value)?;
                } else {
                    self.emit(
                        function,
                        Instruction::Constant(Constant::Nil),
                        binding_form.span,
                    )?;
                }
                self.emit(
                    function,
                    Instruction::Define(name.clone()),
                    binding_form.span,
                )?;
                self.emit(function, Instruction::Pop, binding_form.span)?;
            }
        } else {
            for (_, value) in &parsed {
                if let Some(value) = value {
                    self.compile_expression(function, value)?;
                } else {
                    self.emit(
                        function,
                        Instruction::Constant(Constant::Nil),
                        binding_form.span,
                    )?;
                }
            }
            for (name, _) in parsed.iter().rev() {
                self.emit(
                    function,
                    Instruction::Define(name.clone()),
                    binding_form.span,
                )?;
                self.emit(function, Instruction::Pop, binding_form.span)?;
            }
        }

        let body = items.get(2..).unwrap_or(&[]);
        self.compile_sequence(function, body)?;
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }

    fn compile_flet(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        recursive: bool,
    ) -> Result<(), CompileError> {
        let operator = if recursive { "LABELS" } else { "FLET" };
        if items.len() < 2 {
            return Err(CompileError::new(
                CompileErrorKind::Arity {
                    operator: operator.to_string(),
                    expected: "at least one".to_string(),
                    actual: items.len().saturating_sub(1),
                },
                operator_span(items, span),
            ));
        }
        let Some(binding_form) = items.get(1) else {
            return Err(
                self.internal_error(span, "missing local function bindings after arity check")
            );
        };
        let FormKind::List(bindings) = &binding_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "local function bindings".to_string(),
                },
                binding_form.span,
            ));
        };

        let mut parsed = Vec::with_capacity(bindings.len());
        let mut names = HashSet::new();
        for binding in bindings {
            let FormKind::List(parts) = &binding.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "local function binding".to_string(),
                    },
                    binding.span,
                ));
            };
            if parts.len() < 3 {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "local function needs a name, parameters, and a body".to_string(),
                    },
                    binding.span,
                ));
            }
            let Some(name_form) = parts.first() else {
                return Err(self.internal_error(
                    binding.span,
                    "missing local function name after arity check",
                ));
            };
            let (name, name_escaped) = self.symbol_name_info(name_form, "local function name")?;
            let local_key = Self::local_function_key(&name, name_escaped);
            if !names.insert(local_key) {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "local function names must be unique".to_string(),
                    },
                    name_form.span,
                ));
            }
            let Some(parameter_form) = parts.get(1) else {
                return Err(self.internal_error(
                    binding.span,
                    "missing local function parameters after arity check",
                ));
            };
            let lambda_list = self.parameters(parameter_form)?;
            parsed.push((name, name_escaped, lambda_list, parts[2..].to_vec()));
        }

        if recursive {
            self.emit(function, Instruction::EnterScope, binding_form.span)?;
            self.local_function_scopes.push(names.clone());
        }
        for (name, _name_escaped, lambda_list, body) in &parsed {
            let child = self.reserve_function_with_rest(
                Some(name.clone()),
                lambda_list.required.clone(),
                lambda_list.required_escaped.clone(),
                lambda_list.rest.clone(),
                lambda_list.rest_escaped,
            );
            let optional = self.compile_optional_parameters(&lambda_list.optional)?;
            self.functions[child].optional = optional;
            let keywords = self.compile_keyword_parameters(&lambda_list.keywords)?;
            self.functions[child].keywords = keywords;
            self.functions[child].has_keyword_section = lambda_list.has_keyword_section;
            self.functions[child].allow_other_keys = lambda_list.allow_other_keys;
            let auxiliary = self.compile_auxiliary_parameters(&lambda_list.auxiliary)?;
            self.functions[child].auxiliary = auxiliary;
            self.compile_sequence(child, body)?;
            self.emit(child, Instruction::Return, span)?;
            self.emit(function, Instruction::MakeClosure(child), span)?;
        }
        if !recursive {
            self.emit(function, Instruction::EnterScope, binding_form.span)?;
        }
        for (name, name_escaped, _, _) in parsed.iter().rev() {
            let instruction = if *name_escaped {
                Instruction::DefineFunctionExact(name.clone())
            } else {
                Instruction::DefineFunction(name.clone())
            };
            self.emit(function, instruction, binding_form.span)?;
        }

        let body = items.get(2..).unwrap_or(&[]);
        if !recursive {
            self.local_function_scopes.push(names);
        }
        self.compile_sequence(function, body)?;
        self.local_function_scopes.pop();
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }

    fn parameters(&self, form: &Form) -> Result<OrdinaryLambdaList, CompileError> {
        parse_ordinary_lambda_list(form).map_err(|error| {
            let span = error.span;
            let kind = match error.kind {
                LambdaListErrorKind::ExpectedList => CompileErrorKind::ExpectedList {
                    context: "parameters".to_string(),
                },
                LambdaListErrorKind::ExpectedSymbol { context } => {
                    CompileErrorKind::ExpectedSymbol {
                        context: context.to_string(),
                    }
                }
                LambdaListErrorKind::InvalidForm { message } => {
                    CompileErrorKind::InvalidForm { message }
                }
            };
            CompileError::new(kind, span)
        })
    }

    fn compile_optional_parameters(
        &mut self,
        specifications: &[LambdaListOptionalParameter],
    ) -> Result<Vec<OptionalParameter>, CompileError> {
        let mut optional = Vec::with_capacity(specifications.len());
        for specification in specifications {
            let default_function = self.reserve_function(None, Vec::new());
            self.compile_expression(default_function, &specification.init_form)?;
            self.emit(
                default_function,
                Instruction::Return,
                specification.init_form.span,
            )?;
            optional.push(OptionalParameter {
                name: specification.name.clone(),
                name_escaped: specification.name_escaped,
                default_function,
                supplied_p: specification.supplied_p.clone(),
                supplied_p_escaped: specification.supplied_p_escaped,
            });
        }
        Ok(optional)
    }

    fn compile_auxiliary_parameters(
        &mut self,
        specifications: &[LambdaListAuxiliaryParameter],
    ) -> Result<Vec<AuxiliaryParameter>, CompileError> {
        let mut auxiliary = Vec::with_capacity(specifications.len());
        for specification in specifications {
            let default_function = self.reserve_function(None, Vec::new());
            self.compile_expression(default_function, &specification.init_form)?;
            self.emit(
                default_function,
                Instruction::Return,
                specification.init_form.span,
            )?;
            auxiliary.push(AuxiliaryParameter {
                name: specification.name.clone(),
                name_escaped: specification.name_escaped,
                default_function,
            });
        }
        Ok(auxiliary)
    }

    fn compile_keyword_parameters(
        &mut self,
        specifications: &[LambdaListKeywordParameter],
    ) -> Result<Vec<KeywordParameter>, CompileError> {
        let mut keywords = Vec::with_capacity(specifications.len());
        for specification in specifications {
            let default_function = self.reserve_function(None, Vec::new());
            self.compile_expression(default_function, &specification.init_form)?;
            self.emit(
                default_function,
                Instruction::Return,
                specification.init_form.span,
            )?;
            keywords.push(KeywordParameter {
                keyword_name: specification.keyword_name.clone(),
                keyword_name_escaped: specification.keyword_name_escaped,
                name: specification.name.clone(),
                name_escaped: specification.name_escaped,
                default_function,
                supplied_p: specification.supplied_p.clone(),
                supplied_p_escaped: specification.supplied_p_escaped,
            });
        }
        Ok(keywords)
    }
}
