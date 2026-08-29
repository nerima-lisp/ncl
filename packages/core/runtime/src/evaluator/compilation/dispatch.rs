#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(super) fn prepare_compiled_list(
        &self,
        form: &Form,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let Some(operator) = items.first().and_then(atom_name) else {
            return self.prepare_compiled_list_without_operator(form, items, environment);
        };

        let mut prepared = items.to_vec();
        if Self::is_compiled_opaque_operator(normalize_name(operator).as_str()) {
            return Ok(form.clone());
        }
        match normalize_name(operator).as_str() {
            "THE"
            | "WITH-SIMPLE-RESTART"
            | "BLOCK"
            | "INCF"
            | "DECF"
            | "DEFINE"
            | "DEFVAR"
            | "DEFPARAMETER" => {
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            "EVAL-WHEN" => {
                if prepared.len() > 1 && Self::eval_when_executes(&prepared[1])? {
                    self.prepare_tail(&mut prepared, 2, environment)?;
                }
            }
            "RESTART-CASE" => self.prepare_restart_case(&mut prepared, environment)?,
            "CATCH" => self.prepare_catch(&mut prepared, environment)?,
            "PROGV" => self.prepare_progv(&mut prepared, environment)?,
            "PROG" | "PROG*" => self.prepare_prog(&mut prepared, environment)?,
            "DESTRUCTURING-BIND" | "MULTIPLE-VALUE-BIND" => {
                self.prepare_value_bind(&mut prepared, environment)?;
            }
            "RETURN" => self.prepare_return(&mut prepared, environment)?,
            "RETURN-FROM" => self.prepare_return_from(&mut prepared, environment)?,
            "MULTIPLE-VALUE-SETQ" => {
                return self.prepare_compiled_multiple_value_setq(form, &prepared, environment);
            }
            "LAMBDA" => self.prepare_lambda(&mut prepared, environment)?,
            "DEFUN" => self.prepare_defun(&mut prepared, environment)?,
            "FUNCTION" => {
                if prepared.len() == 2 && is_operator_form(&prepared[1], "LAMBDA") {
                    prepared[1] = self.prepare_compiled_form(&prepared[1], environment)?;
                }
            }
            "COND" => self.prepare_cond(&mut prepared, environment)?,
            "CASE" | "ECASE" | "TYPECASE" | "ETYPECASE" => {
                self.prepare_case(&mut prepared, environment)?;
            }
            "HANDLER-CASE" => self.prepare_handler_case(&mut prepared, environment)?,
            "HANDLER-BIND" | "RESTART-BIND" => {
                self.prepare_handler_bind(&mut prepared, environment)?;
            }
            "LET" | "LET*" => {
                if prepared.len() > 1 {
                    let current = Form::list(prepared.clone(), form.span);
                    return self.prepare_compiled_let(
                        &current,
                        &prepared,
                        environment,
                        normalize_name(operator) == "LET*",
                    );
                }
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            "FLET" | "LABELS" => {
                if prepared.len() > 1 {
                    prepared[1] =
                        self.prepare_local_function_bindings(&prepared[1], environment)?;
                }
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            "DOTIMES" | "DOLIST" => {
                if prepared.len() > 1 {
                    prepared[1] = self.prepare_iteration_binding(&prepared[1], environment)?;
                }
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            "DO" | "DO*" => {
                if prepared.len() > 1 {
                    prepared[1] = self.prepare_do_bindings(&prepared[1], environment)?;
                }
                if prepared.len() > 2 {
                    prepared[2] = self.prepare_do_termination(&prepared[2], environment)?;
                }
                self.prepare_tail(&mut prepared, 3, environment)?;
            }
            "PSETQ" => {
                return self.prepare_compiled_psetq(form, &prepared, environment);
            }
            "SETQ" => {
                return self.prepare_compiled_setq(form, &prepared, environment);
            }
            _ => {
                self.prepare_tail(&mut prepared, 1, environment)?;
            }
        }

        Ok(Form::list(prepared, form.span))
    }

    fn is_compiled_opaque_operator(operator: &str) -> bool {
        matches!(
            operator,
            "DECLARE"
                | "DECLAIM"
                | "PROCLAIM"
                | "DEFSTRUCT"
                | "DEFCLASS"
                | "DEFGENERIC"
                | "DEFMETHOD"
                | "DEFSETF"
                | "DEFINE-MODIFY-MACRO"
                | "DEFCONSTANT"
                | "QUOTE"
                | "QUASIQUOTE"
        )
    }

    fn prepare_compiled_list_without_operator(
        &self,
        form: &Form,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        if items.is_empty() {
            return Ok(form.clone());
        }
        let mut prepared = items.to_vec();
        prepared[0] = self.prepare_compiled_form(&items[0], environment)?;
        self.prepare_tail(&mut prepared, 1, environment)?;
        Ok(Form::list(prepared, form.span))
    }
}
