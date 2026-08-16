impl Runtime {
    pub(crate) fn eval_in(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        self.eval_values_in(form, environment)
            .map(|value| value.primary_value())
    }

    pub(crate) fn eval_values_in(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        match &form.kind {
            FormKind::Atom(atom) => {
                if let Some(expanded) = self.expand_symbol_macro_form(form, environment)? {
                    return self.eval_values_in(&expanded, environment);
                }
                self.eval_atom(atom, form.span, environment)
            }
            FormKind::String(value) => Ok(Value::string(value.clone())),
            FormKind::Character(value) => Ok(Value::Character(*value)),
            FormKind::Vector(items) => Ok(Value::vector(
                items
                    .iter()
                    .map(|item| self.quoted_value(item))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            FormKind::DottedList { .. } => {
                Err(self.invalid("cannot evaluate a dotted list", form.span))
            }
            FormKind::List(items) => self.eval_list_values(items, form.span, environment),
        }
    }

    fn eval_atom(
        &self,
        atom: &str,
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if let Some(value) = literal_atom(atom) {
            return Ok(value);
        }
        let (name, escaped) = resolved_symbol(atom);
        let value = if escaped {
            self.lookup_exact_in(&name, environment)
        } else {
            self.lookup_in(&name, environment)
        };
        value.ok_or_else(|| RuntimeError::UnboundVariable {
            name: normalize_name(&name),
            span: Some(span),
        })
    }

    fn eval_list_values(
        &self,
        items: &[Form],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let form = Form::list(items.to_vec(), span);
        let expanded = self.expand_macros(form, environment)?;
        self.eval_expanded_values(&expanded, environment)
    }

    fn eval_expanded_values(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return self.eval_values_in(form, environment);
        };
        let Some(operator) = items.first() else {
            return Ok(Value::Nil);
        };
        if let Some(name) = atom_name(operator) {
            let escaped = parse_symbol_token(name)
                .map(|token| token.escaped)
                .unwrap_or(false);
            if !escaped {
                match normalize_name(name).as_str() {
                    "QUOTE" => return self.special_quote(items, form.span),
                    "QUASIQUOTE" => return self.special_quasiquote(items, environment),
                    "DECLARE" => return Ok(Value::Nil),
                    "LOCALLY" => return self.special_locally(items, environment),
                    "WITH-COMPILATION-UNIT" => return self.special_progn(&items[1..], environment),
                    "EVAL-WHEN" => return self.special_eval_when(items, environment),
                    "DECLAIM" | "PROCLAIM" => return Ok(Value::Nil),
                    "THE" => return self.special_the(items, environment),
                    "LOAD-TIME-VALUE" => {
                        return self.special_load_time_value(items, environment);
                    }
                    "NTH-VALUE" => return self.special_nth_value(items, environment),
                    "IF" => return self.special_if(items, environment),
                    "PROGN" => return self.special_progn(&items[1..], environment),
                    "PROG1" => return self.special_prog1(items, environment),
                    "PROG2" => return self.special_prog2(items, environment),
                    "PROG" => return self.special_prog(items, environment, false),
                    "PROG*" => return self.special_prog(items, environment, true),
                    "VALUES" => return self.special_values(items, environment),
                    "IGNORE-ERRORS" => return self.special_ignore_errors(items, environment),
                    "HANDLER-CASE" => return self.special_handler_case(items, environment),
                    "HANDLER-BIND" => return self.special_handler_bind(items, environment),
                    "RESTART-BIND" => return self.special_restart_bind(items, environment),
                    "CATCH" => return self.special_catch(items, environment),
                    "PROGV" => return self.special_progv(items, environment),
                    "THROW" => return self.special_throw(items, environment),
                    "WITH-CONDITION-RESTARTS" => {
                        return self.special_with_condition_restarts(items, environment);
                    }
                    "WITH-SIMPLE-RESTART" => {
                        return self.special_with_simple_restart(items, environment);
                    }
                    "WITH-OPEN-FILE" => {
                        let expanded = self.expand_with_open_file(form)?;
                        return self.eval_expanded_values(&expanded, environment);
                    }
                    "WITH-OUTPUT-TO-STRING" => {
                        let expanded = self.expand_with_output_to_string(form)?;
                        return self.eval_expanded_values(&expanded, environment);
                    }
                    "WITH-INPUT-FROM-STRING" => {
                        let expanded = self.expand_with_input_from_string(form)?;
                        return self.eval_expanded_values(&expanded, environment);
                    }
                    "RESTART-CASE" => return self.special_restart_case(items, environment),
                    "UNWIND-PROTECT" => {
                        return self.special_unwind_protect(items, environment);
                    }
                    "BLOCK" => return self.special_block(items, environment),
                    "RETURN" => return self.special_return(items, environment),
                    "RETURN-FROM" => return self.special_return_from(items, environment),
                    "TAGBODY" => return self.special_tagbody(items, environment),
                    "GO" => return self.special_go(items, environment),
                    "MULTIPLE-VALUE-BIND" => {
                        return self.special_multiple_value_bind(items, environment);
                    }
                    "MULTIPLE-VALUE-CALL" => {
                        return self.special_multiple_value_call(items, environment);
                    }
                    "MULTIPLE-VALUE-LIST" => {
                        return self.special_multiple_value_list(items, environment);
                    }
                    "MULTIPLE-VALUE-PROG1" => {
                        return self.special_multiple_value_prog1(items, environment);
                    }
                    "AND" => return self.special_and(&items[1..], environment),
                    "OR" => return self.special_or(&items[1..], environment),
                    "WHEN" => return self.special_when(items, environment, true),
                    "UNLESS" => return self.special_when(items, environment, false),
                    "COND" => return self.special_cond(&items[1..], environment),
                    "CASE" => return self.special_case(items, environment, false),
                    "ECASE" => return self.special_case(items, environment, true),
                    "TYPECASE" => return self.special_typecase(items, environment, false),
                    "ETYPECASE" => return self.special_typecase(items, environment, true),
                    "DESTRUCTURING-BIND" => {
                        return self.special_destructuring_bind(items, environment);
                    }
                    "LET" => return self.special_let(items, environment, false),
                    "LET*" => return self.special_let(items, environment, true),
                    "FLET" => return self.special_flet(items, environment, false),
                    "LABELS" => return self.special_flet(items, environment, true),
                    "MACROLET" => return self.special_macrolet(items, environment),
                    "SYMBOL-MACROLET" => return self.special_symbol_macrolet(items, environment),
                    "DOTIMES" => return self.special_dotimes(items, environment),
                    "DOLIST" => return self.special_dolist(items, environment),
                    "DO" => return self.special_do(items, environment, false),
                    "DO*" => return self.special_do(items, environment, true),
                    "LAMBDA" => return self.special_lambda(items, environment),
                    "FUNCTION" => return self.special_function(items, environment),
                    "DEFUN" => return self.special_defun(items, environment),
                    "DEFMACRO" => return self.special_defmacro(items, environment),
                    "DEFINE-COMPILER-MACRO" => {
                        return self.special_define_compiler_macro(items, environment);
                    }
                    "DEFINE-MODIFY-MACRO" => {
                        return self.special_define_modify_macro(items, environment);
                    }
                    "MACROEXPAND-1" => return self.special_macroexpand_1(items, environment),
                    "MACROEXPAND" => return self.special_macroexpand(items, environment),
                    "DEFPACKAGE" => return self.special_defpackage(items),
                    "IN-PACKAGE" => return self.special_in_package(items),
                    "DEFINE" => return self.special_define(items, environment),
                    "DEFINE-SYMBOL-MACRO" => {
                        return self.special_define_symbol_macro(items, environment);
                    }
                    "SETQ" => return self.special_setq(items, environment),
                    "PSETQ" => return self.special_psetq(items, environment),
                    "MULTIPLE-VALUE-SETQ" => {
                        return self.special_multiple_value_setq(items, environment);
                    }
                    "SETF" => return self.special_setf(items, environment),
                    "PSETF" => return self.special_psetf(items, environment),
                    "PUSH" => return self.special_push(items, environment),
                    "POP" => return self.special_pop(items, environment),
                    "PUSHNEW" => return self.special_pushnew(items, environment),
                    "REMF" => return self.special_remf(items, environment),
                    "ROTATEF" => return self.special_rotatef(items, environment),
                    "SHIFTF" => return self.special_shiftf(items, environment),
                    "INCF" => {
                        return self.special_modify_symbol(items, environment, "INCF", "+");
                    }
                    "DECF" => {
                        return self.special_modify_symbol(items, environment, "DECF", "-");
                    }
                    "DEFSTRUCT" => return self.special_defstruct(items, environment),
                    "DEFINE-CONDITION" => return self.special_define_condition(items, environment),
                    "DEFCLASS" => return self.special_defclass(items, environment),
                    "DEFGENERIC" => return self.special_defgeneric(items, environment),
                    "DEFMETHOD" => return self.special_defmethod(items, environment),
                    "DEFSETF" => return self.special_defsetf(items, environment),
                    "DEFINE-SETF-EXPANDER" => {
                        return self.special_define_setf_expander(items, environment);
                    }
                    "GET-SETF-EXPANSION" => {
                        return self.special_get_setf_expansion(items, environment);
                    }
                    "DEFVAR" => return self.special_defvar(items, environment, false),
                    "DEFPARAMETER" => return self.special_defvar(items, environment, true),
                    "DEFCONSTANT" => return self.special_defconstant(items, environment),
                    "EVAL" => return self.special_eval(items, environment),
                    "FUNCALL" => return self.special_funcall(items, environment),
                    "APPLY" => return self.special_apply(items, environment),
                    "MAP-INTO" => return self.special_map_into(items, environment),
                    "MAPCAR" => return self.special_mapcar(items, environment),
                    _ => {}
                }
            }
        }

        let function = if let Some(name) = atom_name(operator) {
            let (resolved_name, escaped) = resolved_symbol(name);
            let function = if escaped {
                self.lookup_function_exact_in(&resolved_name, environment)
            } else {
                self.lookup_function_in(&resolved_name, environment)
            };
            function.ok_or_else(|| RuntimeError::UnboundVariable {
                name: if escaped {
                    resolved_name
                } else {
                    normalize_name(&resolved_name)
                },
                span: Some(operator.span),
            })?
        } else {
            self.eval_in(operator, environment)?
        };
        let arguments = items[1..]
            .iter()
            .map(|item| self.eval_in(item, environment))
            .collect::<Result<Vec<_>, _>>()?;
        self.apply_in(&function, &arguments, form.span, environment)
    }


}
