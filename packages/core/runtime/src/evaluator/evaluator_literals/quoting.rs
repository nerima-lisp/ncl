use ncl_syntax::{Form, FormKind, SymbolTokenKind, parse_symbol_token};

use crate::environment::normalize_name;
use crate::evaluator::evaluator_literals::atom_parsing::literal_atom;
use crate::{RuntimeError, Value};

pub fn quoted_form_value(form: &Form) -> Result<Value, RuntimeError> {
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

#[cfg(test)]
mod tests {
    use ncl_syntax::{Form, FormKind, Span};

    use super::quoted_form_value;

    #[test]
    fn quoted_values_cover_composite_and_escaped_literals() {
        let span = Span::new(0, 1);
        let cases = [
            (Form::atom("|name|", span), "|name|"),
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
}
