#![allow(clippy::wildcard_imports)]
use super::*;
use crate::environment::special_form_name;

impl Runtime {
    pub(super) fn eval_special_form_iteration(
        &self,
        items: &[Form],
        name: &str,
        environment: &Environment,
    ) -> Result<Option<Value>, RuntimeError> {
        let value = match special_form_name(name).unwrap_or(name) {
            "DOTIMES" => Some(self.special_dotimes(items, environment)?),
            "DOLIST" => Some(self.special_dolist(items, environment)?),
            "DO" => Some(self.special_do(items, environment, false)?),
            "DO*" => Some(self.special_do(items, environment, true)?),
            _ => None,
        };
        Ok(value)
    }

    pub(super) fn eval_special_form_bindings(
        &self,
        items: &[Form],
        name: &str,
        environment: &Environment,
    ) -> Result<Option<Value>, RuntimeError> {
        let value = match special_form_name(name).unwrap_or(name) {
            "DESTRUCTURING-BIND" => Some(self.special_destructuring_bind(items, environment)?),
            "LET" => Some(self.special_let(items, environment, false)?),
            "LET*" => Some(self.special_let(items, environment, true)?),
            "FLET" => Some(self.special_flet(items, environment, false)?),
            "LABELS" => Some(self.special_flet(items, environment, true)?),
            _ => None,
        };
        Ok(value)
    }

    pub(super) fn eval_special_form_conditionals(
        &self,
        items: &[Form],
        name: &str,
        environment: &Environment,
    ) -> Result<Option<Value>, RuntimeError> {
        let value = match special_form_name(name).unwrap_or(name) {
            "AND" => Some(self.special_and(&items[1..], environment)?),
            "OR" => Some(self.special_or(&items[1..], environment)?),
            "WHEN" => Some(self.special_when(items, environment, true)?),
            "UNLESS" => Some(self.special_when(items, environment, false)?),
            "COND" => Some(self.special_cond(&items[1..], environment)?),
            "CASE" => Some(self.special_case(items, environment, false)?),
            "ECASE" => Some(self.special_case(items, environment, true)?),
            "TYPECASE" => Some(self.special_typecase(items, environment, false)?),
            "ETYPECASE" => Some(self.special_typecase(items, environment, true)?),
            _ => None,
        };
        Ok(value)
    }

    pub(super) fn eval_special_form_core(
        &self,
        form: &Form,
        items: &[Form],
        name: &str,
        environment: &Environment,
    ) -> Result<Option<Value>, RuntimeError> {
        let value = match special_form_name(name).unwrap_or(name) {
            "QUOTE" => Some(Self::special_quote(items, form.span)?),
            "QUASIQUOTE" => Some(self.special_quasiquote(items, environment)?),
            "DECLARE" | "DECLAIM" | "PROCLAIM" => Some(Value::Nil),
            "LOCALLY" => Some(self.special_locally(items, environment)?),
            "EVAL-WHEN" => Some(self.special_eval_when(items, environment)?),
            "WITH-COMPILATION-UNIT" => {
                Some(self.special_with_compilation_unit(items, environment)?)
            }
            "THE" => Some(self.special_the(items, environment)?),
            "LOAD-TIME-VALUE" => Some(self.special_load_time_value(items, environment)?),
            "NTH-VALUE" => Some(self.special_nth_value(items, environment)?),
            "IF" => Some(self.special_if(items, environment)?),
            "PROGN" => Some(self.special_progn(&items[1..], environment)?),
            "PROG1" => Some(self.special_prog1(items, environment)?),
            "PROG2" => Some(self.special_prog2(items, environment)?),
            "PROG" => Some(self.special_prog(items, environment, false)?),
            "PROG*" => Some(self.special_prog(items, environment, true)?),
            "VALUES" => Some(self.special_values(items, environment)?),
            "IGNORE-ERRORS" => Some(self.special_ignore_errors(items, environment)?),
            "HANDLER-CASE" => Some(self.special_handler_case(items, environment)?),
            "HANDLER-BIND" => Some(self.special_handler_bind(items, environment)?),
            "RESTART-BIND" => Some(self.special_restart_bind(items, environment)?),
            "CATCH" => Some(self.special_catch(items, environment)?),
            "PROGV" => Some(self.special_progv(items, environment)?),
            "THROW" => Some(self.special_throw(items, environment)?),
            "WITH-CONDITION-RESTARTS" => {
                Some(self.special_with_condition_restarts(items, environment)?)
            }
            "WITH-SIMPLE-RESTART" => Some(self.special_with_simple_restart(items, environment)?),
            "WITH-OPEN-FILE" => {
                let expanded = Self::expand_with_open_file(form)?;
                Some(self.eval_expanded_values(&expanded, environment)?)
            }
            "RESTART-CASE" => Some(self.special_restart_case(items, environment)?),
            "UNWIND-PROTECT" => Some(self.special_unwind_protect(items, environment)?),
            "BLOCK" => Some(self.special_block(items, environment)?),
            "RETURN" => Some(self.special_return(items, environment)?),
            "RETURN-FROM" => Some(self.special_return_from(items, environment)?),
            "TAGBODY" => Some(self.special_tagbody(items, environment)?),
            "GO" => Some(Self::special_go(items, environment)?),
            "MULTIPLE-VALUE-BIND" => Some(self.special_multiple_value_bind(items, environment)?),
            "MULTIPLE-VALUE-CALL" => Some(self.special_multiple_value_call(items, environment)?),
            "MULTIPLE-VALUE-LIST" => Some(self.special_multiple_value_list(items, environment)?),
            "MULTIPLE-VALUE-PROG1" => Some(self.special_multiple_value_prog1(items, environment)?),
            _ => None,
        };
        Ok(value)
    }
}
