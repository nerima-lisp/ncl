#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(super) fn eval_special_form_iteration(
        &self,
        items: &[Form],
        name: &str,
        environment: &Environment,
    ) -> Result<Option<Value>, RuntimeError> {
        let value = match normalize_name(name).as_str() {
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
        let value = match normalize_name(name).as_str() {
            "DESTRUCTURING-BIND" => Some(self.special_destructuring_bind(items, environment)?),
            "LET" => Some(self.special_let(items, environment, false)?),
            "LET*" => Some(self.special_let(items, environment, true)?),
            "FLET" => Some(self.special_flet(items, environment, false)?),
            "LABELS" => Some(self.special_flet(items, environment, true)?),
            "WITH-HASH-TABLE-ITERATOR" => {
                Some(self.special_with_hash_table_iterator(items, environment)?)
            }
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
        let value = match normalize_name(name).as_str() {
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
        let value = match normalize_name(name).as_str() {
            "QUOTE" => Some(self.special_quote(items, form.span)?),
            "QUASIQUOTE" => Some(self.special_quasiquote(items, environment)?),
            "DECLARE" | "DECLAIM" | "PROCLAIM" => Some(Value::Nil),
            "LOCALLY" => Some(self.special_locally(items, environment)?),
            "EVAL-WHEN" => Some(self.special_eval_when(items, environment)?),
            "WITH-COMPILATION-UNIT" => {
                Some(self.special_with_compilation_unit(items, environment)?)
            }
            "THE" => Some(self.special_the(items, environment)?),
            "LOAD-TIME-VALUE" => Some(self.special_load_time_value(items)?),
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
                let expanded = self.expand_with_open_file(form)?;
                Some(self.eval_expanded_values(&expanded, environment)?)
            }
            "WITH-OPEN-STREAM" => {
                let expanded = self.expand_with_open_stream(form)?;
                Some(self.eval_expanded_values(&expanded, environment)?)
            }
            "WITH-INPUT-FROM-STRING" => {
                Some(self.special_with_input_from_string(items, environment)?)
            }
            "WITH-OUTPUT-TO-STRING" => {
                Some(self.special_with_output_to_string(items, environment)?)
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

impl Runtime {
    fn special_with_input_from_string(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(Self::arity(
                "with-input-from-string",
                "at least two",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(binding) = &items[1].kind else {
            return Err(Self::invalid(
                "with-input-from-string binding must be a list",
                items[1].span,
            ));
        };
        if binding.len() < 2 {
            return Err(Self::invalid(
                "with-input-from-string binding needs a stream variable and string",
                items[1].span,
            ));
        }
        let variable = Self::variable_name(&binding[0], "with-input-from-string stream variable")?;
        let source = self.eval_in(&binding[1], environment)?;
        let mut start = None;
        let mut end = None;
        let mut index = None;
        let mut cursor = 2;
        while cursor < binding.len() {
            let key = atom_name(&binding[cursor]).ok_or_else(|| {
                Self::invalid(
                    "with-input-from-string option must be a keyword",
                    binding[cursor].span,
                )
            })?;
            let key = normalize_name(key);
            cursor += 1;
            if cursor >= binding.len() {
                return Err(Self::invalid(
                    "with-input-from-string option needs a value",
                    binding[cursor - 1].span,
                ));
            }
            match key.as_str() {
                ":START" => start = Some(self.eval_in(&binding[cursor], environment)?),
                ":END" => end = Some(self.eval_in(&binding[cursor], environment)?),
                ":INDEX" => {
                    index = Some(Self::variable_name(
                        &binding[cursor],
                        "with-input-from-string index variable",
                    )?)
                }
                _ => {
                    return Err(Self::invalid(
                        "unknown with-input-from-string option",
                        binding[cursor - 1].span,
                    ));
                }
            }
            cursor += 1;
        }
        let mut arguments = vec![source];
        if start.is_some() || end.is_some() {
            arguments.push(start.unwrap_or(Value::Integer(0)));
            if let Some(end) = end {
                arguments.push(end);
            }
        }
        let stream_value = crate::builtins::make_string_input_stream(&arguments)?;
        let Value::Stream(stream) = &stream_value else {
            return Err(Self::invalid(
                "make-string-input-stream did not return a stream",
                items[1].span,
            ));
        };
        let body_environment = environment.child();
        body_environment.define(&variable, stream_value.clone());
        let _standard_stream_guard =
            crate::builtins::standard_streams::bind(stream_value.clone(), Value::Nil);
        let result = crate::builtins::with_stream_context(Some(stream.clone()), None, || {
            self.special_progn(&items[2..], &body_environment)
        })?;
        if let Some(index) = index {
            let position = stream
                .borrow()
                .position()
                .map(|value| Value::Integer(value as i64))
                .unwrap_or(Value::Nil);
            if !body_environment.set(&index, position.clone()) {
                body_environment.define(&index, position);
            }
        }
        Ok(result)
    }

    fn special_with_output_to_string(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(Self::arity(
                "with-output-to-string",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(binding) = &items[1].kind else {
            return Err(Self::invalid(
                "with-output-to-string binding must be a list",
                items[1].span,
            ));
        };
        if !(1..=2).contains(&binding.len()) {
            return Err(Self::invalid(
                "with-output-to-string binding needs one or two variables",
                items[1].span,
            ));
        }
        let variable = Self::variable_name(&binding[0], "with-output-to-string stream variable")?;
        let destination = binding
            .get(1)
            .map(|form| Self::variable_name(form, "with-output-to-string destination"))
            .transpose()?;
        let stream_value = crate::builtins::make_string_output_stream(&[])?;
        let Value::Stream(stream) = &stream_value else {
            return Err(Self::invalid(
                "make-string-output-stream did not return a stream",
                items[1].span,
            ));
        };
        let body_environment = environment.child();
        body_environment.define(&variable, stream_value.clone());
        let _standard_stream_guard =
            crate::builtins::standard_streams::bind(Value::Nil, stream_value.clone());
        crate::builtins::with_stream_context(None, Some(stream.clone()), || {
            self.special_progn(&items[2..], &body_environment)
        })?;
        let output = crate::builtins::get_output_stream_string(&[stream_value])?;
        if let Some(destination) = destination.filter(|destination| {
            matches!(
                environment.lookup(destination.as_str()),
                Some(Value::String(_))
            )
        }) {
            if !environment.set(&destination, output.clone()) {
                environment.define(&destination, output.clone());
            }
        }
        Ok(output)
    }
}
