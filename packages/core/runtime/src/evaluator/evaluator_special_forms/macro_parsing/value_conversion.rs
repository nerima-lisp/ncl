use ncl_syntax::{Form, FormKind, OrdinaryLambdaList, Span, parse_ordinary_lambda_list};

use crate::evaluator::evaluator_literals::{
    escaped_symbol_atom, quoted_form_value, runtime_quoted_form_value,
};
use crate::package::KEYWORD_PACKAGE;
use crate::{Environment, Runtime, RuntimeError, Value};

mod compound;

impl Runtime {
    pub(crate) fn circular_form_error(form: &Form) -> RuntimeError {
        let message = match form
            .original_value
            .as_ref()
            .and_then(|value| value.downcast_ref::<Value>())
        {
            Some(Value::Vector(_)) => "circular vector cannot be converted to a form",
            _ => "circular cons cannot be converted to a form",
        };
        Self::invalid(message, form.span)
    }

    pub(crate) fn parameters(form: &Form) -> Result<OrdinaryLambdaList, RuntimeError> {
        parse_ordinary_lambda_list(form).map_err(|error| {
            let message = error.kind.to_string();
            Self::invalid(&message, error.span)
        })
    }

    pub(crate) fn eval_sequence_values(
        &self,
        forms: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let mut result = Value::Nil;
        for form in forms {
            result = self.eval_values_in(form, environment)?;
        }
        Ok(result)
    }

    pub(crate) fn quoted_value(form: &Form) -> Result<Value, RuntimeError> {
        quoted_form_value(form)
    }

    pub(crate) fn runtime_quoted_value(&self, form: &Form) -> Result<Value, RuntimeError> {
        runtime_quoted_form_value(self, form)
    }

    pub(crate) fn literal_symbol_value(
        &self,
        package: &str,
        name: &str,
        exact: bool,
    ) -> Option<Value> {
        let symbol = {
            let mut packages = self.packages.borrow_mut();
            if package == KEYWORD_PACKAGE {
                if exact {
                    packages.intern_exact_symbol(package, name);
                } else {
                    packages.intern_symbol(package, name);
                }
            }
            if exact {
                packages.exact_symbol_object_for(package, name)
            } else {
                packages.symbol_object_for(package, name)
            }
        };
        symbol.map(Value::interned_symbol)
    }

    pub(crate) fn literal_unqualified_symbol_value(
        &self,
        name: &str,
        exact: bool,
    ) -> Option<Value> {
        let symbol = {
            let package = self.current_package();
            let mut packages = self.packages.borrow_mut();
            if exact {
                packages.intern_exact_symbol(&package, name);
                packages.exact_symbol_object_for(&package, name)
            } else {
                let symbol = packages.symbol_object_for(&package, name);
                if symbol.is_some() {
                    symbol
                } else {
                    packages.intern_symbol(&package, name);
                    packages.symbol_object_for(&package, name)
                }
            }
        };
        symbol.map(Value::interned_symbol)
    }

    pub(crate) fn form_from_value(value: &Value, span: Span) -> Result<Form, RuntimeError> {
        Self::form_from_value_inner(value, span, &mut std::collections::HashSet::new())
    }

    pub(crate) fn retained_value_form(value: &Value, span: Span) -> Form {
        Form::new(
            FormKind::Literal(ncl_syntax::OpaqueLiteral::new(value.clone())),
            span,
        )
    }

    pub(crate) fn form_string(form: &Form) -> Option<&str> {
        match &form.kind {
            FormKind::String(value) => Some(value.as_str()),
            FormKind::Literal(value) => {
                value
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| match value.downcast_ref::<Value>()? {
                        Value::String(value) => Some(value.as_ref()),
                        _ => None,
                    })
            }
            _ => None,
        }
    }

    fn form_from_value_inner(
        value: &Value,
        span: Span,
        active: &mut std::collections::HashSet<usize>,
    ) -> Result<Form, RuntimeError> {
        let mut form = match value {
            Value::Nil | Value::Boolean(false) => Ok(Form::atom("NIL", span)),
            Value::Boolean(true) => Ok(Form::atom("T", span)),
            Value::Integer(value) => Ok(Form::atom(value.to_string(), span)),
            Value::BigInteger(value) => Ok(Form::atom(value.to_string(), span)),
            Value::Rational(value) => Ok(Form::atom(
                format!("{}/{}", value.numerator(), value.denominator()),
                span,
            )),
            Value::BigRational(value) => Ok(Form::atom(
                format!("{}/{}", value.numerator(), value.denominator()),
                span,
            )),
            Value::Character(value) => Ok(Form::new(FormKind::Character(*value), span)),
            Value::Symbol(value) => Ok(Form::atom(value.as_ref(), span)),
            Value::SymbolExact(value) => Ok(Form::atom(escaped_symbol_atom(value), span)),
            Value::InternedSymbol(value) => {
                let name = if value.exact() {
                    escaped_symbol_atom(value.name())
                } else {
                    value.name().to_string()
                };
                let atom = if value.keyword() {
                    format!(":{name}")
                } else {
                    match value.package().name() {
                        Some(package) if package == crate::package::DEFAULT_PACKAGE => name,
                        Some(package) => format!("{package}::{name}"),
                        None => value.reference(),
                    }
                };
                Ok(Form::atom(atom, span))
            }
            Value::UninternedSymbol(_) => {
                let (name, _) = value
                    .variable_reference()
                    .expect("uninterned symbols have variable references");
                Ok(Form::atom(name, span))
            }
            Value::Keyword(value) => Ok(Form::atom(format!(":{value}"), span)),
            Value::KeywordExact(value) => {
                Ok(Form::atom(format!(":{}", escaped_symbol_atom(value)), span))
            }
            Value::Cons(_) => Self::cons_form_from_value(value, span, active),
            Value::Vector(values) => {
                if active.insert(values.identity()) {
                    let converted = values
                        .snapshot()
                        .iter()
                        .map(|value| Self::form_from_value_inner(value, span, active))
                        .collect::<Result<Vec<_>, _>>();
                    active.remove(&values.identity());
                    Ok(Form::new(FormKind::Vector(converted?), span))
                } else {
                    Ok(Form::new(FormKind::CircularReference, span))
                }
            }
            Value::Float(_)
            | Value::Complex(_)
            | Value::Package(_)
            | Value::PackageObject(_)
            | Value::Array { .. }
            | Value::HashTable { .. }
            | Value::Stream(_)
            | Value::RandomState(_)
            | Value::Condition(_)
            | Value::Restart(_)
            | Value::Environment(_)
            | Value::Class(_)
            | Value::Instance(_)
            | Value::Structure { .. }
            | Value::Function(_) => Ok(Self::retained_value_form(value, span)),
            Value::String(value) => Ok(Form::new(
                FormKind::Literal(ncl_syntax::OpaqueLiteral::new(value.to_string())),
                span,
            )),
            Value::Values(_) | Value::Unbound => Err(RuntimeError::Type {
                expected: "FORM".to_string(),
                actual: value.type_name().to_string(),
                span: Some(span),
            }),
        }?;
        form.original_value = Some(ncl_syntax::OpaqueLiteral::new(value.clone()));
        Ok(form)
    }
}
