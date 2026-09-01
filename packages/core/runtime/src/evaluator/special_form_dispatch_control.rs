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
            "WITH-OPEN-STREAM" => {
                let expanded = Self::expand_with_open_stream(form)?;
                Some(self.eval_expanded_values(&expanded, environment)?)
            }
            "WITH-INPUT-FROM-STRING" => Some(self.special_with_input_from_string(items, environment)?),
            "WITH-OUTPUT-TO-STRING" => Some(self.special_with_output_to_string(items, environment)?),
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

impl Runtime {
    fn special_with_input_from_string(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(Self::arity("with-input-from-string", "at least 2", items.len().saturating_sub(1)));
        }
        let binding = match &items[1].kind { ncl_syntax::FormKind::List(parts) if parts.len() >= 2 => parts, _ => return Err(Self::invalid("with-input-from-string binding must contain a variable and string form", items[1].span)) };
        let name = match &binding[0].kind { ncl_syntax::FormKind::Atom(name) => name, _ => return Err(Self::invalid("with-input-from-string variable must be a symbol", binding[0].span)) };
        let mut arguments = Vec::with_capacity(binding.len() - 1);
        for form in &binding[1..] {
            arguments.push(self.eval_values_in(form, environment)?.primary_value());
        }
        let stream = crate::builtins::make_string_input_stream(&arguments)?;
        let local = environment.child();
        local.define(name, stream.clone());
        let _guard = crate::builtins::standard_streams::bind(stream, Value::Nil);
        self.special_progn(&items[2..], &local)
    }

    fn special_with_output_to_string(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 { return Err(Self::arity("with-output-to-string", "at least 1", 0)); }
        let binding = match &items[1].kind { ncl_syntax::FormKind::List(parts) if !parts.is_empty() => parts, _ => return Err(Self::invalid("with-output-to-string binding must contain a variable", items[1].span)) };
        let name = match &binding[0].kind { ncl_syntax::FormKind::Atom(name) => name, _ => return Err(Self::invalid("with-output-to-string variable must be a symbol", binding[0].span)) };
        let stream = Value::string_output_stream();
        let local = environment.child();
        local.define(name, stream.clone());
        { let _guard = crate::builtins::standard_streams::bind(Value::Nil, stream.clone()); self.special_progn(&items[2..], &local)?; }
        crate::builtins::get_output_stream_string(&[stream])
    }
}
