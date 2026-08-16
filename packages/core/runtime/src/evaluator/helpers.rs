fn atom_name(form: &Form) -> Option<&str> {
    match &form.kind {
        FormKind::Atom(value) => Some(value),
        _ => None,
    }
}

fn is_nil_form(form: &Form) -> bool {
    atom_name(form).is_some_and(|name| name.eq_ignore_ascii_case("nil"))
}

fn is_macro_keyword_form(form: &Form) -> bool {
    macro_keyword_name(form).is_some()
}

fn macro_keyword_name(form: &Form) -> Option<String> {
    let name = atom_name(form)?;
    let keyword = name.strip_prefix(':')?;
    (!keyword.is_empty()).then(|| normalize_name(keyword))
}

fn macro_dotted_parts(value: &Value) -> Option<(Vec<Value>, Value)> {
    match value {
        Value::Nil => Some((Vec::new(), Value::Nil)),
        Value::List(values) => Some((values.as_ref().clone(), Value::Nil)),
        Value::DottedList { items, tail } => {
            let mut values = items.as_ref().clone();
            match tail.as_ref() {
                Value::Nil => Some((values, Value::Nil)),
                Value::List(more) => {
                    values.extend(more.as_ref().iter().cloned());
                    Some((values, Value::Nil))
                }
                Value::DottedList { .. } => {
                    let (more, dotted_tail) = macro_dotted_parts(tail)?;
                    values.extend(more);
                    Some((values, dotted_tail))
                }
                other => Some((values, other.clone())),
            }
        }
        _ => None,
    }
}

fn control_tag(form: &Form) -> Option<String> {
    let name = atom_name(form)?;
    if name.is_empty() || name == ":" {
        return None;
    }
    if name.starts_with(':') {
        return (name.len() > 1).then(|| normalize_name(name));
    }
    if name.eq_ignore_ascii_case("nil")
        || name.eq_ignore_ascii_case("t")
        || name.parse::<i64>().is_ok()
        || literal_atom(name).is_none()
    {
        Some(normalize_name(name))
    } else {
        None
    }
}

fn unqualified_name(name: &str) -> String {
    let normalized = normalize_name(name);
    package::split_symbol(&normalized)
        .map(|(_, symbol, _)| symbol.to_string())
        .unwrap_or(normalized)
}

fn is_special_operator_name(name: &str) -> bool {
    matches!(
        unqualified_name(name).as_str(),
        "BLOCK"
            | "CATCH"
            | "EVAL-WHEN"
            | "FLET"
            | "FUNCTION"
            | "GO"
            | "IF"
            | "LABELS"
            | "LET"
            | "LET*"
            | "LOAD-TIME-VALUE"
            | "LOCALLY"
            | "WITH-COMPILATION-UNIT"
            | "MACROLET"
            | "MULTIPLE-VALUE-CALL"
            | "MULTIPLE-VALUE-PROG1"
            | "PROGN"
            | "PROGV"
            | "QUOTE"
            | "SETQ"
            | "SYMBOL-MACROLET"
            | "TAGBODY"
            | "THE"
            | "THROW"
            | "UNWIND-PROTECT"
            | "REMF"
    )
}

fn is_case_default_form(form: &Form) -> bool {
    let Some(name) = atom_name(form) else {
        return false;
    };
    let Ok(token) = parse_symbol_token(name) else {
        return false;
    };
    token.kind == SymbolTokenKind::Symbol
        && !token.escaped
        && matches!(unqualified_name(name).as_str(), "T" | "OTHERWISE")
}

fn is_operator_form(form: &Form, name: &str) -> bool {
    match &form.kind {
        FormKind::List(items) => items
            .first()
            .and_then(atom_name)
            .is_some_and(|operator| operator.eq_ignore_ascii_case(name)),
        _ => false,
    }
}

