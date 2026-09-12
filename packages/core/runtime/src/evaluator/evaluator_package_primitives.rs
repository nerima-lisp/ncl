#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn apply_package_listing_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        if !matches!(name, "DOCUMENTATION" | "LIST-ALL-PACKAGES") {
            return None;
        }
        let result = (|| -> Result<Value, RuntimeError> {
            match name {
                "DOCUMENTATION" => match arguments.len() {
                    2 => {
                        let package = self.package_name_from_value(&arguments[0], span)?;
                        Ok(self
                            .packages
                            .borrow()
                            .package_documentation(&package)
                            .map_or(Value::Nil, |documentation| {
                                Value::string(documentation.as_str())
                            }))
                    }
                    _ => Err(Self::arity("documentation", "two", arguments.len())),
                },
                "LIST-ALL-PACKAGES" => match arguments {
                    [] => {
                        let packages = self.packages.borrow();
                        let names = packages.all_package_names();
                        Ok(Value::list(
                            names
                                .into_iter()
                                .filter_map(|name| {
                                    packages
                                        .package_object_for(&name)
                                        .map(Value::package_object)
                                })
                                .collect(),
                        ))
                    }
                    _ => Err(Self::arity("list-all-packages", "zero", arguments.len())),
                },
                _ => unreachable!("package listing primitive name was prevalidated"),
            }
        })();
        Some(result)
    }

    pub(crate) fn apply_method_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        if !matches!(name, "CALL-NEXT-METHOD" | "NEXT-METHOD-P") {
            return None;
        }
        let result = match name {
            "CALL-NEXT-METHOD" => {
                let (continuation, default_arguments) = {
                    let contexts = self.method_context.borrow();
                    let Some(context) = contexts.last() else {
                        return Some(Err(Self::invalid(
                            "call-next-method is only available in a method",
                            span,
                        )));
                    };
                    (context.next.clone(), context.arguments.clone())
                };
                let Some(continuation) = continuation else {
                    return Some(Err(Self::invalid("no next method is applicable", span)));
                };
                let next_arguments = match arguments {
                    [] => default_arguments,
                    _ => arguments.to_vec(),
                };
                self.invoke_continuation(continuation, &next_arguments, span, environment)
            }
            "NEXT-METHOD-P" => match arguments {
                [] => {
                    let has_next = self
                        .method_context
                        .borrow()
                        .last()
                        .and_then(|context| context.next.as_ref())
                        .is_some();
                    Ok(Value::boolean(has_next))
                }
                _ => Err(Self::arity("next-method-p", "zero", arguments.len())),
            },
            _ => unreachable!("method primitive name was prevalidated"),
        };
        Some(result)
    }
}
