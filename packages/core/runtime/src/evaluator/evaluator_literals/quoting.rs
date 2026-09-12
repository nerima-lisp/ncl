use ncl_syntax::{Form, FormKind, SymbolTokenKind, parse_symbol_token};

use crate::environment::normalize_name;
use crate::evaluator::evaluator_literals::atom_parsing::literal_atom;
use crate::package::KEYWORD_PACKAGE;
use crate::{Runtime, RuntimeError, Value};

pub fn quoted_form_value(form: &Form) -> Result<Value, RuntimeError> {
    quoted_form_value_inner(form, None)
}

pub(crate) fn runtime_quoted_form_value(
    runtime: &Runtime,
    form: &Form,
) -> Result<Value, RuntimeError> {
    quoted_form_value_inner(form, Some(runtime))
}

fn quoted_form_value_inner(form: &Form, runtime: Option<&Runtime>) -> Result<Value, RuntimeError> {
    if let Some(value) = &form.original_value {
        return value
            .downcast_ref::<Value>()
            .cloned()
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: "opaque literal belongs to an incompatible runtime".to_string(),
                span: Some(form.span),
            });
    }
    match &form.kind {
        FormKind::CircularReference => Err(RuntimeError::InvalidForm {
            message: "circular reference has no original value".to_string(),
            span: Some(form.span),
        }),
        FormKind::Literal(value) => {
            if let Some(value) = value.downcast_ref::<Value>() {
                return Ok(value.clone());
            }
            if let Some(value) = value.downcast_ref::<String>() {
                return Ok(Value::string(value.clone()));
            }
            Err(RuntimeError::InvalidForm {
                message: "opaque literal belongs to an incompatible runtime".to_string(),
                span: Some(form.span),
            })
        }
        FormKind::Atom(atom) => {
            if let Ok(token) = parse_symbol_token(atom) {
                match token.kind {
                    SymbolTokenKind::Uninterned => {
                        return Ok(Value::uninterned_symbol(token.name));
                    }
                    SymbolTokenKind::Keyword => {
                        if let Some(runtime) = runtime
                            && let Some(value) = runtime.literal_symbol_value(
                                KEYWORD_PACKAGE,
                                &token.name,
                                token.escaped,
                            )
                        {
                            return Ok(value);
                        }
                        return Ok(if token.escaped {
                            Value::keyword_exact(token.name)
                        } else {
                            Value::keyword(token.name)
                        });
                    }
                    SymbolTokenKind::Symbol => {
                        if let Some(package) = token.package {
                            if let Some(runtime) = runtime
                                && let Some(value) = runtime.literal_symbol_value(
                                    &package,
                                    &token.name,
                                    token.escaped,
                                )
                            {
                                return Ok(value);
                            }
                            let name = format!("{}::{}", normalize_name(&package), token.name);
                            return Ok(if token.escaped {
                                Value::symbol_exact(name)
                            } else {
                                Value::symbol(name)
                            });
                        }
                        if let Some(value) = literal_atom(atom) {
                            return Ok(value);
                        }
                        if let Some(runtime) = runtime {
                            if let Some(value) =
                                runtime.literal_unqualified_symbol_value(&token.name, token.escaped)
                            {
                                return Ok(value);
                            }
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
                .map(|item| quoted_form_value_inner(item, runtime))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        FormKind::DottedList { items, tail } => Ok(Value::dotted_list(
            items
                .iter()
                .map(|item| quoted_form_value_inner(item, runtime))
                .collect::<Result<Vec<_>, _>>()?,
            quoted_form_value_inner(tail, runtime)?,
        )),
        FormKind::Vector(items) => Ok(Value::vector(
            items
                .iter()
                .map(|item| quoted_form_value_inner(item, runtime))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        FormKind::Complex { real, imaginary } => {
            let real = quoted_form_value_inner(real, runtime)?;
            let imaginary = quoted_form_value_inner(imaginary, runtime)?;
            if !real.is_real_number() || !imaginary.is_real_number() {
                return Err(RuntimeError::InvalidForm {
                    message: "complex literal components must be real numbers".to_string(),
                    span: Some(form.span),
                });
            }
            Ok(Value::complex(real, imaginary))
        }
    }
}

#[cfg(test)]
mod tests {
    use ncl_syntax::{Form, FormKind, OpaqueLiteral, Span};

    use super::{quoted_form_value, runtime_quoted_form_value};
    use crate::{Runtime, RuntimeError, Value};

    #[test]
    fn foreign_opaque_literal_returns_invalid_form() {
        let span = Span::new(3, 7);
        let form = Form::new(FormKind::Literal(OpaqueLiteral::new(42_u32)), span);

        match quoted_form_value(&form) {
            Err(RuntimeError::InvalidForm {
                message,
                span: error_span,
            }) => {
                assert_eq!(message, "opaque literal belongs to an incompatible runtime");
                assert_eq!(error_span, Some(span));
            }
            other => panic!("expected incompatible literal error, got {other:?}"),
        }
    }

    #[test]
    fn quoted_values_cover_composite_and_escaped_literals() {
        let span = Span::new(0, 1);
        let cases = [
            (Form::atom("|name|", span), "|name|"),
            (Form::atom("", span), ""),
            (Form::atom(":|key|", span), ":|key|"),
            (Form::atom("#:|temporary|", span), "#:temporary"),
            (Form::atom("pkg:|name|", span), "|PKG::name|"),
            (Form::new(FormKind::String("text".into()), span), "\"text\""),
            (Form::new(FormKind::Character('x'), span), "#\\x"),
            (Form::list(vec![Form::atom("1", span)], span), "(1)"),
            (
                Form::dotted_list(vec![Form::atom("1", span)], Form::atom("2", span), span),
                "(1 . 2)",
            ),
            (
                Form::new(FormKind::Vector(vec![Form::atom("1", span)]), span),
                "#(1)",
            ),
        ];

        for (form, expected) in cases {
            let value = match quoted_form_value(&form) {
                Ok(value) => value,
                Err(error) => panic!("literal form failed: {error}"),
            };
            assert_eq!(value.to_string(), expected);
        }
    }

    #[test]
    fn runtime_quoted_keyword_is_a_stable_interned_symbol() {
        let value =
            runtime_quoted_form_value(&Runtime::new(), &Form::atom(":ready", Span::new(0, 6)))
                .unwrap_or_else(|error| panic!("keyword literal failed: {error}"));
        assert!(
            matches!(value, Value::InternedSymbol(symbol) if symbol.keyword() && symbol.name() == "READY")
        );
    }

    #[test]
    fn runtime_quoted_unqualified_symbols_are_interned_in_current_package() {
        let runtime = Runtime::new();
        let ordinary = runtime_quoted_form_value(&runtime, &Form::atom("ready", Span::new(0, 5)))
            .unwrap_or_else(|error| panic!("ordinary symbol literal failed: {error}"));
        let exact = runtime_quoted_form_value(&runtime, &Form::atom("|Ready|", Span::new(0, 7)))
            .unwrap_or_else(|error| panic!("exact symbol literal failed: {error}"));
        let nil = runtime_quoted_form_value(&runtime, &Form::atom("NIL", Span::new(0, 3)))
            .unwrap_or_else(|error| panic!("NIL literal failed: {error}"));
        let truth = runtime_quoted_form_value(&runtime, &Form::atom("T", Span::new(0, 1)))
            .unwrap_or_else(|error| panic!("T literal failed: {error}"));

        match ordinary {
            Value::InternedSymbol(symbol) => {
                assert_eq!(symbol.name(), "READY");
                assert!(!symbol.exact());
                assert_eq!(symbol.package().name().as_deref(), Some("NCL-USER"));
            }
            other => panic!("expected interned ordinary symbol, got {other:?}"),
        }
        match exact {
            Value::InternedSymbol(symbol) => {
                assert_eq!(symbol.name(), "Ready");
                assert!(symbol.exact());
                assert_eq!(symbol.package().name().as_deref(), Some("NCL-USER"));
            }
            other => panic!("expected interned exact symbol, got {other:?}"),
        }
        assert!(matches!(nil, Value::Nil));
        assert!(matches!(truth, Value::Boolean(true)));
    }
}