fn is_special_form(form: &Form) -> bool {
    let Some(operator) = atom_name(form) else {
        return false;
    };
    matches!(
        normalize_name(operator).as_str(),
        "QUOTE"
            | "QUASIQUOTE"
            | "DECLARE"
            | "LOCALLY"
            | "EVAL-WHEN"
            | "LOAD-TIME-VALUE"
            | "NTH-VALUE"
            | "DECLAIM"
            | "PROCLAIM"
            | "THE"
            | "IF"
            | "PROGN"
            | "PROG1"
            | "PROG2"
            | "PROG"
            | "PROG*"
            | "VALUES"
            | "IGNORE-ERRORS"
            | "HANDLER-CASE"
            | "HANDLER-BIND"
            | "RESTART-BIND"
            | "WITH-CONDITION-RESTARTS"
            | "CATCH"
            | "PROGV"
            | "THROW"
            | "WITH-SIMPLE-RESTART"
            | "WITH-OPEN-FILE"
            | "WITH-OUTPUT-TO-STRING"
            | "WITH-INPUT-FROM-STRING"
            | "RESTART-CASE"
            | "UNWIND-PROTECT"
            | "BLOCK"
            | "RETURN"
            | "RETURN-FROM"
            | "TAGBODY"
            | "GO"
            | "MULTIPLE-VALUE-BIND"
            | "MULTIPLE-VALUE-CALL"
            | "MULTIPLE-VALUE-LIST"
            | "MULTIPLE-VALUE-PROG1"
            | "AND"
            | "OR"
            | "WHEN"
            | "UNLESS"
            | "COND"
            | "CASE"
            | "ECASE"
            | "TYPECASE"
            | "ETYPECASE"
            | "DESTRUCTURING-BIND"
            | "LET"
            | "LET*"
            | "FLET"
            | "LABELS"
            | "MACROLET"
            | "SYMBOL-MACROLET"
            | "DOTIMES"
            | "DOLIST"
            | "DO"
            | "DO*"
            | "LAMBDA"
            | "FUNCTION"
            | "DEFUN"
            | "DEFMACRO"
            | "DEFINE-MODIFY-MACRO"
            | "MACROEXPAND-1"
            | "MACROEXPAND"
            | "DEFPACKAGE"
            | "IN-PACKAGE"
            | "DEFINE"
            | "DEFINE-SYMBOL-MACRO"
            | "SETQ"
            | "PSETQ"
            | "MULTIPLE-VALUE-SETQ"
            | "SETF"
            | "PSETF"
            | "PUSH"
            | "POP"
            | "PUSHNEW"
            | "REMF"
            | "ROTATEF"
            | "SHIFTF"
            | "DEFSETF"
            | "INCF"
            | "DECF"
            | "DEFSTRUCT"
            | "DEFINE-CONDITION"
            | "DEFCLASS"
            | "DEFGENERIC"
            | "DEFMETHOD"
            | "DEFVAR"
            | "DEFPARAMETER"
            | "DEFCONSTANT"
            | "DEFINE-SETF-EXPANDER"
            | "GET-SETF-EXPANSION"
            | "EVAL"
            | "FUNCALL"
            | "APPLY"
            | "MAP-INTO"
            | "MAPCAR"
    )
}

fn prefix_argument<'form>(items: &'form [Form], name: &str) -> Option<&'form Form> {
    if items.len() != 2 {
        return None;
    }
    atom_name(&items[0]).filter(|operator| operator.eq_ignore_ascii_case(name))?;
    Some(&items[1])
}

fn quasiquote_marker(name: &str, value: Value) -> Value {
    Value::list(vec![Value::symbol(name), value])
}

