#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn apply_symbol_creation_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        if !matches!(
            name,
            "MAKE-SYMBOL" | "GENSYM" | "GENTEMP" | "INTERN" | "FIND-SYMBOL"
        ) {
            return None;
        }
        Some((|| -> Result<Value, RuntimeError> {
            match name {
                "MAKE-SYMBOL" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("make-symbol", "one", arguments.len()));
                    }
                    let Value::String(value) = &arguments[0] else {
                        return Err(Self::invalid("make-symbol argument must be a string", span));
                    };
                    Ok(Value::uninterned_symbol(value.as_ref()))
                }
                "GENSYM" => {
                    if arguments.len() > 1 {
                        return Err(Self::arity("gensym", "zero or one", arguments.len()));
                    }
                    let prefix = match arguments.first() {
                        None => "G".into(),
                        Some(Value::String(v)) => v.to_string(),
                        Some(v) => v.symbol_name().map(str::to_owned).ok_or_else(|| {
                            Self::invalid("gensym prefix must be a string designator", span)
                        })?,
                    };
                    let counter = self.gensym_counter.get();
                    self.gensym_counter.set(counter.wrapping_add(1));
                    Ok(Value::uninterned_symbol(format!("{prefix}{counter}")))
                }
                "GENTEMP" => {
                    if !(0..=2).contains(&arguments.len()) {
                        return Err(Self::arity("gentemp", "zero, one, or two", arguments.len()));
                    }
                    let prefix = match arguments.first() {
                        None => "T".to_string(),
                        Some(Value::String(value)) => value.to_string(),
                        Some(_) => {
                            return Err(Self::invalid("gentemp prefix must be a string", span));
                        }
                    };
                    let package_name = arguments
                        .get(1)
                        .map(|value| self.package_name_from_value(value, span))
                        .transpose()?
                        .unwrap_or_else(|| self.current_package());
                    if package_name == package::COMMON_LISP_PACKAGE {
                        return Err(Self::package_error(
                            "GENTEMP cannot create symbols in this package",
                            span,
                        ));
                    }
                    loop {
                        let counter = self.gentemp_counter.get();
                        self.gentemp_counter.set(counter.wrapping_add(1));
                        let candidate = format!("{prefix}{counter}");
                        let created = {
                            let mut packages = self.packages.borrow_mut();
                            if packages.symbol_exists_exact(&package_name, &candidate) {
                                false
                            } else {
                                packages.intern_exact_symbol(&package_name, &candidate);
                                true
                            }
                        };
                        if created {
                            return Ok(self.package_symbol_value(&package_name, &candidate));
                        }
                    }
                }
                "INTERN" | "FIND-SYMBOL" => {
                    if !(1..=2).contains(&arguments.len()) {
                        return Err(Self::arity(
                            &name.to_ascii_lowercase(),
                            "one or two",
                            arguments.len(),
                        ));
                    }
                    let symbol_name = if name == "FIND-SYMBOL" {
                        Self::symbol_lookup_name_from_value(&arguments[0], span)?
                    } else {
                        Self::symbol_name_from_value(&arguments[0], span)?
                    };
                    let package_name = arguments
                        .get(1)
                        .map(|v| self.package_name_from_value(v, span))
                        .transpose()?
                        .unwrap_or_else(|| self.current_package());
                    if name == "INTERN" {
                        let Some(status) = self
                            .packages
                            .borrow_mut()
                            .intern_symbol(&package_name, &symbol_name)
                        else {
                            return Err(Self::package_error(
                                &format!("unknown package {package_name}"),
                                span,
                            ));
                        };
                        Ok(Value::values(vec![
                            self.package_symbol_value(&package_name, &symbol_name),
                            Self::symbol_status_value(status),
                        ]))
                    } else {
                        let status = self
                            .packages
                            .borrow()
                            .symbol_status_exact(&package_name, &symbol_name);
                        match status {
                            None => Ok(Value::values(vec![Value::Nil, Value::Nil])),
                            Some(status) => Ok(Value::values(vec![
                                self.package_symbol_value(&package_name, &symbol_name),
                                Self::symbol_status_value(status),
                            ])),
                        }
                    }
                }
                _ => unreachable!("symbol creation primitive name was prevalidated"),
            }
        })())
    }
}
