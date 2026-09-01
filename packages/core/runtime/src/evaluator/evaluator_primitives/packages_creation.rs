#![allow(clippy::wildcard_imports)]
use super::*;
use std::collections::{HashMap, HashSet};

impl Runtime {
    pub(crate) fn apply_package_creation_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        if name == "DELETE-PACKAGE" {
            return Some((|| -> Result<Value, RuntimeError> {
                if arguments.len() != 1 {
                    return Err(Self::arity(name, "one", arguments.len()));
                }
                let package_name = Self::package_designator_name(&arguments[0], span)?;
                Ok(Value::boolean(self.packages.borrow_mut().delete_package(&package_name)))
            })());
        }
        if name == "RENAME-PACKAGE" {
            return Some((|| -> Result<Value, RuntimeError> {
                if arguments.len() != 2 && arguments.len() != 3 {
                    return Err(Self::arity(name, "two or three", arguments.len()));
                }
                let old_name = Self::package_designator_name(&arguments[0], span)?;
                let new_name = Self::package_designator_name(&arguments[1], span)?;
                let nicknames = arguments
                    .get(2)
                    .map(|value| {
                        let values = match value {
                            Value::Nil => Vec::new(),
                            Value::List(values) => values.as_ref().clone(),
                            _ => vec![value.clone()],
                        };
                        values
                            .into_iter()
                            .map(|value| Self::package_designator_name(&value, span))
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .transpose()?
                    .unwrap_or_default();
                let renamed = self.packages.borrow_mut().rename_package(&old_name, &new_name, nicknames)
                    .map_err(|message| Self::package_error(&message, span))?;
                Ok(Value::boolean(renamed))
            })());
        }
        if name != "MAKE-PACKAGE" {
            return None;
        }
        Some((|| -> Result<Value, RuntimeError> {
            if arguments.is_empty() {
                return Err(Self::arity(name, "at least one", arguments.len()));
            }
            let package_name = Self::package_designator_name(&arguments[0], span)?;
            let mut nicknames = Vec::new();
            let mut use_packages = vec![package::COMMON_LISP_PACKAGE.to_string()];
            let mut index = 1;
            while index < arguments.len() {
                let keyword = arguments[index].symbol_name().ok_or_else(|| RuntimeError::Type {
                    expected: "KEYWORD".into(), actual: arguments[index].type_name().into(), span: Some(span),
                })?;
                let value = arguments.get(index + 1).ok_or_else(|| Self::arity(name, "keyword/value pairs", arguments.len()))?;
                let values = match value { Value::Nil => Vec::new(), Value::List(values) => values.as_ref().clone(), _ => vec![value.clone()] };
                match keyword.trim_start_matches(':').to_ascii_uppercase().as_str() {
                    "NICKNAMES" => nicknames = values.into_iter().map(|value| Self::package_designator_name(&value, span)).collect::<Result<_, _>>()?,
                    "USE" => use_packages = values.into_iter().map(|value| Self::package_name_from_value(&self, &value, span)).collect::<Result<_, _>>()?,
                    _ => return Err(Self::package_error(&format!("unknown MAKE-PACKAGE keyword {keyword}"), span)),
                }
                index += 2;
            }
            self.packages.borrow_mut().define_package(&package_name, nicknames, use_packages, HashSet::new(), None, HashMap::new())
                .map_err(|message| Self::package_error(&message, span))?;
            Ok(Value::package(package_name))
        })())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ncl_syntax::Span;

    #[test]
    fn make_package_accepts_nicknames_and_use_options() {
        let runtime = Runtime::new();
        let result = runtime
            .apply_package_creation_primitive(
                "MAKE-PACKAGE",
                &[
                    Value::string("created-package"),
                    Value::symbol(":NICKNAMES"),
                    Value::list(vec![Value::string("created-nickname")]),
                    Value::symbol(":USE"),
                    Value::list(vec![Value::string(package::COMMON_LISP_PACKAGE)]),
                ],
                Span::new(0, 0),
            )
            .expect("MAKE-PACKAGE should be recognized")
            .expect("MAKE-PACKAGE should create the package");
        assert!(matches!(result, Value::Package(name) if name.as_ref() == "CREATED-PACKAGE"));
        assert!(runtime.packages.borrow().package_exists("created-nickname"));
    }
}