pub(crate) fn quoted_form_value(form: &Form) -> Result<Value, RuntimeError> {
    match &form.kind {
        FormKind::Atom(atom) => {
            if let Ok(token) = parse_symbol_token(atom) {
                match token.kind {
                    SymbolTokenKind::Uninterned => {
                        return Ok(Value::uninterned_symbol(token.name));
                    }
                    SymbolTokenKind::Keyword => {
                        return Ok(if token.escaped {
                            Value::keyword_exact(token.name)
                        } else {
                            Value::keyword(token.name)
                        });
                    }
                    SymbolTokenKind::Symbol => {
                        if let Some(package) = token.package {
                            let name = format!("{}::{}", normalize_name(&package), token.name);
                            return Ok(if token.escaped {
                                Value::symbol_exact(name)
                            } else {
                                Value::symbol(name)
                            });
                        }
                        if token.escaped {
                            return Ok(Value::symbol_exact(token.name));
                        }
                    }
                }
            }
            Ok(literal_atom(atom).unwrap_or_else(|| Value::symbol(atom)))
        }
        FormKind::String(value) => Ok(Value::string(value.clone())),
        FormKind::Character(value) => Ok(Value::Character(*value)),
        FormKind::List(items) => Ok(Value::list(
            items
                .iter()
                .map(quoted_form_value)
                .collect::<Result<Vec<_>, _>>()?,
        )),
        FormKind::DottedList { items, tail } => Ok(Value::dotted_list(
            items
                .iter()
                .map(quoted_form_value)
                .collect::<Result<Vec<_>, _>>()?,
            quoted_form_value(tail)?,
        )),
        FormKind::Vector(items) => Ok(Value::vector(
            items
                .iter()
                .map(quoted_form_value)
                .collect::<Result<Vec<_>, _>>()?,
        )),
    }
}

fn escaped_symbol_atom(value: &str) -> String {
    let mut result = String::with_capacity(value.len() + 2);
    result.push('|');
    for character in value.chars() {
        if matches!(character, '|' | '\\') {
            result.push('\\');
        }
        result.push(character);
    }
    result.push('|');
    result
}

fn generated_form_span() -> Span {
    Span::new(0, 0)
}

fn lambda_symbol_form(name: &str, escaped: bool) -> Form {
    let atom = if escaped {
        escaped_symbol_atom(name)
    } else {
        name.to_string()
    };
    Form::atom(atom, generated_form_span())
}

fn lambda_keyword_form(name: &str, escaped: bool) -> Form {
    let atom = if escaped {
        format!(":{}", escaped_symbol_atom(name))
    } else {
        format!(":{name}")
    };
    Form::atom(atom, generated_form_span())
}

fn lambda_optional_form(parameter: &LambdaListOptionalParameter) -> Form {
    if !parameter.init_form_supplied && parameter.supplied_p.is_none() {
        return lambda_symbol_form(&parameter.name, parameter.name_escaped);
    }
    let mut items = vec![
        lambda_symbol_form(&parameter.name, parameter.name_escaped),
        parameter.init_form.clone(),
    ];
    if let Some(supplied_p) = &parameter.supplied_p {
        items.push(lambda_symbol_form(
            supplied_p,
            parameter.supplied_p_escaped.unwrap_or(false),
        ));
    }
    Form::list(items, generated_form_span())
}

fn lambda_keyword_parameter_form(parameter: &LambdaListKeywordParameter) -> Form {
    let binding = if parameter.keyword_name == parameter.name
        && parameter.keyword_name_escaped == parameter.name_escaped
    {
        lambda_symbol_form(&parameter.name, parameter.name_escaped)
    } else {
        Form::list(
            vec![
                lambda_keyword_form(&parameter.keyword_name, parameter.keyword_name_escaped),
                lambda_symbol_form(&parameter.name, parameter.name_escaped),
            ],
            generated_form_span(),
        )
    };
    if !parameter.init_form_supplied && parameter.supplied_p.is_none() {
        return binding;
    }
    let mut items = vec![binding, parameter.init_form.clone()];
    if let Some(supplied_p) = &parameter.supplied_p {
        items.push(lambda_symbol_form(
            supplied_p,
            parameter.supplied_p_escaped.unwrap_or(false),
        ));
    }
    Form::list(items, generated_form_span())
}

