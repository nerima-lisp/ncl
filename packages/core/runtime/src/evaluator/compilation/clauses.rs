#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(super) fn prepare_cond_clause(
        &self,
        clause: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &clause.kind else {
            return Ok(clause.clone());
        };

        let mut prepared = items.clone();
        for item in &mut prepared {
            *item = self.prepare_compiled_form(item, environment)?;
        }
        Ok(Form::list(prepared, clause.span))
    }

    pub(super) fn prepare_case_clause(
        &self,
        clause: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &clause.kind else {
            return Ok(clause.clone());
        };

        let mut prepared = items.clone();
        self.prepare_tail(&mut prepared, 1, environment)?;
        Ok(Form::list(prepared, clause.span))
    }

    pub(super) fn prepare_handler_case_clause(
        &self,
        clause: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &clause.kind else {
            return Ok(clause.clone());
        };

        let mut prepared = items.clone();
        self.prepare_tail(&mut prepared, 2, environment)?;
        Ok(Form::list(prepared, clause.span))
    }

    pub(super) fn prepare_restart_case_clause(
        &self,
        clause: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &clause.kind else {
            return Ok(clause.clone());
        };

        let mut prepared = items.clone();
        if prepared.len() > 1 {
            prepared[1] = self.prepare_compiled_lambda_list(&items[1], environment)?;
        }
        self.prepare_tail(&mut prepared, 2, environment)?;
        Ok(Form::list(prepared, clause.span))
    }
}
