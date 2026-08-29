#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(super) fn apply_symbol_set(
        &self,
        arguments: &[Value],
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 2 {
            return Err(Self::arity("set", "two", arguments.len()));
        }
        let Some((name, exact)) = arguments[0].symbol_reference() else {
            return Err(Self::invalid("set first argument must be a symbol", span));
        };
        self.ensure_symbol_writable(name, exact, span)?;
        Ok(if exact {
            self.set_symbol_value_exact(name, arguments[1].clone())
        } else {
            self.set_symbol_value(name, arguments[1].clone())
        })
    }

    pub(super) fn apply_symbol_unbound(
        &self,
        name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(Self::arity(
                &name.to_ascii_lowercase(),
                "one",
                arguments.len(),
            ));
        }
        let Some((symbol_name, exact)) = arguments[0].symbol_reference() else {
            return Err(Self::invalid(
                "unbound operation argument must be a symbol",
                span,
            ));
        };
        if name == "MAKUNBOUND" {
            self.ensure_symbol_writable(symbol_name, exact, span)?;
            if exact {
                self.makunbound_exact_symbol(symbol_name);
            } else {
                self.makunbound_symbol(symbol_name);
            }
        } else if exact {
            self.fmakunbound_exact_symbol(symbol_name);
        } else {
            self.fmakunbound_symbol(symbol_name);
        }
        Ok(arguments[0].clone())
    }
}
