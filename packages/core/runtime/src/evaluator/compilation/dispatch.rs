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
            "QUASIQUOTE" => {
                if prepared.len() == 2 {
                    prepared[1] = self.prepare_compiled_quasiquote(&prepared[1], environment, 1)?;
                }
            }
            "SETF"
            | "%SETF-INTRINSIC-STORE"
            | "PSETF"
            | "SHIFTF"
            | "ROTATEF"
            | "PUSH"
            | "POP"
            | "PUSHNEW" => {
                self.prepare_compiled_place_mutation(&mut prepared, environment)?;
            }
            "THE" | "WITH-SIMPLE-RESTART" | "BLOCK" | "DEFINE" | "DEFVAR" | "DEFPARAMETER" => {
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            "INCF" | "DECF" => {
                return self.prepare_compiled_arithmetic_place(items, environment, operator);
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
            "DEFMETHOD" => self.prepare_defmethod(&mut prepared, environment)?,
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
            "LOCALLY" => {
                return self.prepare_compiled_locally(form, &prepared, environment);
            }
            "FLET" | "LABELS" => {
                if prepared.len() > 1 {
                    prepared[1] =
                        self.prepare_local_function_bindings(&prepared[1], environment)?;
                }
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            "DOTIMES" | "DOLIST" => {
                self.prepare_compiled_iteration(&mut prepared, environment)?;
            }
            "DO" | "DO*" => {
                self.prepare_compiled_do(&mut prepared, environment)?;
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

    fn prepare_compiled_iteration(
        &self,
        items: &mut [Form],
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        if items.len() > 1 {
            items[1] = self.prepare_iteration_binding(&items[1], environment)?;
        }
        self.prepare_tail(items, 2, environment)
    }

    fn prepare_compiled_do(
        &self,
        items: &mut [Form],
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        if items.len() > 1 {
            items[1] = self.prepare_do_bindings(&items[1], environment)?;
        }
        if items.len() > 2 {
            items[2] = self.prepare_do_termination(&items[2], environment)?;
        }
        self.prepare_tail(items, 3, environment)
    }

    fn prepare_compiled_arithmetic_place(
        &self,
        items: &[Form],
        environment: &Environment,
        operator: &str,
    ) -> Result<Form, RuntimeError> {
        let arithmetic = if normalize_name(operator) == "INCF" {
            "+"
        } else {
            "-"
        };
        let expanded = self.expand_arithmetic_place(items, environment, operator, arithmetic)?;
        self.prepare_compiled_form(&expanded, environment)
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
                | "DEFSETF"
                | "DEFINE-MODIFY-MACRO"
                | "DEFCONSTANT"
                | "QUOTE"
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