fn lambda_auxiliary_form(parameter: &LambdaListAuxiliaryParameter) -> Form {
    if parameter.init_form == Form::atom("NIL", parameter.init_form.span) {
        return lambda_symbol_form(&parameter.name, parameter.name_escaped);
    }
    Form::list(
        vec![
            lambda_symbol_form(&parameter.name, parameter.name_escaped),
            parameter.init_form.clone(),
        ],
        generated_form_span(),
    )
}

fn closure_lambda_form(data: ClosureLambdaForm<'_>) -> Form {
    let ClosureLambdaForm {
        parameters,
        required_escaped,
        optional,
        rest,
        rest_escaped,
        keywords,
        has_keyword_section,
        allow_other_keys,
        auxiliary,
        body,
    } = data;
    let mut lambda_list = Vec::new();
    for (name, escaped) in parameters.iter().zip(required_escaped.iter().copied()) {
        lambda_list.push(lambda_symbol_form(name, escaped));
    }
    if !optional.is_empty() {
        lambda_list.push(Form::atom("&OPTIONAL", generated_form_span()));
        lambda_list.extend(optional.iter().map(lambda_optional_form));
    }
    if let Some(rest) = rest {
        lambda_list.push(Form::atom("&REST", generated_form_span()));
        lambda_list.push(lambda_symbol_form(rest, rest_escaped));
    }
    if has_keyword_section {
        lambda_list.push(Form::atom("&KEY", generated_form_span()));
        lambda_list.extend(keywords.iter().map(lambda_keyword_parameter_form));
        if allow_other_keys {
            lambda_list.push(Form::atom("&ALLOW-OTHER-KEYS", generated_form_span()));
        }
    }
    if !auxiliary.is_empty() {
        lambda_list.push(Form::atom("&AUX", generated_form_span()));
        lambda_list.extend(auxiliary.iter().map(lambda_auxiliary_form));
    }
    let mut lambda = vec![
        Form::atom("LAMBDA", generated_form_span()),
        Form::list(lambda_list, generated_form_span()),
    ];
    lambda.extend(body.iter().cloned());
    Form::list(lambda, generated_form_span())
}

fn literal_atom(atom: &str) -> Option<Value> {
    let token = parse_symbol_token(atom).ok()?;
    match token.kind {
        SymbolTokenKind::Keyword => Some(if token.escaped {
            Value::keyword_exact(token.name)
        } else {
            Value::keyword(token.name)
        }),
        SymbolTokenKind::Symbol if token.package.is_none() && !token.escaped => {
            match token.name.as_str() {
                "NIL" | "#F" => return Some(Value::Nil),
                "T" | "#T" => return Some(Value::boolean(true)),
                _ => {}
            }
            if let Ok(value) = token.name.parse::<i64>() {
                return Some(Value::Integer(value));
            }
            if let Some((numerator, denominator)) = token.name.split_once('/')
                && let (Ok(numerator), Ok(denominator)) =
                    (numerator.parse::<i128>(), denominator.parse::<i128>())
            {
                return Value::rational(numerator, denominator).ok();
            }
            token.name.parse::<f64>().ok().map(Value::Float)
        }
        _ => None,
    }
}

fn resolved_symbol(atom: &str) -> (String, bool) {
    let Ok(token) = parse_symbol_token(atom) else {
        return (normalize_name(atom), false);
    };
    match token.kind {
        SymbolTokenKind::Uninterned => (format!("#:{}", token.name), token.escaped),
        SymbolTokenKind::Keyword => (format!(":{}", token.name), token.escaped),
        SymbolTokenKind::Symbol => {
            let name = if token.escaped {
                token.name
            } else {
                normalize_name(&token.name)
            };
            let resolved = token.package.map_or(name.clone(), |package| {
                package::canonical_symbol_name(&package, &name)
            });
            (resolved, token.escaped)
        }
    }
}
