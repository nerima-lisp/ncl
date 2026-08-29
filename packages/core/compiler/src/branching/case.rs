use crate::{
    CompileError, CompileErrorKind, CompileState, Constant, Form, FormKind, FunctionId,
    Instruction, Span, case_default_clause, normalize_name,
};

impl CompileState {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn compile_case(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        let operator = items
            .first()
            .and_then(|form| match &form.kind {
                FormKind::Atom(atom) => Some(normalize_name(atom)),
                _ => None,
            })
            .unwrap_or_else(|| "CASE".to_string());
        if items.len() < 2 {
            return Err(Self::arity_error(items, &operator, "at least one", span));
        }

        let mut clauses = Vec::new();
        let mut default_clause: Option<(Vec<Form>, Span)> = None;
        for clause in items.iter().skip(2) {
            let FormKind::List(clause_items) = &clause.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "case clause".to_string(),
                    },
                    clause.span,
                ));
            };
            let Some(key_spec) = clause_items.first() else {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "case clause cannot be empty".to_string(),
                    },
                    clause.span,
                ));
            };
            if case_default_clause(key_spec) {
                default_clause = Some((clause_items.get(1..).unwrap_or(&[]).to_vec(), clause.span));
                continue;
            }
            let keys = match &key_spec.kind {
                FormKind::List(keys) => keys.clone(),
                _ => vec![key_spec.clone()],
            };
            clauses.push((
                keys,
                clause_items.get(1..).unwrap_or(&[]).to_vec(),
                clause.span,
            ));
        }

        let key_name = self.fresh_name("CASE_KEY");
        self.emit(function, Instruction::EnterScope, span)?;
        self.compile_expression(function, &items[1])?;
        self.emit(
            function,
            Instruction::Define(key_name.clone()),
            items[1].span,
        )?;
        self.emit(function, Instruction::Pop, items[1].span)?;

        let mut body_jumps: Vec<(Vec<usize>, Vec<Form>, Span)> = Vec::new();
        for (keys, body, clause_span) in clauses {
            let mut clause_jumps = Vec::new();
            for key in keys {
                self.emit(
                    function,
                    Instruction::FunctionLoad("EQL".to_string()),
                    key.span,
                )?;
                self.emit(function, Instruction::Load(key_name.clone()), items[1].span)?;
                self.emit(function, Instruction::Quote(key.clone()), key.span)?;
                self.emit(function, Instruction::Call(2), key.span)?;
                let false_jump =
                    self.emit(function, Instruction::JumpIfFalse(usize::MAX), key.span)?;
                let body_jump = self.emit(function, Instruction::Jump(usize::MAX), key.span)?;
                let next_key = self.instruction_count(function, key.span)?;
                self.patch_jump(function, false_jump, next_key, key.span)?;
                clause_jumps.push(body_jump);
            }
            body_jumps.push((clause_jumps, body, clause_span));
        }

        let default_jump = if default_clause.is_some() {
            Some(self.emit(function, Instruction::Jump(usize::MAX), span)?)
        } else {
            None
        };
        let no_match_jump = if default_clause.is_none() {
            if operator.eq_ignore_ascii_case("ECASE") {
                self.emit(
                    function,
                    Instruction::FunctionLoad("__NCL_ECASE_ERROR".to_string()),
                    span,
                )?;
                self.emit(function, Instruction::Call(0), span)?;
            } else {
                self.emit(function, Instruction::Constant(Constant::Nil), span)?;
            }
            Some(self.emit(function, Instruction::Jump(usize::MAX), span)?)
        } else {
            None
        };

        let mut end_jumps = Vec::new();
        for (jumps, body, clause_span) in body_jumps {
            let target = self.instruction_count(function, clause_span)?;
            for jump in jumps {
                self.patch_jump(function, jump, target, clause_span)?;
            }
            self.compile_sequence(function, &body)?;
            end_jumps.push(self.emit(function, Instruction::Jump(usize::MAX), clause_span)?);
        }

        if let Some((body, clause_span)) = default_clause {
            let target = self.instruction_count(function, clause_span)?;
            if let Some(jump) = default_jump {
                self.patch_jump(function, jump, target, clause_span)?;
            }
            self.compile_sequence(function, &body)?;
            end_jumps.push(self.emit(function, Instruction::Jump(usize::MAX), clause_span)?);
        }

        let end = self.instruction_count(function, span)?;
        if let Some(jump) = no_match_jump {
            self.patch_jump(function, jump, end, span)?;
        }
        for jump in end_jumps {
            self.patch_jump(function, jump, end, span)?;
        }
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
