use super::*;

impl CompileState {
    pub(super) fn is_local_function(&self, name: &str) -> bool {
        self.local_function_scopes
            .iter()
            .rev()
            .any(|scope| scope.contains(name))
    }

    pub(super) fn reserve_function(
        &mut self,
        name: Option<String>,
        parameters: Vec<String>,
    ) -> FunctionId {
        let required_escaped = vec![false; parameters.len()];
        self.reserve_function_with_rest(name, parameters, required_escaped, None, false)
    }

    pub(super) fn reserve_function_with_rest(
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

    pub(super) fn local_function_key(name: &str, escaped: bool) -> String {
        if escaped {
            format!("\0{name}")
        } else {
            normalize_name(name)
        }
    }

    pub(super) fn emit(
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

    pub(super) fn collect_names(&mut self, forms: &[Form]) {
        for form in forms {
            self.collect_form_names(form);
        }
    }

    pub(super) fn collect_form_names(&mut self, form: &Form) {
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
}
