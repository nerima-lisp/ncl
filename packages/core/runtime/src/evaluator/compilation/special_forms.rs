#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(super) fn prepare_restart_case(
        &self,
        prepared: &mut [Form],
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        if prepared.len() > 1 {
            prepared[1] = self.prepare_compiled_form(&prepared[1], environment)?;
        }
        for clause in prepared.iter_mut().skip(2) {
            *clause = self.prepare_restart_case_clause(clause, environment)?;
        }
        Ok(())
    }

    pub(super) fn prepare_catch(
        &self,
        prepared: &mut [Form],
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        if prepared.len() > 1 {
            prepared[1] = self.prepare_compiled_form(&prepared[1], environment)?;
        }
        self.prepare_tail(prepared, 2, environment)
    }

    pub(super) fn prepare_progv(
        &self,
        prepared: &mut [Form],
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        for index in 1..=2 {
            if prepared.len() > index {
                prepared[index] = self.prepare_compiled_form(&prepared[index], environment)?;
            }
        }
        self.prepare_tail(prepared, 3, environment)
    }

    pub(super) fn prepare_prog(
        &self,
        prepared: &mut [Form],
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        if prepared.len() > 1 {
            prepared[1] = self.prepare_prog_bindings(&prepared[1], environment)?;
        }
        self.prepare_tail(prepared, 2, environment)
    }

    pub(super) fn prepare_value_bind(
        &self,
        prepared: &mut [Form],
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        if prepared.len() > 2 {
            prepared[2] = self.prepare_compiled_form(&prepared[2], environment)?;
        }
        self.prepare_tail(prepared, 3, environment)
    }

    pub(super) fn prepare_return(
        &self,
        prepared: &mut [Form],
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        if prepared.len() > 1 {
            prepared[1] = self.prepare_compiled_form(&prepared[1], environment)?;
        }
        Ok(())
    }

    pub(super) fn prepare_return_from(
        &self,
        prepared: &mut [Form],
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        if prepared.len() > 2 {
            prepared[2] = self.prepare_compiled_form(&prepared[2], environment)?;
        }
        Ok(())
    }

    pub(super) fn prepare_cond(
        &self,
        prepared: &mut [Form],
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        for clause in prepared.iter_mut().skip(1) {
            *clause = self.prepare_cond_clause(clause, environment)?;
        }
        Ok(())
    }

    pub(super) fn prepare_case(
        &self,
        prepared: &mut [Form],
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        if prepared.len() > 1 {
            prepared[1] = self.prepare_compiled_form(&prepared[1], environment)?;
        }
        for clause in prepared.iter_mut().skip(2) {
            *clause = self.prepare_case_clause(clause, environment)?;
        }
        Ok(())
    }

    pub(super) fn prepare_handler_case(
        &self,
        prepared: &mut [Form],
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        if prepared.len() > 1 {
            prepared[1] = self.prepare_compiled_form(&prepared[1], environment)?;
        }
        for clause in prepared.iter_mut().skip(2) {
            *clause = self.prepare_handler_case_clause(clause, environment)?;
        }
        Ok(())
    }

    pub(super) fn prepare_handler_bind(
        &self,
        prepared: &mut [Form],
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        if prepared.len() > 1 {
            let FormKind::List(handlers) = &prepared[1].kind else {
                return Ok(());
            };
            let mut prepared_handlers = Vec::with_capacity(handlers.len());
            for handler in handlers {
                let FormKind::List(parts) = &handler.kind else {
                    prepared_handlers.push(handler.clone());
                    continue;
                };
                let mut prepared_parts = parts.clone();
                if prepared_parts.len() > 1 {
                    prepared_parts[1] = self.prepare_compiled_form(&parts[1], environment)?;
                }
                prepared_handlers.push(Form::list(prepared_parts, handler.span));
            }
            prepared[1] = Form::list(prepared_handlers, prepared[1].span);
        }
        self.prepare_tail(prepared, 2, environment)
    }
}
