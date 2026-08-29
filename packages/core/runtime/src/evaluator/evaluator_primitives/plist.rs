#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn apply_symbol_property_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        match name {
            "GET" => Some(Self::apply_get_property(arguments, environment, span)),
            "PUTPROP" => Some(Self::apply_put_property(arguments, environment, span)),
            "REMPROP" => Some(Self::apply_rem_property(arguments, environment, span)),
            "SYMBOL-PLIST" => Some(Self::apply_symbol_plist(arguments, environment, span)),
            "SET" => Some(self.apply_symbol_set(arguments, span)),
            "MAKUNBOUND" | "FMAKUNBOUND" => Some(self.apply_symbol_unbound(name, arguments, span)),
            _ => None,
        }
    }

    fn apply_get_property(
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !(2..=3).contains(&arguments.len()) {
            return Err(Self::arity("get", "two or three", arguments.len()));
        }
        if arguments[0].symbol_reference().is_none() {
            return Err(Self::invalid("get first argument must be a symbol", span));
        }
        let plist = environment
            .symbol_plist(&arguments[0])
            .unwrap_or(Value::Nil);
        let Some(properties) = plist.list_items() else {
            return Err(RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: plist.type_name().to_string(),
                span: Some(span),
            });
        };
        if !properties.len().is_multiple_of(2) {
            return Err(Self::invalid("GET needs an even property list", span));
        }
        for index in (0..properties.len()).step_by(2) {
            if properties[index].eq_value(&arguments[1]) {
                return Ok(properties[index + 1].clone());
            }
        }
        Ok(arguments.get(2).cloned().unwrap_or(Value::Nil))
    }

    fn apply_put_property(
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 3 {
            return Err(Self::arity("putprop", "three", arguments.len()));
        }
        if arguments[0].symbol_reference().is_none() {
            return Err(Self::invalid(
                "putprop first argument must be a symbol",
                span,
            ));
        }
        let plist = environment
            .symbol_plist(&arguments[0])
            .unwrap_or(Value::Nil);
        let Some(mut properties) = plist.list_items() else {
            return Err(RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: plist.type_name().to_string(),
                span: Some(span),
            });
        };
        if !properties.len().is_multiple_of(2) {
            return Err(Self::invalid("PUTPROP needs an even property list", span));
        }
        if let Some(index) = (0..properties.len())
            .step_by(2)
            .find(|index| properties[*index].eq_value(&arguments[2]))
        {
            properties[index] = arguments[1].clone();
        } else {
            properties.extend([arguments[2].clone(), arguments[1].clone()]);
        }
        environment.set_symbol_plist(&arguments[0], Value::list(properties));
        Ok(arguments[1].clone())
    }

    fn apply_rem_property(
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 2 {
            return Err(Self::arity("remprop", "two", arguments.len()));
        }
        if arguments[0].symbol_reference().is_none() {
            return Err(Self::invalid(
                "remprop first argument must be a symbol",
                span,
            ));
        }
        let plist = environment
            .symbol_plist(&arguments[0])
            .unwrap_or(Value::Nil);
        let Some(mut properties) = plist.list_items() else {
            return Err(RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: plist.type_name().to_string(),
                span: Some(span),
            });
        };
        if !properties.len().is_multiple_of(2) {
            return Err(Self::invalid("REMPROP needs an even property list", span));
        }
        let Some(index) = (0..properties.len())
            .step_by(2)
            .find(|index| properties[*index].eq_value(&arguments[1]))
        else {
            return Ok(Value::Nil);
        };
        properties.drain(index..index + 2);
        if properties.is_empty() {
            environment.remove_symbol_property(&arguments[0]);
        } else {
            environment.set_symbol_plist(&arguments[0], Value::list(properties));
        }
        Ok(Value::boolean(true))
    }

    fn apply_symbol_plist(
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(Self::arity("symbol-plist", "one", arguments.len()));
        }
        if arguments[0].symbol_reference().is_none() {
            return Err(Self::invalid(
                "symbol-plist argument must be a symbol",
                span,
            ));
        }
        Ok(environment
            .symbol_plist(&arguments[0])
            .unwrap_or(Value::Nil))
    }
}
